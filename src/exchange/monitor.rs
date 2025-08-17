use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use tokio::time::{sleep, Duration, interval};
use rust_decimal::Decimal;
use chrono::Utc;
use serde::Deserialize;
use std::str::FromStr;

use crate::config::{Config, ExchangeConfig, ExchangeType};
use crate::types::{PriceData, OrderBookSnapshot, OrderBookLevel, ExchangeInfo};
use alloy::primitives::U256;

#[derive(Debug, Deserialize)]
struct BinanceBookTicker {
    symbol: String,
    #[serde(rename = "bidPrice")] bid_price: String,
    #[serde(rename = "bidQty")] bid_qty: String,
    #[serde(rename = "askPrice")] ask_price: String,
    #[serde(rename = "askQty")] ask_qty: String,
}

#[derive(Debug, Deserialize)]
struct DexScreenerPair {
    #[serde(rename = "priceUsd")] price_usd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerResponse {
    pairs: Option<Vec<DexScreenerPair>>,
}

#[derive(Debug, Deserialize)]
struct BinanceDepth {
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

/// 여러 거래소를 동시에 모니터링하는 시스템
/// 
/// 각 거래소의 가격, 오더북, 유동성 데이터를 실시간으로 수집하고
/// 마이크로아비트래지 전략에 데이터를 공급합니다.
pub struct ExchangeMonitor {
    config: Arc<Config>,
    is_running: Arc<AtomicBool>,
    
    // 데이터 전송 채널들
    price_sender: Option<mpsc::UnboundedSender<PriceData>>,
    orderbook_sender: Option<mpsc::UnboundedSender<OrderBookSnapshot>>,
    
    // 거래소별 연결 상태
    connection_status: Arc<tokio::sync::Mutex<HashMap<String, ConnectionStatus>>>,
    
