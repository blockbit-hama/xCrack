//! 실시간 가격 모니터링 시스템
//! 
//! 이 모듈은 여러 거래소에서 실시간으로 가격 데이터를 수집하고
//! 아비트리지 기회를 탐지하기 위한 가격 모니터링 시스템을 제공합니다.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::{interval, sleep};
use tracing::{info, debug, warn, error};
use ethers::types::U256;
use rust_decimal::Decimal;
use chrono::{Utc, Duration as ChronoDuration};

use crate::config::Config;
use crate::exchange::{ExchangeClient, ExchangeClientFactory};
use super::types::{
    PriceData, OrderBookSnapshot, ExchangeInfo, ExchangeType, 
    MonitoringStatus, PriceFeedStatus, MicroArbitrageConfig
};

/// 실시간 가격 모니터
pub struct PriceMonitor {
    config: Arc<Config>,
    exchanges: Arc<RwLock<HashMap<String, ExchangeInfo>>>,
    exchange_clients: Arc<RwLock<HashMap<String, Arc<dyn ExchangeClient>>>>,
    price_cache: Arc<RwLock<HashMap<String, HashMap<String, PriceData>>>>,
    orderbook_cache: Arc<RwLock<HashMap<String, HashMap<String, OrderBookSnapshot>>>>,
    
    // 상태 관리
    is_running: Arc<RwLock<bool>>,
    monitoring_status: Arc<RwLock<MonitoringStatus>>,
    price_feed_status: Arc<RwLock<PriceFeedStatus>>,
    
    // 채널
    price_sender: Option<mpsc::UnboundedSender<PriceData>>,
    orderbook_sender: Option<mpsc::UnboundedSender<OrderBookSnapshot>>,
    
    // 설정
    update_interval_ms: u64,
    max_retries: u32,
    timeout_ms: u64,
}

impl PriceMonitor {
    /// 새로운 가격 모니터 생성
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        info!("📊 가격 모니터 초기화 중...");
        
        let micro_config = &config.strategies.micro_arbitrage;
        let mut exchanges = HashMap::new();
        let mut exchange_clients = HashMap::new();
        
        // 거래소 정보 및 클라이언트 초기화
        for exchange_config in &micro_config.exchanges {
            if exchange_config.enabled {
                let exchange_info = ExchangeInfo {
                    name: exchange_config.name.clone(),
                    exchange_type: match exchange_config.exchange_type {
                        crate::config::ExchangeType::DEX => ExchangeType::DEX,
                        crate::config::ExchangeType::CEX => ExchangeType::CEX,
                    },
                    api_endpoint: exchange_config.api_endpoint.clone(),
                    trading_pairs: exchange_config.trading_pairs.clone(),
                    fee_percentage: exchange_config.fee_percentage,
                    min_order_size: exchange_config.min_order_size.parse::<u64>()
                        .map(U256::from)
                        .unwrap_or(U256::from(10)),
                    max_order_size: exchange_config.max_order_size.parse::<u64>()
                        .map(U256::from)
                        .unwrap_or(U256::from(100000)),
                    latency_ms: 50, // 기본 지연시간
                    is_active: false,
                    last_heartbeat: None,
                };
                
                // 거래소 클라이언트 생성
                let client = Self::create_exchange_client(exchange_config).await?;
                exchange_clients.insert(exchange_config.name.clone(), client);
                exchanges.insert(exchange_config.name.clone(), exchange_info);
            }
        }
        
        let update_interval_ms = micro_config.price_update_interval_ms;
        
        info!("✅ 가격 모니터 초기화 완료 - {}개 거래소", exchanges.len());
        
