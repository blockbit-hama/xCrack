use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use tokio::time::{sleep, Duration, interval, Instant};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use chrono::Utc;

use crate::config::Config;
use crate::types::{PriceData, OrderBookSnapshot};
use alloy::primitives::U256;
use crate::strategies::MicroArbitrageStrategy;

/// 가격 피드 관리자
/// 
/// ExchangeMonitor로부터 실시간 가격 데이터를 수신하여
/// MicroArbitrageStrategy에 전달하고 데이터 품질을 관리합니다.
pub struct PriceFeedManager {
    config: Arc<Config>,
    is_running: Arc<AtomicBool>,
    
    // 데이터 수신 채널들
    price_receiver: Option<mpsc::UnboundedReceiver<PriceData>>,
    orderbook_receiver: Option<mpsc::UnboundedReceiver<OrderBookSnapshot>>,
    
    // 전략 참조
    micro_arbitrage_strategy: Option<Arc<MicroArbitrageStrategy>>,
    
    // 데이터 캐시 (최신 가격 정보)
    price_cache: Arc<RwLock<HashMap<String, HashMap<String, PriceData>>>>, // exchange -> symbol -> price
    orderbook_cache: Arc<RwLock<HashMap<String, HashMap<String, OrderBookSnapshot>>>>, // exchange -> symbol -> orderbook
    
    // 데이터 품질 관리
    data_quality_tracker: Arc<RwLock<DataQualityTracker>>,
    
    // 성능 통계
    stats: Arc<RwLock<FeedManagerStats>>,
}

#[derive(Debug, Clone)]
struct DataQualityTracker {
    // 거래소별 데이터 품질 점수 (0.0 ~ 1.0)
    exchange_quality_scores: HashMap<String, f64>,
    
    // 데이터 지연 추적 (밀리초)
    average_latencies: HashMap<String, u64>,
    
    // 데이터 누락 추적
    missing_data_counts: HashMap<String, u32>,
    
    // 스테일 데이터 감지 (마지막 업데이트로부터 경과 시간)
    stale_data_thresholds: HashMap<String, Duration>,
    