    // 모니터링 통계
    stats: Arc<tokio::sync::Mutex<MonitoringStats>>,
}

#[derive(Debug, Clone)]
struct ConnectionStatus {
    is_connected: bool,
    last_update: chrono::DateTime<Utc>,
    latency_ms: u64,
    error_count: u32,
    reconnect_attempts: u32,
}

#[derive(Debug, Clone)]
pub struct MonitoringStats {
    pub total_price_updates: u64,
    pub total_orderbook_updates: u64,
    pub active_connections: u32,
    pub failed_connections: u32,
    pub avg_latency_ms: f64,
    pub uptime_percentage: f64,
    pub data_quality_score: f64,
}

impl ExchangeMonitor {
    /// 블랙리스트 토큰이 포함된 페어인지 확인
    fn is_pair_blacklisted(&self, pair: &str) -> bool {
        let upper_pair = pair.to_uppercase();
        for token in &self.config.strategies.micro_arbitrage.blacklist_tokens {
            if upper_pair.contains(&token.to_uppercase()) {
                return true;
            }
        }
        false
    }
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            price_sender: None,
            orderbook_sender: None,
            connection_status: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            stats: Arc::new(tokio::sync::Mutex::new(MonitoringStats {
                total_price_updates: 0,
                total_orderbook_updates: 0,
                active_connections: 0,
                failed_connections: 0,
                avg_latency_ms: 0.0,
                uptime_percentage: 100.0,
                data_quality_score: 1.0,
            })),
        }
    }
    
    /// 모니터링 시작
    pub async fn start(
        &mut self,
        price_sender: mpsc::UnboundedSender<PriceData>,
        orderbook_sender: mpsc::UnboundedSender<OrderBookSnapshot>,
    ) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(anyhow!("ExchangeMonitor is already running"));
        }
        
        self.price_sender = Some(price_sender);
        self.orderbook_sender = Some(orderbook_sender);
        
        info!("🔍 거래소 모니터링 시작");
        info!("  📊 모니터링 대상: {}개 거래소", self.config.strategies.micro_arbitrage.exchanges.len());
        info!("  💱 거래 페어: {}개", self.config.strategies.micro_arbitrage.trading_pairs.len());
        
        self.is_running.store(true, Ordering::SeqCst);
        
        // 각 거래소별 모니터링 태스크 시작
        for exchange_config in &self.config.strategies.micro_arbitrage.exchanges {
            if exchange_config.enabled {
                self.start_exchange_monitoring(exchange_config.clone()).await?;
            }
        }
        
        // 통계 업데이트 태스크 시작
        self.start_stats_updater().await;
        
        // 연결 상태 모니터링 태스크 시작
        self.start_health_monitor().await;
        
        info!("✅ 거래소 모니터링 시작 완료");
        Ok(())
    }
    
    /// 모니터링 중지
    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);
        
        // 모든 연결 종료 대기 (최대 5초)
        let mut wait_time = 0;
        while wait_time < 5000 {
            let connections = self.connection_status.lock().await;
            let active_count = connections.values().filter(|status| status.is_connected).count();
            
            if active_count == 0 {
                break;
            }
            
            drop(connections);
            sleep(Duration::from_millis(100)).await;
            wait_time += 100;
        }
        
        info!("⏹️ 거래소 모니터링 중지됨");
        Ok(())
    }
    
    /// 특정 거래소 모니터링 시작
    async fn start_exchange_monitoring(&self, exchange_config: ExchangeConfig) -> Result<()> {
        let exchange_name = exchange_config.name.clone();
        
        // 연결 상태 초기화
        {
            let mut status = self.connection_status.lock().await;
            status.insert(exchange_name.clone(), ConnectionStatus {
                is_connected: false,
                last_update: Utc::now(),
                latency_ms: 0,
                error_count: 0,
                reconnect_attempts: 0,
            });
        }
        
        match exchange_config.exchange_type {
            ExchangeType::DEX => {
                self.start_dex_monitoring(exchange_config).await?;
            }
            ExchangeType::CEX => {
                self.start_cex_monitoring(exchange_config).await?;
            }
        }
        
        Ok(())
    }
    
    /// DEX 모니터링 시작
    async fn start_dex_monitoring(&self, exchange_config: ExchangeConfig) -> Result<()> {
        let exchange_name = exchange_config.name.clone();
        let is_running = Arc::clone(&self.is_running);
        let price_sender = self.price_sender.as_ref().unwrap().clone();
        let orderbook_sender = self.orderbook_sender.as_ref().unwrap().clone();
        let connection_status = Arc::clone(&self.connection_status);
        let stats = Arc::clone(&self.stats);
        let trading_pairs = self.config.strategies.micro_arbitrage.trading_pairs.clone();
        let filtered_pairs: Vec<String> = trading_pairs
            .iter()
            .filter(|p| !self.is_pair_blacklisted(p))
            .cloned()
            .collect();
        let update_interval = Duration::from_millis(self.config.strategies.micro_arbitrage.price_update_interval_ms);
        
        info!("🌐 DEX 모니터링 시작: {}", exchange_name);
        
        // DEX 모니터링 태스크 스폰
        tokio::spawn(async move {
            let mut sequence = 0u64;
            let mut reconnect_attempts = 0u32;
            
            while is_running.load(Ordering::SeqCst) {
                // Mock 모드에서는 시뮬레이션된 데이터 생성
                if crate::mocks::is_mock_mode() {
                    match Self::generate_mock_dex_data(&exchange_name, &trading_pairs, sequence).await {
                        Ok(data) => {
                            for (price_data, orderbook_data) in data {
                                if let Err(e) = price_sender.send(price_data) {
                                    error!("가격 데이터 전송 실패: {}", e);
                                    break;
                                }
                                
                                if let Err(e) = orderbook_sender.send(orderbook_data) {
                                    error!("오더북 데이터 전송 실패: {}", e);
                                    break;
                                }
                            }
                            
                            // 연결 상태 업데이트
                            Self::update_connection_status(&connection_status, &exchange_name, true, 10 + fastrand::u64(5..15)).await;
                            
                            // 통계 업데이트
                            Self::update_monitoring_stats(&stats, trading_pairs.len() as u64, trading_pairs.len() as u64).await;
                            
                            sequence += 1;
                            reconnect_attempts = 0;
                        }
                        Err(e) => {
                            error!("DEX 데이터 생성 실패: {}", e);
                            Self::update_connection_status(&connection_status, &exchange_name, false, 0).await;
                            reconnect_attempts += 1;
                        }
                    }
                } else {
                    // 실제 DEX 데이터: DexScreener를 통해 심볼 기반 가격을 조회하고 synthetic orderbook 구성
                    let http = reqwest::Client::builder()
                        .timeout(Duration::from_millis(2000))
                        .build()
                        .unwrap();
                    let mut any_success = false;
                    let spread = 0.001; // 0.1% synthetic spread
                    for pair in &filtered_pairs {
                        match Self::fetch_dexscreener_price(&http, pair).await {
                            Ok(last) => {
                                let bid = last * Decimal::from_str(&format!("{:.6}", 1.0 - spread / 2.0)).unwrap_or(Decimal::ONE);
                                let ask = last * Decimal::from_str(&format!("{:.6}", 1.0 + spread / 2.0)).unwrap_or(Decimal::ONE);
                                let price = PriceData {
                                    symbol: pair.clone(),
                                    exchange: exchange_name.clone(),
                                    bid,
                                    ask,
                                    last_price: last,
                                    volume_24h: U256::from(0u64),
                                    timestamp: Utc::now(),
                                    sequence,
                                };
                                // 얕은 synthetic 오더북
                                let bids = vec![OrderBookLevel { price: bid, quantity: U256::from(1000u64) }];
                                let asks = vec![OrderBookLevel { price: ask, quantity: U256::from(1000u64) }];
                                let ob = OrderBookSnapshot {
                                    exchange: exchange_name.clone(),
                                    symbol: pair.clone(),
                                    bids,
                                    asks,
                                    timestamp: Utc::now(),
                                    sequence,
                                };
                                let _ = price_sender.send(price);
                                let _ = orderbook_sender.send(ob);
                                any_success = true;
                            }
                            Err(err) => {
                                debug!("DexScreener 가격 조회 실패 {}: {}", pair, err);
                            }
                        }
                    }
                    if any_success {
                        Self::update_connection_status(&connection_status, &exchange_name, true, 90).await;
                        Self::update_monitoring_stats(&stats, filtered_pairs.len() as u64, filtered_pairs.len() as u64).await;
                        sequence += 1;
                    } else {
                        warn!("{} 실제 DEX 데이터 수집 실패 (모든 페어)", exchange_name);
                        Self::update_connection_status(&connection_status, &exchange_name, false, 0).await;
                        reconnect_attempts += 1;
                    }
                }
                
                sleep(update_interval).await;
            }
            
            info!("🔌 DEX 모니터링 종료: {}", exchange_name);
        });
        
        Ok(())
    }
    
    /// CEX 모니터링 시작
    async fn start_cex_monitoring(&self, exchange_config: ExchangeConfig) -> Result<()> {
        let exchange_name = exchange_config.name.clone();
        let is_running = Arc::clone(&self.is_running);
        let price_sender = self.price_sender.as_ref().unwrap().clone();
        let orderbook_sender = self.orderbook_sender.as_ref().unwrap().clone();
        let connection_status = Arc::clone(&self.connection_status);
        let stats = Arc::clone(&self.stats);
        let trading_pairs = self.config.strategies.micro_arbitrage.trading_pairs.clone();
        let filtered_pairs: Vec<String> = trading_pairs
            .iter()
            .filter(|p| !self.is_pair_blacklisted(p))
            .cloned()
            .collect();
        let update_interval = Duration::from_millis(self.config.strategies.micro_arbitrage.price_update_interval_ms);
        let orderbook_depth = self.config.strategies.micro_arbitrage.order_book_depth as usize;
        
        info!("🏛️ CEX 모니터링 시작: {}", exchange_name);
        
        // CEX 모니터링 태스크 스폰
        tokio::spawn(async move {
            let mut sequence = 0u64;
            let mut reconnect_attempts = 0u32;
            let http = reqwest::Client::builder()
                .timeout(Duration::from_millis(1500))
                .build()
                .unwrap();
            
            while is_running.load(Ordering::SeqCst) {
                // Mock 모드에서는 시뮬레이션된 데이터 생성
                if crate::mocks::is_mock_mode() {
                    match Self::generate_mock_cex_data(&exchange_name, &trading_pairs, sequence).await {
                        Ok(data) => {
                            for (price_data, orderbook_data) in data {
                                if let Err(e) = price_sender.send(price_data) {
                                    error!("가격 데이터 전송 실패: {}", e);
                                    break;
                                }
                                
                                if let Err(e) = orderbook_sender.send(orderbook_data) {
                                    error!("오더북 데이터 전송 실패: {}", e);
                                    break;
                                }
                            }
                            
                            // 연결 상태 업데이트
                            Self::update_connection_status(&connection_status, &exchange_name, true, 5 + fastrand::u64(2..8)).await;
                            
                            // 통계 업데이트
                            Self::update_monitoring_stats(&stats, trading_pairs.len() as u64, trading_pairs.len() as u64).await;
                            
                            sequence += 1;
                            reconnect_attempts = 0;
                        }
                        Err(e) => {
                            error!("CEX 데이터 생성 실패: {}", e);
                            Self::update_connection_status(&connection_status, &exchange_name, false, 0).await;
                            reconnect_attempts += 1;
                        }
                    }
                } else {
                    // 실제 CEX API 호출: Binance 호환 엔드포인트 우선 지원
                    let mut any_success = false;
                    for pair in &filtered_pairs {
                        if let Some(binance_symbol) = Self::to_binance_symbol(pair) {
                            match Self::fetch_binance_book_ticker(&http, &binance_symbol).await {
                                Ok((bid, ask, last)) => {
                                    let (bids, asks) = match Self::fetch_binance_orderbook(&http, &binance_symbol, orderbook_depth as u32).await {
                                        Ok((b, a)) => (b, a),
                                        Err(_) => (vec![OrderBookLevel { price: bid, quantity: U256::from(0u64) }],
                                                   vec![OrderBookLevel { price: ask, quantity: U256::from(0u64) }]),
                                    };
                                    let price = PriceData {
                                        symbol: pair.clone(),
                                        exchange: exchange_name.clone(),
                                        bid,
                                        ask,
                                        last_price: last,
                                        volume_24h: U256::from(0u64),
                                        timestamp: Utc::now(),
                                        sequence,
                                    };
                                    let ob = OrderBookSnapshot {
                                        exchange: exchange_name.clone(),
                                        symbol: pair.clone(),
                                        bids,
                                        asks,
                                        timestamp: Utc::now(),
                                        sequence,
                                    };
                                    let _ = price_sender.send(price);
                                    let _ = orderbook_sender.send(ob);
                                    any_success = true;
                                }
                                Err(err) => {
                                    debug!("Binance 티커 실패 {}: {}", binance_symbol, err);
                                    continue;
                                }
                            }
                        } else {
                            debug!("매핑 불가 CEX 심볼: {} ({}에서 스킵)", pair, exchange_name);
                        }
                    }

                    if any_success {
                        Self::update_connection_status(&connection_status, &exchange_name, true, 40).await;
                        Self::update_monitoring_stats(&stats, filtered_pairs.len() as u64, filtered_pairs.len() as u64).await;
                        sequence += 1;
                    } else {
                        warn!("{} 실제 CEX 데이터 수집 실패 (모든 페어)", exchange_name);
                        Self::update_connection_status(&connection_status, &exchange_name, false, 0).await;
                        reconnect_attempts += 1;
                    }
                }
                
                sleep(update_interval).await;
            }
            
            info!("🔌 CEX 모니터링 종료: {}", exchange_name);
        });
        
        Ok(())
    }
    
    /// Mock DEX 데이터 생성
    async fn generate_mock_dex_data(
        exchange_name: &str,
        trading_pairs: &[String],
        sequence: u64,
    ) -> Result<Vec<(PriceData, OrderBookSnapshot)>> {
        let mut data = Vec::new();
        let timestamp = Utc::now();
        
        for pair in trading_pairs {
            // 기본 가격 (거래소별로 약간의 차이)
            let base_price = match pair.as_str() {
                "WETH/USDC" => 2000.0,
                "WETH/USDT" => 2001.0,
                "WETH/DAI" => 1999.0,
                "WBTC/USDC" => 45000.0,
                "WBTC/USDT" => 45050.0,
                _ => 100.0,
            };
            
            // DEX는 일반적으로 더 높은 슬리피지와 변동성
            let price_volatility = 0.02; // 2% 변동성
            let spread = 0.005; // 0.5% 스프레드
            
            // 거래소별 가격 차이
            let exchange_multiplier = match exchange_name {
                "uniswap_v2" => 1.0,
                "sushiswap" => 0.999, // 약간 낮은 가격
                _ => 1.0,
            };
            
            let price_adjustment = (fastrand::f64() - 0.5) * price_volatility;
            let adjusted_price = base_price * exchange_multiplier * (1.0 + price_adjustment);
            
            let bid_price = adjusted_price * (1.0 - spread / 2.0);
            let ask_price = adjusted_price * (1.0 + spread / 2.0);
            
            // 가격 데이터 생성
            let price_data = PriceData {
                symbol: pair.clone(),
                exchange: exchange_name.to_string(),
                bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
                ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
                last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
                volume_24h: U256::from(fastrand::u64(100000..1000000)),
                timestamp,
                sequence,
            };
            
            // 오더북 데이터 생성
            let mut bids = Vec::new();
            let mut asks = Vec::new();
            
            // 10개 레벨 생성
            for i in 0..10 {
                let bid_level_price = bid_price * (1.0 - (i as f64) * 0.001);
                let ask_level_price = ask_price * (1.0 + (i as f64) * 0.001);
                
                bids.push(OrderBookLevel {
                    price: Decimal::from_f64_retain(bid_level_price).unwrap_or_default(),
                    quantity: U256::from(fastrand::u64(100..10000)),
                });
                
                asks.push(OrderBookLevel {
                    price: Decimal::from_f64_retain(ask_level_price).unwrap_or_default(),
                    quantity: U256::from(fastrand::u64(100..10000)),
                });
            }
            
            let orderbook_data = OrderBookSnapshot {
                exchange: exchange_name.to_string(),
                symbol: pair.clone(),
                bids,
                asks,
                timestamp,
                sequence,
            };
            
            data.push((price_data, orderbook_data));
        }
        
        Ok(data)
    }
    
    /// Mock CEX 데이터 생성
    async fn generate_mock_cex_data(
        exchange_name: &str,
        trading_pairs: &[String],
        sequence: u64,
    ) -> Result<Vec<(PriceData, OrderBookSnapshot)>> {
        let mut data = Vec::new();
        let timestamp = Utc::now();
        
        for pair in trading_pairs {
            // 기본 가격
            let base_price = match pair.as_str() {
                "WETH/USDC" => 2000.0,
                "WETH/USDT" => 2001.0,
                "WETH/DAI" => 1999.0,
                "WBTC/USDC" => 45000.0,
                "WBTC/USDT" => 45050.0,
                _ => 100.0,
            };
            
            // CEX는 일반적으로 더 낮은 스프레드와 안정적인 가격
            let price_volatility = 0.01; // 1% 변동성
            let spread = 0.001; // 0.1% 스프레드
            
            // 거래소별 가격 차이
            let exchange_multiplier = match exchange_name {
                "mock_binance" => 1.001, // 약간 높은 가격
                "mock_coinbase" => 0.999,
                _ => 1.0,
            };
            
            let price_adjustment = (fastrand::f64() - 0.5) * price_volatility;
            let adjusted_price = base_price * exchange_multiplier * (1.0 + price_adjustment);
            
            let bid_price = adjusted_price * (1.0 - spread / 2.0);
            let ask_price = adjusted_price * (1.0 + spread / 2.0);
            
            // 가격 데이터 생성
            let price_data = PriceData {
                symbol: pair.clone(),
                exchange: exchange_name.to_string(),
                bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
                ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
                last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
                volume_24h: U256::from(fastrand::u64(1000000..10000000)), // CEX는 더 큰 거래량
                timestamp,
                sequence,
            };
            
            // 오더북 데이터 생성 (CEX는 더 깊은 유동성)
            let mut bids = Vec::new();
            let mut asks = Vec::new();
            
            // 20개 레벨 생성
            for i in 0..20 {
                let bid_level_price = bid_price * (1.0 - (i as f64) * 0.0001);
                let ask_level_price = ask_price * (1.0 + (i as f64) * 0.0001);
                
                bids.push(OrderBookLevel {
                    price: Decimal::from_f64_retain(bid_level_price).unwrap_or_default(),
                    quantity: U256::from(fastrand::u64(1000..50000)), // CEX는 더 큰 주문 크기
                });
                
                asks.push(OrderBookLevel {
                    price: Decimal::from_f64_retain(ask_level_price).unwrap_or_default(),
                    quantity: U256::from(fastrand::u64(1000..50000)),
                });
            }
            
            let orderbook_data = OrderBookSnapshot {
                exchange: exchange_name.to_string(),
                symbol: pair.clone(),
                bids,
                asks,
                timestamp,
                sequence,
            };
            
            data.push((price_data, orderbook_data));
        }
        
        Ok(data)
    }
    
    /// 연결 상태 업데이트
    async fn update_connection_status(
        connection_status: &Arc<tokio::sync::Mutex<HashMap<String, ConnectionStatus>>>,
        exchange_name: &str,
        is_connected: bool,
        latency_ms: u64,
    ) {
        let mut status = connection_status.lock().await;
        if let Some(conn_status) = status.get_mut(exchange_name) {
            conn_status.is_connected = is_connected;
            conn_status.last_update = Utc::now();
            conn_status.latency_ms = latency_ms;
            
            if !is_connected {
                conn_status.error_count += 1;
            }
        }
    }
    
    /// 모니터링 통계 업데이트
    async fn update_monitoring_stats(
        stats: &Arc<tokio::sync::Mutex<MonitoringStats>>,
        price_updates: u64,
        orderbook_updates: u64,
    ) {
        let mut stats_guard = stats.lock().await;
        stats_guard.total_price_updates += price_updates;
        stats_guard.total_orderbook_updates += orderbook_updates;
    }
    
    /// 통계 업데이터 시작
    async fn start_stats_updater(&self) {
        let is_running = Arc::clone(&self.is_running);
        let stats = Arc::clone(&self.stats);
        let connection_status = Arc::clone(&self.connection_status);
        
        tokio::spawn(async move {
            let mut update_interval = interval(Duration::from_secs(10)); // 10초마다 업데이트
            
            while is_running.load(Ordering::SeqCst) {
                update_interval.tick().await;
                
                let connections = connection_status.lock().await;
                let active_connections = connections.values().filter(|status| status.is_connected).count() as u32;
                let total_connections = connections.len() as u32;
                let failed_connections = total_connections - active_connections;
                
                let avg_latency = if active_connections > 0 {
                    let total_latency: u64 = connections.values()
                        .filter(|status| status.is_connected)
                        .map(|status| status.latency_ms)
                        .sum();
                    total_latency as f64 / active_connections as f64
                } else {
                    0.0
                };
                
                let uptime_percentage = if total_connections > 0 {
                    (active_connections as f64 / total_connections as f64) * 100.0
                } else {
                    0.0
                };
                
                drop(connections);
                
                // 통계 업데이트
                let mut stats_guard = stats.lock().await;
                stats_guard.active_connections = active_connections;
                stats_guard.failed_connections = failed_connections;
                stats_guard.avg_latency_ms = avg_latency;
                stats_guard.uptime_percentage = uptime_percentage;
                stats_guard.data_quality_score = if uptime_percentage > 95.0 { 1.0 } else if uptime_percentage > 80.0 { 0.8 } else { 0.6 };
                
                debug!("📊 모니터링 통계 업데이트 - 활성: {}/{}, 지연: {:.1}ms, 업타임: {:.1}%",
                       active_connections, total_connections, avg_latency, uptime_percentage);
            }
        });
    }
    
    /// 헬스 모니터 시작
    async fn start_health_monitor(&self) {
        let is_running = Arc::clone(&self.is_running);
        let connection_status = Arc::clone(&self.connection_status);
        
        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(30)); // 30초마다 헬스 체크
            
            while is_running.load(Ordering::SeqCst) {
                check_interval.tick().await;
                
                let mut status = connection_status.lock().await;
                let now = Utc::now();
                
                for (exchange_name, conn_status) in status.iter_mut() {
                    // 5분 이상 업데이트가 없으면 연결 끊어진 것으로 간주
                    let time_since_update = now - conn_status.last_update;
                    if time_since_update.num_seconds() > 300 {
                        if conn_status.is_connected {
                            warn!("⚠️ 거래소 연결 타임아웃: {} (마지막 업데이트: {}초 전)", 
                                  exchange_name, time_since_update.num_seconds());
                            conn_status.is_connected = false;
                            conn_status.error_count += 1;
                        }
                    }
                    
                    // 재연결 시도 로직
                    if !conn_status.is_connected && conn_status.reconnect_attempts < 10 {
                        if conn_status.reconnect_attempts < 5 || time_since_update.num_seconds() > 60 {
                            info!("🔄 거래소 재연결 시도: {} (시도: {}회)", 
                                  exchange_name, conn_status.reconnect_attempts + 1);
                            conn_status.reconnect_attempts += 1;
                        }
                    }
                }
            }
        });
    }
    
    /// 현재 연결 상태 조회
    pub async fn get_connection_status(&self) -> HashMap<String, ConnectionStatus> {
        self.connection_status.lock().await.clone()
    }
    
    /// 모니터링 통계 조회
    pub async fn get_monitoring_stats(&self) -> MonitoringStats {
        self.stats.lock().await.clone()
    }
    
    /// 실행 중인지 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