        Ok(Self {
            config,
            exchanges: Arc::new(RwLock::new(exchanges)),
            exchange_clients: Arc::new(RwLock::new(exchange_clients)),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            orderbook_cache: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            monitoring_status: Arc::new(RwLock::new(MonitoringStatus {
                is_running: false,
                active_exchanges: 0,
                failed_exchanges: 0,
                avg_latency_ms: 0.0,
                data_quality_score: 0.0,
                last_heartbeat: None,
                error_count: 0,
                last_error: None,
            })),
            price_feed_status: Arc::new(RwLock::new(PriceFeedStatus {
                is_active: false,
                feeds_count: 0,
                last_update: None,
                update_frequency_ms: update_interval_ms,
                missed_updates: 0,
                data_freshness_ms: 0,
            })),
            price_sender: None,
            orderbook_sender: None,
            update_interval_ms,
            max_retries: 3,
            timeout_ms: 5000,
        })
    }
    
    /// 가격 모니터링 시작
    pub async fn start(
        &mut self,
        price_sender: mpsc::UnboundedSender<PriceData>,
        orderbook_sender: mpsc::UnboundedSender<OrderBookSnapshot>,
    ) -> Result<()> {
        info!("🚀 가격 모니터링 시작...");
        
        self.price_sender = Some(price_sender);
        self.orderbook_sender = Some(orderbook_sender);
        
        // 실행 상태 설정
        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }
        
        // 가격 피드 상태 업데이트
        {
            let mut status = self.price_feed_status.write().await;
            status.is_active = true;
            status.feeds_count = self.exchanges.read().await.len() as u32;
        }
        
        // 모니터링 상태 업데이트
        {
            let mut status = self.monitoring_status.write().await;
            status.is_running = true;
            status.last_heartbeat = Some(Instant::now());
        }
        
        // 각 거래소별 모니터링 태스크 시작
        self.start_exchange_monitoring().await?;
        
        // 헬스 체크 태스크 시작
        self.start_health_monitoring().await;
        
        info!("✅ 가격 모니터링 시작 완료");
        Ok(())
    }
    
    /// 가격 모니터링 중지
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 가격 모니터링 중지 중...");
        
        // 실행 상태 설정
        {
            let mut is_running = self.is_running.write().await;
            *is_running = false;
        }
        
        // 가격 피드 상태 업데이트
        {
            let mut status = self.price_feed_status.write().await;
            status.is_active = false;
        }
        
        // 모니터링 상태 업데이트
        {
            let mut status = self.monitoring_status.write().await;
            status.is_running = false;
        }
        
        info!("✅ 가격 모니터링 중지 완료");
        Ok(())
    }
    
    /// 거래소별 모니터링 시작
    async fn start_exchange_monitoring(&self) -> Result<()> {
        let exchanges = self.exchanges.read().await;
        let exchange_clients = self.exchange_clients.read().await;
        
        for (exchange_name, exchange_info) in exchanges.iter() {
            if let Some(client) = exchange_clients.get(exchange_name) {
                let monitor = ExchangeMonitor::new(
                    exchange_name.clone(),
                    exchange_info.clone(),
                    client.clone(),
                    self.price_sender.clone(),
                    self.orderbook_sender.clone(),
                    self.update_interval_ms,
                    self.max_retries,
                    self.timeout_ms,
                );
                
                tokio::spawn(async move {
                    if let Err(e) = monitor.start().await {
                        error!("❌ {} 모니터링 실패: {}", exchange_name, e);
                    }
                });
            }
        }
        
        Ok(())
    }
    
    /// 헬스 모니터링 시작
    async fn start_health_monitoring(&self) {
        let is_running = Arc::clone(&self.is_running);
        let monitoring_status = Arc::clone(&self.monitoring_status);
        let price_feed_status = Arc::clone(&self.price_feed_status);
        let exchanges = Arc::clone(&self.exchanges);
        
        tokio::spawn(async move {
            let mut health_interval = interval(Duration::from_secs(10));
            
            while *is_running.read().await {
                health_interval.tick().await;
                
                // 모니터링 상태 업데이트
                let mut status = monitoring_status.write().await;
                status.last_heartbeat = Some(Instant::now());
                
                // 활성 거래소 수 계산
                let exchanges_guard = exchanges.read().await;
                let active_count = exchanges_guard.values()
                    .filter(|ex| ex.is_active)
                    .count() as u32;
                let total_count = exchanges_guard.len() as u32;
                
                status.active_exchanges = active_count;
                status.failed_exchanges = total_count - active_count;
                
                // 가격 피드 상태 업데이트
                let mut feed_status = price_feed_status.write().await;
                feed_status.data_freshness_ms = if let Some(last_update) = feed_status.last_update {
                    Utc::now().signed_duration_since(last_update).num_milliseconds() as u64
                } else {
                    u64::MAX
                };
                
                debug!("🏥 헬스 체크 - 활성: {}/{}", active_count, total_count);
            }
        });
    }
    
    /// 거래소 클라이언트 생성
    async fn create_exchange_client(
        exchange_config: &crate::config::ExchangeConfig,
    ) -> Result<Arc<dyn ExchangeClient>> {
        match exchange_config.exchange_type {
            crate::config::ExchangeType::CEX => {
                match exchange_config.name.to_lowercase().as_str() {
                    "binance" => {
                        let api_key = std::env::var("BINANCE_API_KEY")
                            .or_else(|_| exchange_config.api_key.as_ref().cloned().ok_or_else(|| anyhow!("BINANCE_API_KEY not found")))?;
                        let secret_key = std::env::var("BINANCE_SECRET_KEY")
                            .or_else(|_| exchange_config.secret_key.as_ref().cloned().ok_or_else(|| anyhow!("BINANCE_SECRET_KEY not found")))?;
                        Ok(ExchangeClientFactory::create_binance_client(api_key, secret_key))
                    }
                    "coinbase" => {
                        let api_key = std::env::var("COINBASE_API_KEY")
                            .or_else(|_| exchange_config.api_key.as_ref().cloned().ok_or_else(|| anyhow!("COINBASE_API_KEY not found")))?;
                        let secret_key = std::env::var("COINBASE_SECRET_KEY")
                            .or_else(|_| exchange_config.secret_key.as_ref().cloned().ok_or_else(|| anyhow!("COINBASE_SECRET_KEY not found")))?;
                        let passphrase = std::env::var("COINBASE_PASSPHRASE")
                            .or_else(|_| exchange_config.passphrase.as_ref().cloned().ok_or_else(|| anyhow!("COINBASE_PASSPHRASE not found")))?;
                        Ok(ExchangeClientFactory::create_coinbase_client(api_key, secret_key, passphrase))
                    }
                    _ => {
                        warn!("⚠️ 지원되지 않는 CEX: {}, Binance로 폴백", exchange_config.name);
                        let api_key = std::env::var("BINANCE_API_KEY").unwrap_or_default();
                        let secret_key = std::env::var("BINANCE_SECRET_KEY").unwrap_or_default();
                        Ok(ExchangeClientFactory::create_binance_client(api_key, secret_key))
                    }
                }
            }
            crate::config::ExchangeType::DEX => {
                match exchange_config.name.to_lowercase().as_str() {
                    "uniswap_v2" => Ok(ExchangeClientFactory::create_uniswap_v2_client()),
                    "uniswap_v3" => Ok(ExchangeClientFactory::create_uniswap_v3_client()),
                    "sushiswap" => Ok(ExchangeClientFactory::create_sushiswap_client()),
                    _ => {
                        warn!("⚠️ 지원되지 않는 DEX: {}, Uniswap V2로 폴백", exchange_config.name);
                        Ok(ExchangeClientFactory::create_uniswap_v2_client())
                    }
                }
            }
        }
    }
    
    /// 특정 거래소의 가격 데이터 가져오기
    pub async fn get_price_data(&self, exchange: &str, symbol: &str) -> Option<PriceData> {
        let cache = self.price_cache.read().await;
        cache.get(exchange)?.get(symbol).cloned()
    }
    
    /// 특정 거래소의 오더북 데이터 가져오기
    pub async fn get_orderbook_data(&self, exchange: &str, symbol: &str) -> Option<OrderBookSnapshot> {
        let cache = self.orderbook_cache.read().await;
        cache.get(exchange)?.get(symbol).cloned()
    }
    
    /// 모든 거래소의 가격 데이터 가져오기
    pub async fn get_all_price_data(&self, symbol: &str) -> HashMap<String, PriceData> {
        let cache = self.price_cache.read().await;
        let mut result = HashMap::new();
        
        for (exchange, prices) in cache.iter() {
            if let Some(price_data) = prices.get(symbol) {
                result.insert(exchange.clone(), price_data.clone());
            }
        }
        
        result
    }
    
    /// 모니터링 상태 가져오기
    pub async fn get_monitoring_status(&self) -> MonitoringStatus {
        self.monitoring_status.read().await.clone()
    }
    
    /// 가격 피드 상태 가져오기
    pub async fn get_price_feed_status(&self) -> PriceFeedStatus {
        self.price_feed_status.read().await.clone()
    }
    
    /// 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

