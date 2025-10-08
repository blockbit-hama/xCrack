use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug, warn, error};
use tokio::time::{sleep, Duration, interval, Instant};
use rust_decimal::Decimal;
use chrono::Utc;

use crate::config::Config;
use crate::types::{PriceData, OrderBookSnapshot, OrderBookLevel};
use crate::strategies::MicroArbitrageStrategy;
use crate::exchange::{ExchangeClient, ExchangeClientFactory};
use ethers::types::U256;

/// 실시간 스캔 스케줄러
/// 
/// 거래소 가격 피드를 구독하고 마이크로 아비트래지 전략에 
/// 실시간으로 스캔 및 실행 명령을 보내는 스케줄러
#[derive(Debug)]
pub struct RealTimeScheduler {
    config: Arc<Config>,
    is_running: Arc<AtomicBool>,
    
    // 전략 참조
    micro_arbitrage_strategy: Option<Arc<MicroArbitrageStrategy>>,
    
    // 거래소 클라이언트들
    exchange_clients: Vec<Arc<dyn ExchangeClient>>,
    
    // 스캔 설정
    scan_interval_ms: u64,
    price_update_interval_ms: u64,
    
    // 데이터 전송 채널
    price_sender: Option<mpsc::UnboundedSender<PriceData>>,
    orderbook_sender: Option<mpsc::UnboundedSender<OrderBookSnapshot>>,
    
    // 통계
    stats: Arc<RwLock<SchedulerStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    pub total_scans: u64,
    pub successful_scans: u64,
    pub failed_scans: u64,
    pub opportunities_found: u64,
    pub executions_attempted: u64,
    pub successful_executions: u64,
    pub avg_scan_time_ms: f64,
    pub last_scan_time: Option<chrono::DateTime<Utc>>,
    pub scan_rate_per_minute: f64,
}