/// 간단한 Decimal -> U256 변환: 소수부는 무시 (CEX 수량 근사)
fn decimal_to_u256(value: Decimal) -> U256 {
    if value.is_sign_negative() {
        return U256::from(0u64);
    }
    let int_part = value.trunc();
    if let Ok(v) = int_part.to_string().parse::<u128>() {
        U256::from(v)
    } else {
        U256::from(0u64)
    }
}

// =========================
// CEX(Binance) 지원 유틸
// =========================
impl ExchangeMonitor {
    /// 프로젝트 표준 심볼("WETH/USDC")을 Binance 심볼("ETHUSDC")로 변환
    fn to_binance_symbol(pair: &str) -> Option<String> {
        let parts: Vec<&str> = pair.split('/').collect();
        if parts.len() != 2 { return None; }
        let base_input = parts[0].to_uppercase();
        let base = match base_input.as_str() {
            "WETH" => "ETH",
            "WBTC" => "BTC",
            other => other,
        };
        let quote = parts[1].to_uppercase();
        Some(format!("{}{}", base, quote))
    }

    

    async fn fetch_binance_book_ticker(client: &reqwest::Client, symbol: &str) -> Result<(Decimal, Decimal, Decimal)> {
        let url = format!("https://api.binance.com/api/v3/ticker/bookTicker?symbol={}", symbol);
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        let data: BinanceBookTicker = resp.json().await?;
        let bid = Decimal::from_str(&data.bid_price).unwrap_or_default();
        let ask = Decimal::from_str(&data.ask_price).unwrap_or_default();
        let last = if bid > Decimal::ZERO && ask > Decimal::ZERO { (bid + ask) / Decimal::from(2u8) } else { bid.max(ask) };
        Ok((bid, ask, last))
    }