/// 개별 거래소 모니터
struct ExchangeMonitor {
    exchange_name: String,
    exchange_info: ExchangeInfo,
    client: Arc<dyn ExchangeClient>,
    price_sender: Option<mpsc::UnboundedSender<PriceData>>,
    orderbook_sender: Option<mpsc::UnboundedSender<OrderBookSnapshot>>,
    update_interval_ms: u64,
    max_retries: u32,
    timeout_ms: u64,
}

impl ExchangeMonitor {
    fn new(
        exchange_name: String,
        exchange_info: ExchangeInfo,
        client: Arc<dyn ExchangeClient>,
        price_sender: Option<mpsc::UnboundedSender<PriceData>>,
        orderbook_sender: Option<mpsc::UnboundedSender<OrderBookSnapshot>>,
        update_interval_ms: u64,
        max_retries: u32,
        timeout_ms: u64,
    ) -> Self {
        Self {
            exchange_name,
            exchange_info,
            client,
            price_sender,
            orderbook_sender,
            update_interval_ms,
            max_retries,
            timeout_ms,
        }
    }
    
    async fn start(&self) -> Result<()> {
        info!("📡 {} 모니터링 시작", self.exchange_name);
        
        let mut update_interval = interval(Duration::from_millis(self.update_interval_ms));
        
        loop {
            update_interval.tick().await;
            
            // 연결 상태 확인
            if !self.client.is_connected().await {
                warn!("⚠️ {} 연결 끊어짐, 재연결 시도 중...", self.exchange_name);
                continue;
            }
            
            // 각 거래 페어에 대해 가격 데이터 수집
            for symbol in &self.exchange_info.trading_pairs {
                if let Err(e) = self.update_price_data(symbol).await {
                    warn!("⚠️ {} {} 가격 업데이트 실패: {}", self.exchange_name, symbol, e);
                }
                
                if let Err(e) = self.update_orderbook_data(symbol).await {
                    warn!("⚠️ {} {} 오더북 업데이트 실패: {}", self.exchange_name, symbol, e);
                }
            }
        }
    }
    