impl RealTimeScheduler {
    pub fn new(config: Arc<Config>) -> Self {
        // Fallback if scan_interval_ms is not present in config: reuse price_update_interval or a default
        let scan_interval_ms = config
            .strategies
            .micro_arbitrage
            .price_update_interval_ms
            .saturating_mul(2)
            .max(10);
        let price_update_interval_ms = std::cmp::min(scan_interval_ms / 4, 250); // 최대 4Hz 가격 업데이트
        
        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            micro_arbitrage_strategy: None,
            exchange_clients: Vec::new(),
            scan_interval_ms,
            price_update_interval_ms,
            price_sender: None,
            orderbook_sender: None,
            stats: Arc::new(RwLock::new(SchedulerStats::default())),
        }
    }
    
    /// 스케줄러 시작
    pub async fn start(
        &mut self,
        micro_arbitrage_strategy: Arc<MicroArbitrageStrategy>,
        price_sender: mpsc::UnboundedSender<PriceData>,
        orderbook_sender: mpsc::UnboundedSender<OrderBookSnapshot>,
    ) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(anyhow!("RealTimeScheduler is already running"));
        }
        
        info!("⏰ 실시간 스캔 스케줄러 시작");
        info!("  📊 스캔 간격: {}ms", self.scan_interval_ms);
        info!("  📈 가격 업데이트 간격: {}ms", self.price_update_interval_ms);
        
        self.micro_arbitrage_strategy = Some(micro_arbitrage_strategy);
        self.price_sender = Some(price_sender);
        self.orderbook_sender = Some(orderbook_sender);
        
        // 거래소 클라이언트들 초기화
        self.initialize_exchange_clients().await?;
        
        self.is_running.store(true, Ordering::SeqCst);
        
        // 실시간 가격 피드 태스크 시작
        self.start_price_feed_tasks().await?;
        
        // 스캔 스케줄러 태스크 시작
        self.start_scan_scheduler_task().await?;
        
        // 통계 업데이트 태스크 시작
        self.start_stats_update_task().await;
        
        info!("✅ 실시간 스캔 스케줄러 시작 완료");
        Ok(())
    }
    
    /// 스케줄러 중지
    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);
        
        // 모든 태스크 완료 대기
        sleep(Duration::from_millis(200)).await;
        
        info!("⏹️ 실시간 스캔 스케줄러 중지됨");
        Ok(())
    }
    
    /// 거래소 클라이언트들 초기화
    async fn initialize_exchange_clients(&mut self) -> Result<()> {
        info!("🔗 거래소 클라이언트 초기화 중...");
        
        for exchange_config in &self.config.strategies.micro_arbitrage.exchanges {
            if !exchange_config.enabled {
                continue;
            }
            
            let client = match exchange_config.name.to_lowercase().as_str() {
                "binance" | "mock_binance" => {
                    let api_key = std::env::var("BINANCE_API_KEY").unwrap_or_else(|_| "mock_key".to_string());
                    let secret_key = std::env::var("BINANCE_SECRET_KEY").unwrap_or_else(|_| "mock_secret".to_string());
                    ExchangeClientFactory::create_binance_client(api_key, secret_key)
                }
                "coinbase" | "mock_coinbase" => {
                    let api_key = std::env::var("COINBASE_API_KEY").unwrap_or_else(|_| "mock_key".to_string());
                    let secret_key = std::env::var("COINBASE_SECRET_KEY").unwrap_or_else(|_| "mock_secret".to_string());
                    let passphrase = std::env::var("COINBASE_PASSPHRASE").unwrap_or_else(|_| "mock_passphrase".to_string());
                    ExchangeClientFactory::create_coinbase_client(api_key, secret_key, passphrase)
                }
                _ => {
                    warn!("⚠️ 지원되지 않는 거래소: {}, Mock 클라이언트 사용", exchange_config.name);
                    ExchangeClientFactory::create_binance_client("mock_key".to_string(), "mock_secret".to_string())
                }
            };
            
            self.exchange_clients.push(client);
            info!("  ✅ {} 클라이언트 초기화 완료", exchange_config.name);
        }
        
        info!("🔗 총 {}개 거래소 클라이언트 초기화 완료", self.exchange_clients.len());
        Ok(())
    }
    
    /// 실시간 가격 피드 태스크들 시작
    async fn start_price_feed_tasks(&self) -> Result<()> {
        let trading_pairs = self.config.strategies.micro_arbitrage.trading_pairs.clone();
        
        for client in &self.exchange_clients {
            let client_clone = Arc::clone(client);
            let is_running = Arc::clone(&self.is_running);
            let price_sender = self.price_sender.as_ref().unwrap().clone();
            let orderbook_sender = self.orderbook_sender.as_ref().unwrap().clone();
            let trading_pairs_clone = trading_pairs.clone();
            let price_update_interval = self.price_update_interval_ms;
            
            // 각 거래소별 가격 피드 태스크
            tokio::spawn(async move {
                let exchange_name = client_clone.name().to_string();
                info!("📡 {} 가격 피드 태스크 시작", exchange_name);
                
                let mut price_interval = interval(Duration::from_millis(price_update_interval));
                
                while is_running.load(Ordering::SeqCst) {
                    price_interval.tick().await;
                    
                    for symbol in &trading_pairs_clone {
                        // 가격 데이터 수집
                        match Self::collect_price_data(&client_clone, &exchange_name, symbol).await {
                            Ok(price_data) => {
                                if let Err(e) = price_sender.send(price_data) {
                                    error!("가격 데이터 전송 실패 ({}): {}", exchange_name, e);
                                }
                            }
                            Err(e) => {
                                debug!("가격 데이터 수집 실패 ({} - {}): {}", exchange_name, symbol, e);
                            }
                        }
                        
                        // 오더북 데이터 수집 (Mock 환경에서는 가격 기반으로 생성)
                        if let Ok(orderbook) = Self::collect_orderbook_data(&client_clone, &exchange_name, symbol).await {
                            if let Err(e) = orderbook_sender.send(orderbook) {
                                error!("오더북 데이터 전송 실패 ({}): {}", exchange_name, e);
                            }
                        }
                        
                        // 과도한 API 호출 방지
                        if trading_pairs_clone.len() > 1 {
                            sleep(Duration::from_millis(10)).await;
                        }
                    }
                }
                
                info!("📡 {} 가격 피드 태스크 종료", exchange_name);
            });
        }
        
        Ok(())
    }
    
    /// 스캔 스케줄러 태스크 시작
    async fn start_scan_scheduler_task(&self) -> Result<()> {
        let is_running = Arc::clone(&self.is_running);
        let strategy = self.micro_arbitrage_strategy.as_ref().unwrap().clone();
        let stats = Arc::clone(&self.stats);
        let scan_interval_ms = self.scan_interval_ms;
        
        tokio::spawn(async move {
            info!("⚡ 스캔 스케줄러 태스크 시작 ({}ms 간격)", scan_interval_ms);
            
            let mut scan_interval = interval(Duration::from_millis(scan_interval_ms));
            
            while is_running.load(Ordering::SeqCst) {
                scan_interval.tick().await;
                
                let scan_start = Instant::now();
                
                // 마이크로 아비트래지 스캔 및 실행
                match strategy.scan_and_execute().await {
                    Ok(executed_count) => {
                        let scan_time = scan_start.elapsed();
                        
                        // 통계 업데이트
                        Self::update_scan_stats(&stats, scan_time, true, executed_count).await;
                        
                        if executed_count > 0 {
                            debug!("⚡ 스캔 완료: {}개 기회 실행 ({:.2}ms)", executed_count, scan_time.as_millis());
                        }
                    }
                    Err(e) => {
                        let scan_time = scan_start.elapsed();
                        error!("💥 스캔 실행 실패: {} ({:.2}ms)", e, scan_time.as_millis());
                        
                        // 실패 통계 업데이트
                        Self::update_scan_stats(&stats, scan_time, false, 0).await;
                    }
                }
            }
            
            info!("⚡ 스캔 스케줄러 태스크 종료");
        });
        
        Ok(())
    }
    
    /// 가격 데이터 수집
    async fn collect_price_data(
        client: &Arc<dyn ExchangeClient>,
        exchange_name: &str,
        symbol: &str,
    ) -> Result<PriceData> {
        let current_price = client.get_current_price(symbol).await?;
        
        // Mock 환경에서는 bid/ask 스프레드 시뮬레이션
        let spread_percentage = 0.001; // 0.1% 스프레드
        let spread = current_price * Decimal::from_f64_retain(spread_percentage).unwrap_or_default();
        
        let bid = current_price - spread / Decimal::from(2);
        let ask = current_price + spread / Decimal::from(2);
        
        // 24시간 거래량 시뮬레이션
        let base_volume = match symbol.to_uppercase().as_str() {
            "ETHUSDT" | "ETH-USD" => 100000,
            "BTCUSDT" | "BTC-USD" => 50000,
            _ => 10000,
        };
        let volume_variance = fastrand::u32(8000..12000) as f64 / 10000.0; // 0.8 ~ 1.2
        let volume_24h = U256::from((base_volume as f64 * volume_variance) as u64);
        
        Ok(PriceData {
            symbol: symbol.to_string(),
            exchange: exchange_name.to_string(),
            bid,
            ask,
            last_price: current_price,
            volume_24h,
            timestamp: Utc::now(),
            sequence: fastrand::u64(1..1000000),
        })
    }
    
    /// 오더북 데이터 수집 (Mock 시뮬레이션)
    async fn collect_orderbook_data(
        client: &Arc<dyn ExchangeClient>,
        exchange_name: &str,
        symbol: &str,
    ) -> Result<OrderBookSnapshot> {
        let current_price = client.get_current_price(symbol).await?;
        
        // Mock 오더북 생성 (10 레벨)
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        
        let base_quantity = U256::from(fastrand::u32(50..200));
        
        // Bid 레벨들 (가격 내림차순)
        for i in 1..=10 {
            let price_offset = Decimal::from_f64_retain(i as f64 * 0.0005).unwrap_or_default(); // 0.05% 간격
            let price = current_price - price_offset;
            let quantity = base_quantity + U256::from(fastrand::u32(0..100));
            
            bids.push(OrderBookLevel { price, quantity });
        }
        
        // Ask 레벨들 (가격 오름차순)
        for i in 1..=10 {
            let price_offset = Decimal::from_f64_retain(i as f64 * 0.0005).unwrap_or_default();
            let price = current_price + price_offset;
            let quantity = base_quantity + U256::from(fastrand::u32(0..100));
            
            asks.push(OrderBookLevel { price, quantity });
        }
        
        Ok(OrderBookSnapshot {
            symbol: symbol.to_string(),
            exchange: exchange_name.to_string(),
            bids,
            asks,
            timestamp: Utc::now(),
            sequence: fastrand::u64(1..1000000),
        })
    }
    
    /// 스캔 통계 업데이트
    async fn update_scan_stats(
        stats: &Arc<RwLock<SchedulerStats>>,
        scan_time: Duration,
        success: bool,
        executed_count: usize,
    ) {
        let mut stats_guard = stats.write().await;
        
        stats_guard.total_scans += 1;
        
        if success {
            stats_guard.successful_scans += 1;
            stats_guard.executions_attempted += executed_count as u64;
            if executed_count > 0 {
                stats_guard.opportunities_found += executed_count as u64;
                stats_guard.successful_executions += executed_count as u64; // Mock에서는 100% 성공률 가정
            }
        } else {
            stats_guard.failed_scans += 1;
        }
        
        // 평균 스캔 시간 업데이트
        let scan_time_ms = scan_time.as_millis() as f64;
        if stats_guard.total_scans == 1 {
            stats_guard.avg_scan_time_ms = scan_time_ms;
        } else {
            stats_guard.avg_scan_time_ms = 
                (stats_guard.avg_scan_time_ms * (stats_guard.total_scans - 1) as f64 + scan_time_ms) 
                / stats_guard.total_scans as f64;
        }
        
        stats_guard.last_scan_time = Some(Utc::now());
    }
    
    /// 통계 업데이트 태스크 시작
    async fn start_stats_update_task(&self) {
        let is_running = Arc::clone(&self.is_running);
        let stats = Arc::clone(&self.stats);
        
        tokio::spawn(async move {
            let mut update_interval = interval(Duration::from_secs(60)); // 1분마다 업데이트
            let mut last_scan_count = 0u64;
            
            while is_running.load(Ordering::SeqCst) {
                update_interval.tick().await;
                
                let mut stats_guard = stats.write().await;
                
                // 분당 스캔 비율 계산
                let current_scan_count = stats_guard.total_scans;
                let scans_in_last_minute = current_scan_count - last_scan_count;
                stats_guard.scan_rate_per_minute = scans_in_last_minute as f64;
                last_scan_count = current_scan_count;
                
                debug!("📈 스케줄러 통계 - 총 스캔: {}, 성공: {}, 실패: {}, 분당스캔: {:.1}", 
                       stats_guard.total_scans,
                       stats_guard.successful_scans, 
                       stats_guard.failed_scans,
                       stats_guard.scan_rate_per_minute);
            }
        });
    }
    
    /// 현재 실행 중인지 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// 통계 조회
    pub async fn get_stats(&self) -> SchedulerStats {
        self.stats.read().await.clone()
    }
    
    /// 스캔 간격 동적 조정
    pub async fn adjust_scan_interval(&mut self, new_interval_ms: u64) {
        if new_interval_ms >= 10 && new_interval_ms <= 5000 { // 10ms ~ 5초 제한
            self.scan_interval_ms = new_interval_ms;
            info!("⏰ 스캔 간격 조정: {}ms", new_interval_ms);
        } else {
            warn!("⚠️ 유효하지 않은 스캔 간격: {}ms (10-5000ms 범위)", new_interval_ms);
        }
    }
    
    /// 수동 스캔 트리거
    pub async fn trigger_manual_scan(&self) -> Result<usize> {
        if let Some(strategy) = &self.micro_arbitrage_strategy {
            debug!("🔍 수동 스캔 트리거");
            let executed_count = strategy.scan_and_execute().await?;
            
            // 통계 업데이트
            let scan_start = Instant::now();
            Self::update_scan_stats(&self.stats, scan_start.elapsed(), true, executed_count).await;
            
            Ok(executed_count)
        } else {
            Err(anyhow!("마이크로 아비트래지 전략이 초기화되지 않음"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_real_time_scheduler_creation() {
        let config = Arc::new(Config::default());
        let scheduler = RealTimeScheduler::new(config);
        
        assert!(!scheduler.is_running());
        assert_eq!(scheduler.exchange_clients.len(), 0);
    }
    
    #[tokio::test]
    async fn test_stats_initialization() {
        let config = Arc::new(Config::default());
        let scheduler = RealTimeScheduler::new(config);
        
        let stats = scheduler.get_stats().await;
        assert_eq!(stats.total_scans, 0);
        assert_eq!(stats.successful_scans, 0);
        assert_eq!(stats.failed_scans, 0);
    }
}