    async fn fetch_binance_orderbook(client: &reqwest::Client, symbol: &str, depth: u32) -> Result<(Vec<OrderBookLevel>, Vec<OrderBookLevel>)> {
        let limit = match depth {
            d if d <= 5 => 5,
            d if d <= 10 => 10,
            d if d <= 20 => 20,
            d if d <= 50 => 50,
            d if d <= 100 => 100,
            _ => 100,
        };
        let url = format!("https://api.binance.com/api/v3/depth?symbol={}&limit={}", symbol, limit);
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        let data: BinanceDepth = resp.json().await?;
        let bids = data.bids.into_iter().take(limit as usize).filter_map(|arr| {
            let price = Decimal::from_str(&arr[0]).ok()?;
            let qty_dec = Decimal::from_str(&arr[1]).ok()?;
            let qty_u256 = decimal_to_u256(qty_dec);
            Some(OrderBookLevel { price, quantity: qty_u256 })
        }).collect();
        let asks = data.asks.into_iter().take(limit as usize).filter_map(|arr| {
            let price = Decimal::from_str(&arr[0]).ok()?;
            let qty_dec = Decimal::from_str(&arr[1]).ok()?;
            let qty_u256 = decimal_to_u256(qty_dec);
            Some(OrderBookLevel { price, quantity: qty_u256 })
        }).collect();
        Ok((bids, asks))
    }