    async fn update_price_data(&self, symbol: &str) -> Result<()> {
        let mut retries = 0;
        
        while retries < self.max_retries {
            match self.client.get_current_price(symbol).await {
                Ok(price) => {
                    let price_data = PriceData {
                        symbol: symbol.to_string(),
                        exchange: self.exchange_name.clone(),
                        bid: price.bid,
                        ask: price.ask,
                        last_price: price.last_price,
                        volume_24h: price.volume_24h,
                        timestamp: Utc::now(),
                        sequence: 0, // 실제로는 거래소에서 제공
                        spread: price.ask - price.bid,
                        price_impact: 0.0, // 나중에 계산
                    };
                    
                    // 가격 데이터 전송
                    if let Some(sender) = &self.price_sender {
                        if let Err(e) = sender.send(price_data) {
                            warn!("⚠️ 가격 데이터 전송 실패: {}", e);
                        }
                    }
                    
                    return Ok(());
                }
                Err(e) => {
                    retries += 1;
                    if retries < self.max_retries {
                        warn!("⚠️ {} 가격 조회 실패 (시도 {}/{}): {}", 
                              self.exchange_name, retries, self.max_retries, e);
                        sleep(Duration::from_millis(1000)).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        
        Err(anyhow!("최대 재시도 횟수 초과"))
    }
    
    async fn update_orderbook_data(&self, symbol: &str) -> Result<()> {
        let mut retries = 0;
        
        while retries < self.max_retries {
            match self.client.get_orderbook(symbol).await {
                Ok(orderbook) => {
                    let orderbook_snapshot = OrderBookSnapshot {
                        symbol: symbol.to_string(),
                        exchange: self.exchange_name.clone(),
                        bids: orderbook.bids,
                        asks: orderbook.asks,
                        timestamp: Utc::now(),
                        sequence: 0, // 실제로는 거래소에서 제공
                    };
                    
                    // 오더북 데이터 전송
                    if let Some(sender) = &self.orderbook_sender {
                        if let Err(e) = sender.send(orderbook_snapshot) {
                            warn!("⚠️ 오더북 데이터 전송 실패: {}", e);
                        }
                    }
                    
                    return Ok(());
                }
                Err(e) => {
                    retries += 1;
                    if retries < self.max_retries {
                        warn!("⚠️ {} 오더북 조회 실패 (시도 {}/{}): {}", 
                              self.exchange_name, retries, self.max_retries, e);
                        sleep(Duration::from_millis(1000)).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        
        Err(anyhow!("최대 재시도 횟수 초과"))
    }
}