    // 가격 이상치 감지
    price_anomaly_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub struct FeedManagerStats {
    pub total_price_updates: u64,
    pub total_orderbook_updates: u64,
    pub processed_updates: u64,
    pub filtered_updates: u64,
    pub error_count: u32,
    pub average_processing_time_ms: f64,
    pub data_quality_score: f64,
    pub cache_hit_rate: f64,
    pub uptime_percentage: f64,
}

impl PriceFeedManager {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            price_receiver: None,
            orderbook_receiver: None,
            micro_arbitrage_strategy: None,
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            orderbook_cache: Arc::new(RwLock::new(HashMap::new())),
            data_quality_tracker: Arc::new(RwLock::new(DataQualityTracker {
                exchange_quality_scores: HashMap::new(),
                average_latencies: HashMap::new(),
                missing_data_counts: HashMap::new(),
                stale_data_thresholds: HashMap::new(),
                price_anomaly_counts: HashMap::new(),
            })),
            stats: Arc::new(RwLock::new(FeedManagerStats {
                total_price_updates: 0,
                total_orderbook_updates: 0,
                processed_updates: 0,
                filtered_updates: 0,
                error_count: 0,
                average_processing_time_ms: 0.0,
                data_quality_score: 1.0,
                cache_hit_rate: 0.0,
                uptime_percentage: 100.0,
            })),
        }
    }
    
    /// 가격 피드 매니저 시작
    pub async fn start(
        &mut self,
        price_receiver: mpsc::UnboundedReceiver<PriceData>,
        orderbook_receiver: mpsc::UnboundedReceiver<OrderBookSnapshot>,
        micro_arbitrage_strategy: Arc<MicroArbitrageStrategy>,
    ) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(anyhow!("PriceFeedManager is already running"));
        }
        
        self.price_receiver = Some(price_receiver);
        self.orderbook_receiver = Some(orderbook_receiver);
        self.micro_arbitrage_strategy = Some(micro_arbitrage_strategy);
        
        info!("📡 가격 피드 매니저 시작");
        
        self.is_running.store(true, Ordering::SeqCst);
        
        // 데이터 품질 추적기 초기화
        self.initialize_data_quality_tracker().await;
        
        // 가격 데이터 처리 태스크 시작
        self.start_price_data_processor().await?;
        
        // 오더북 데이터 처리 태스크 시작
        self.start_orderbook_data_processor().await?;
        
        // 데이터 품질 모니터링 태스크 시작
        self.start_data_quality_monitor().await;
        
        // 캐시 정리 태스크 시작
        self.start_cache_cleanup_task().await;
        
        // 통계 업데이트 태스크 시작
        self.start_stats_updater().await;
        
        info!("✅ 가격 피드 매니저 시작 완료");
        Ok(())
    }
    
    /// 가격 피드 매니저 중지
    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);
        
        // 모든 처리 중인 데이터 완료 대기
        sleep(Duration::from_millis(100)).await;
        
        info!("⏹️ 가격 피드 매니저 중지됨");
        Ok(())
    }
    
    /// 데이터 품질 추적기 초기화
    async fn initialize_data_quality_tracker(&self) {
        let mut tracker = self.data_quality_tracker.write().await;
        
        // 모든 거래소에 대해 초기값 설정
        for exchange_config in &self.config.strategies.micro_arbitrage.exchanges {
            if exchange_config.enabled {
                tracker.exchange_quality_scores.insert(exchange_config.name.clone(), 1.0);
                tracker.average_latencies.insert(exchange_config.name.clone(), 50);
                tracker.missing_data_counts.insert(exchange_config.name.clone(), 0);
                tracker.stale_data_thresholds.insert(exchange_config.name.clone(), Duration::from_secs(5));
                tracker.price_anomaly_counts.insert(exchange_config.name.clone(), 0);
            }
        }
    }
    
    /// 가격 데이터 처리 태스크 시작
    async fn start_price_data_processor(&mut self) -> Result<()> {
        let mut price_receiver = self.price_receiver.take()
            .ok_or_else(|| anyhow!("Price receiver not available"))?;
        
        let is_running = Arc::clone(&self.is_running);
        let price_cache = Arc::clone(&self.price_cache);
        let data_quality_tracker = Arc::clone(&self.data_quality_tracker);
        let stats = Arc::clone(&self.stats);
        let strategy = self.micro_arbitrage_strategy.as_ref().unwrap().clone();
        
        tokio::spawn(async move {
            info!("💰 가격 데이터 처리 태스크 시작");
            
            while is_running.load(Ordering::SeqCst) {
                match price_receiver.recv().await {
                    Some(price_data) => {
                        let processing_start = Instant::now();
                        
                        // 데이터 검증 및 품질 체크
                        match Self::validate_price_data(&price_data).await {
                            Ok(true) => {
                                // 캐시 업데이트
                                Self::update_price_cache(&price_cache, price_data.clone()).await;
                                
                                // 전략에 데이터 전달
                                if let Err(e) = strategy.update_price_data(price_data.clone()).await {
                                    error!("전략 가격 데이터 업데이트 실패: {}", e);
                                    Self::update_error_stats(&stats).await;
                                } else {
                                    // 성공 통계 업데이트
                                    let processing_time = processing_start.elapsed().as_millis() as f64;
                                    Self::update_processing_stats(&stats, processing_time, true).await;
                                }
                                
                                // 데이터 품질 추적 업데이트
                                Self::update_price_quality_tracking(&data_quality_tracker, &price_data).await;
                            }
                            Ok(false) => {
                                // 검증 실패 - 필터링됨
                                debug!("가격 데이터 필터링: {} - {}", price_data.exchange, price_data.symbol);
                                Self::update_filtering_stats(&stats).await;
                            }
                            Err(e) => {
                                error!("가격 데이터 검증 오류: {}", e);
                                Self::update_error_stats(&stats).await;
                            }
                        }
                    }
                    None => {
                        warn!("가격 데이터 채널이 닫혔습니다");
                        break;
                    }
                }
            }
            
            info!("💰 가격 데이터 처리 태스크 종료");
        });
        
        Ok(())
    }
    
    /// 오더북 데이터 처리 태스크 시작
    async fn start_orderbook_data_processor(&mut self) -> Result<()> {
        let mut orderbook_receiver = self.orderbook_receiver.take()
            .ok_or_else(|| anyhow!("Orderbook receiver not available"))?;
        
        let is_running = Arc::clone(&self.is_running);
        let orderbook_cache = Arc::clone(&self.orderbook_cache);
        let data_quality_tracker = Arc::clone(&self.data_quality_tracker);
        let stats = Arc::clone(&self.stats);
        let strategy = self.micro_arbitrage_strategy.as_ref().unwrap().clone();
        
        tokio::spawn(async move {
            info!("📚 오더북 데이터 처리 태스크 시작");
            
            while is_running.load(Ordering::SeqCst) {
                match orderbook_receiver.recv().await {
                    Some(orderbook_data) => {
                        let processing_start = Instant::now();
                        
                        // 데이터 검증 및 품질 체크
                        match Self::validate_orderbook_data(&orderbook_data).await {
                            Ok(true) => {
                                // 캐시 업데이트
                                Self::update_orderbook_cache(&orderbook_cache, orderbook_data.clone()).await;
                                
                                // 전략에 데이터 전달
                                if let Err(e) = strategy.update_orderbook_data(orderbook_data.clone()).await {
                                    error!("전략 오더북 데이터 업데이트 실패: {}", e);
                                    Self::update_error_stats(&stats).await;
                                } else {
                                    // 성공 통계 업데이트
                                    let processing_time = processing_start.elapsed().as_millis() as f64;
                                    Self::update_processing_stats(&stats, processing_time, false).await;
                                }
                                
                                // 데이터 품질 추적 업데이트
                                Self::update_orderbook_quality_tracking(&data_quality_tracker, &orderbook_data).await;
                            }
                            Ok(false) => {
                                // 검증 실패 - 필터링됨
                                debug!("오더북 데이터 필터링: {} - {}", orderbook_data.exchange, orderbook_data.symbol);
                                Self::update_filtering_stats(&stats).await;
                            }
                            Err(e) => {
                                error!("오더북 데이터 검증 오류: {}", e);
                                Self::update_error_stats(&stats).await;
                            }
                        }
                    }
                    None => {
                        warn!("오더북 데이터 채널이 닫혔습니다");
                        break;
                    }
                }
            }
            
            info!("📚 오더북 데이터 처리 태스크 종료");
        });
        
        Ok(())
    }
    
    /// 가격 데이터 검증
    async fn validate_price_data(price_data: &PriceData) -> Result<bool> {
        // 기본 유효성 검사
        if price_data.bid <= Decimal::ZERO || price_data.ask <= Decimal::ZERO {
            return Ok(false);
        }
        
        // 스프레드 검사 (bid >= ask는 비정상)
        if price_data.bid >= price_data.ask {
            return Ok(false);
        }
        
        // 시간 검사 (너무 오래된 데이터 제외)
        let age = Utc::now() - price_data.timestamp;
        if age.num_seconds() > 10 {
            return Ok(false);
        }
        
        // 가격 범위 검사 (너무 극단적인 값 제외)
        let price_f64 = price_data.last_price.to_f64().unwrap_or(0.0);
        if price_f64 < 0.001 || price_f64 > 1_000_000.0 {
            return Ok(false);
        }
        
        // 스프레드 비율 검사 (50% 이상 스프레드는 비정상)
        let spread_ratio = ((price_data.ask - price_data.bid) / price_data.last_price).to_f64().unwrap_or(0.0);
        if spread_ratio > 0.5 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// 오더북 데이터 검증
    async fn validate_orderbook_data(orderbook_data: &OrderBookSnapshot) -> Result<bool> {
        // 기본 유효성 검사
        if orderbook_data.bids.is_empty() || orderbook_data.asks.is_empty() {
            return Ok(false);
        }
        
        // 시간 검사
        let age = Utc::now() - orderbook_data.timestamp;
        if age.num_seconds() > 10 {
            return Ok(false);
        }
        
        // 가격 순서 검사 (bid는 내림차순, ask는 오름차순이어야 함)
        let mut prev_bid_price = None;
        for bid in &orderbook_data.bids {
            if let Some(prev_price) = prev_bid_price {
                if bid.price > prev_price {
                    return Ok(false); // bid는 내림차순이어야 함
                }
            }
            prev_bid_price = Some(bid.price);
            
            if bid.price <= Decimal::ZERO || bid.quantity <= U256::ZERO {
                return Ok(false);
            }
        }
        
        let mut prev_ask_price = None;
        for ask in &orderbook_data.asks {
            if let Some(prev_price) = prev_ask_price {
                if ask.price < prev_price {
                    return Ok(false); // ask는 오름차순이어야 함
                }
            }
            prev_ask_price = Some(ask.price);
            
            if ask.price <= Decimal::ZERO || ask.quantity <= U256::ZERO {
                return Ok(false);
            }
        }
        
        // 최고 bid가 최저 ask보다 낮아야 함
        if let (Some(best_bid), Some(best_ask)) = (orderbook_data.bids.first(), orderbook_data.asks.first()) {
            if best_bid.price >= best_ask.price {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// 가격 캐시 업데이트
    async fn update_price_cache(
        cache: &Arc<RwLock<HashMap<String, HashMap<String, PriceData>>>>,
        price_data: PriceData,
    ) {
        let mut cache_guard = cache.write().await;
        let exchange_cache = cache_guard.entry(price_data.exchange.clone()).or_insert_with(HashMap::new);
        exchange_cache.insert(price_data.symbol.clone(), price_data);
    }
    
    /// 오더북 캐시 업데이트
    async fn update_orderbook_cache(
        cache: &Arc<RwLock<HashMap<String, HashMap<String, OrderBookSnapshot>>>>,
        orderbook_data: OrderBookSnapshot,
    ) {
        let mut cache_guard = cache.write().await;
        let exchange_cache = cache_guard.entry(orderbook_data.exchange.clone()).or_insert_with(HashMap::new);
        exchange_cache.insert(orderbook_data.symbol.clone(), orderbook_data);
    }
    
    /// 가격 품질 추적 업데이트
    async fn update_price_quality_tracking(
        tracker: &Arc<RwLock<DataQualityTracker>>,
        price_data: &PriceData,
    ) {
        let mut tracker_guard = tracker.write().await;
        
        // 지연시간 계산
        let latency = (Utc::now() - price_data.timestamp).num_milliseconds() as u64;
        
        // 평균 지연시간 업데이트
        if let Some(avg_latency) = tracker_guard.average_latencies.get_mut(&price_data.exchange) {
            *avg_latency = (*avg_latency + latency) / 2; // 단순 이동 평균
        }
        
        // 품질 점수 업데이트 (지연시간 기반)
        let quality_score = if latency < 100 {
            1.0
        } else if latency < 500 {
            0.8
        } else if latency < 1000 {
            0.6
        } else {
            0.4
        };
        
        tracker_guard.exchange_quality_scores.insert(price_data.exchange.clone(), quality_score);
    }
    
    /// 오더북 품질 추적 업데이트
    async fn update_orderbook_quality_tracking(
        tracker: &Arc<RwLock<DataQualityTracker>>,
        orderbook_data: &OrderBookSnapshot,
    ) {
        let mut tracker_guard = tracker.write().await;
        
        // 지연시간 계산
        let latency = (Utc::now() - orderbook_data.timestamp).num_milliseconds() as u64;
        
        // 평균 지연시간 업데이트
        if let Some(avg_latency) = tracker_guard.average_latencies.get_mut(&orderbook_data.exchange) {
            *avg_latency = (*avg_latency + latency) / 2;
        }
        
        // 오더북 깊이를 고려한 품질 점수
        let depth_score = if orderbook_data.bids.len() >= 10 && orderbook_data.asks.len() >= 10 {
            1.0
        } else if orderbook_data.bids.len() >= 5 && orderbook_data.asks.len() >= 5 {
            0.8
        } else {
            0.6
        };
        
        let latency_score = if latency < 100 {
            1.0
        } else if latency < 500 {
            0.8
        } else {
            0.6
        };
        
        let combined_score = (depth_score + latency_score) / 2.0;
        tracker_guard.exchange_quality_scores.insert(orderbook_data.exchange.clone(), combined_score);
    }
    
    /// 처리 통계 업데이트
    async fn update_processing_stats(
        stats: &Arc<RwLock<FeedManagerStats>>,
        processing_time_ms: f64,
        is_price_data: bool,
    ) {
        let mut stats_guard = stats.write().await;
        
        if is_price_data {
            stats_guard.total_price_updates += 1;
        } else {
            stats_guard.total_orderbook_updates += 1;
        }
        
        stats_guard.processed_updates += 1;
        
        // 평균 처리 시간 업데이트
        stats_guard.average_processing_time_ms = 
            (stats_guard.average_processing_time_ms * (stats_guard.processed_updates - 1) as f64 + processing_time_ms) 
            / stats_guard.processed_updates as f64;
    }
    
    /// 필터링 통계 업데이트
    async fn update_filtering_stats(stats: &Arc<RwLock<FeedManagerStats>>) {
        let mut stats_guard = stats.write().await;
        stats_guard.filtered_updates += 1;
    }
    
    /// 오류 통계 업데이트
    async fn update_error_stats(stats: &Arc<RwLock<FeedManagerStats>>) {
        let mut stats_guard = stats.write().await;
        stats_guard.error_count += 1;
    }
    
    /// 데이터 품질 모니터 시작
    async fn start_data_quality_monitor(&self) {
        let is_running = Arc::clone(&self.is_running);
        let data_quality_tracker = Arc::clone(&self.data_quality_tracker);
        let stats = Arc::clone(&self.stats);
        
        tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(30)); // 30초마다 모니터링
            
            while is_running.load(Ordering::SeqCst) {
                monitor_interval.tick().await;
                
                let tracker = data_quality_tracker.read().await;
                
                // 전체 품질 점수 계산
                let total_score = if !tracker.exchange_quality_scores.is_empty() {
                    tracker.exchange_quality_scores.values().sum::<f64>() / tracker.exchange_quality_scores.len() as f64
                } else {
                    0.0
                };
                
                // 평균 지연시간 계산
                let avg_latency = if !tracker.average_latencies.is_empty() {
                    tracker.average_latencies.values().sum::<u64>() / tracker.average_latencies.len() as u64
                } else {
                    0
                };
                
                drop(tracker);
                
                // 통계 업데이트
                let mut stats_guard = stats.write().await;
                stats_guard.data_quality_score = total_score;
                
                debug!("📈 데이터 품질 모니터링 - 품질점수: {:.2}, 평균지연: {}ms", 
                       total_score, avg_latency);
                
                // 품질 경고
                if total_score < 0.7 {
                    warn!("⚠️ 데이터 품질 저하 감지: {:.2}", total_score);
                }
            }
        });
    }
    
    /// 캐시 정리 태스크 시작
    async fn start_cache_cleanup_task(&self) {
        let is_running = Arc::clone(&self.is_running);
        let price_cache = Arc::clone(&self.price_cache);
        let orderbook_cache = Arc::clone(&self.orderbook_cache);
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60)); // 1분마다 정리
            
            while is_running.load(Ordering::SeqCst) {
                cleanup_interval.tick().await;
                
                let now = Utc::now();
                let cutoff_time = now - chrono::Duration::seconds(300); // 5분 이상 된 데이터 제거
                
                // 가격 캐시 정리
                {
                    let mut cache = price_cache.write().await;
                    for exchange_cache in cache.values_mut() {
                        exchange_cache.retain(|_, price_data| price_data.timestamp > cutoff_time);
                    }
                }
                
                // 오더북 캐시 정리
                {
                    let mut cache = orderbook_cache.write().await;
                    for exchange_cache in cache.values_mut() {
                        exchange_cache.retain(|_, orderbook_data| orderbook_data.timestamp > cutoff_time);
                    }
                }
                
                debug!("🧹 캐시 정리 완료");
            }
        });
    }
    
    /// 통계 업데이트 태스크 시작
    async fn start_stats_updater(&self) {
        let is_running = Arc::clone(&self.is_running);
        let stats = Arc::clone(&self.stats);
        
        tokio::spawn(async move {
            let mut update_interval = interval(Duration::from_secs(10)); // 10초마다 업데이트
            
            while is_running.load(Ordering::SeqCst) {
                update_interval.tick().await;
                
                let mut stats_guard = stats.write().await;
                
                // 캐시 히트율 계산 (간단한 예시)
                let total_requests = stats_guard.processed_updates + stats_guard.filtered_updates;
                stats_guard.cache_hit_rate = if total_requests > 0 {
                    stats_guard.processed_updates as f64 / total_requests as f64
                } else {
                    0.0
                };
                
                // 업타임 계산 (오류율 기반)
                let total_operations = stats_guard.processed_updates + stats_guard.error_count as u64;
                stats_guard.uptime_percentage = if total_operations > 0 {
                    (stats_guard.processed_updates as f64 / total_operations as f64) * 100.0
                } else {
                    100.0
                };
                
                debug!("📊 피드 매니저 통계 - 처리: {}, 필터링: {}, 오류: {}, 캐시히트율: {:.2}%", 
                       stats_guard.processed_updates, 
                       stats_guard.filtered_updates, 
                       stats_guard.error_count,
                       stats_guard.cache_hit_rate * 100.0);
            }
        });
    }
    
    /// 현재 가격 데이터 조회
    pub async fn get_latest_price(&self, exchange: &str, symbol: &str) -> Option<PriceData> {
        let cache = self.price_cache.read().await;
        cache.get(exchange)?.get(symbol).cloned()
    }
    
    /// 현재 오더북 데이터 조회
    pub async fn get_latest_orderbook(&self, exchange: &str, symbol: &str) -> Option<OrderBookSnapshot> {
        let cache = self.orderbook_cache.read().await;
        cache.get(exchange)?.get(symbol).cloned()
    }
    
    /// 데이터 품질 점수 조회
    pub async fn get_data_quality_score(&self, exchange: &str) -> Option<f64> {
        let tracker = self.data_quality_tracker.read().await;
        tracker.exchange_quality_scores.get(exchange).copied()
    }
    
    /// 통계 조회
    pub async fn get_stats(&self) -> FeedManagerStats {
        self.stats.read().await.clone()
    }
    
    /// 실행 중인지 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PriceData;
    use tokio::sync::mpsc;
    use rust_decimal::Decimal;
    
    #[tokio::test]
    async fn test_price_feed_manager_creation() {
        let config = Arc::new(crate::config::Config::default());
        let manager = PriceFeedManager::new(config);
        
        assert!(!manager.is_running());
    }
    
    #[tokio::test]
    async fn test_price_data_validation() {
        // 유효한 데이터
        let valid_price_data = PriceData {
            symbol: "WETH/USDC".to_string(),
            exchange: "uniswap_v2".to_string(),
            bid: Decimal::from_f64_retain(1999.0).unwrap(),
            ask: Decimal::from_f64_retain(2001.0).unwrap(),
            last_price: Decimal::from_f64_retain(2000.0).unwrap(),
            volume_24h: U256::from(1000000),
            timestamp: Utc::now(),
            sequence: 1,
        };
        
        let result = PriceFeedManager::validate_price_data(&valid_price_data).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // 무효한 데이터 (bid >= ask)
        let invalid_price_data = PriceData {
            symbol: "WETH/USDC".to_string(),
            exchange: "uniswap_v2".to_string(),
            bid: Decimal::from_f64_retain(2001.0).unwrap(),
            ask: Decimal::from_f64_retain(1999.0).unwrap(),
            last_price: Decimal::from_f64_retain(2000.0).unwrap(),
            volume_24h: U256::from(1000000),
            timestamp: Utc::now(),
            sequence: 1,
        };
        
        let result = PriceFeedManager::validate_price_data(&invalid_price_data).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}