    /// DexScreener에서 페어 문자열("WETH/USDC")을 기반으로 USD 가격을 조회하고 Decimal로 반환
    async fn fetch_dexscreener_price(client: &reqwest::Client, pair: &str) -> Result<Decimal> {
        // 간단히 기본/상대 토큰 심볼만 가져와 심볼 검색 API 사용
        let parts: Vec<&str> = pair.split('/').collect();
        if parts.len() != 2 { return Err(anyhow!("invalid pair")); }
        let base = parts[0].to_uppercase();
        // DexScreener symbol search
        let url = format!("https://api.dexscreener.com/latest/dex/search?q={}", base);
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        let body: DexScreenerResponse = resp.json().await?;
        if let Some(pairs) = body.pairs {
            for p in pairs {
                if let Some(price_str) = p.price_usd {
                    if let Ok(val) = Decimal::from_str(&price_str) { return Ok(val); }
                }
            }
        }
        Err(anyhow!("price not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    
    #[tokio::test]
    async fn test_exchange_monitor_creation() {
        let config = Arc::new(crate::config::Config::default());
        let monitor = ExchangeMonitor::new(config);
        
        assert!(!monitor.is_running());
    }
    
    #[tokio::test]
    async fn test_mock_data_generation() {
        let trading_pairs = vec!["WETH/USDC".to_string(), "WBTC/USDC".to_string()];
        
        let dex_data = ExchangeMonitor::generate_mock_dex_data("uniswap_v2", &trading_pairs, 1).await;
        assert!(dex_data.is_ok());
        
        let data = dex_data.unwrap();
        assert_eq!(data.len(), 2);
        
        let (price_data, orderbook_data) = &data[0];
        assert_eq!(price_data.exchange, "uniswap_v2");
        assert_eq!(price_data.symbol, "WETH/USDC");
        assert!(!orderbook_data.bids.is_empty());
        assert!(!orderbook_data.asks.is_empty());
    }
}