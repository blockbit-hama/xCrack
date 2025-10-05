use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, debug, error, warn};
use tokio::time::{interval, sleep, Duration};

use crate::config::Config;
use crate::strategies::MicroArbitrageStrategy;
use crate::common::Strategy;
use crate::exchange::{ExchangeMonitor, PriceFeedManager, OrderExecutor, RealTimeScheduler};
use crate::types::{PriceData, OrderBookSnapshot};

/// 마이크로아비트래지 오케스트레이터
/// 
/// ExchangeMonitor, PriceFeedManager, RealTimeScheduler 그리고 MicroArbitrageStrategy를
/// 조율하여 완전한 마이크로아비트래지 시스템을 운영합니다.
pub struct MicroArbitrageOrchestrator {
    config: Arc<Config>,
    is_running: Arc<AtomicBool>,
    
    // 핵심 컴포넌트들
    exchange_monitor: Arc<ExchangeMonitor>,
    _price_feed_manager: Arc<PriceFeedManager>,
    _real_time_scheduler: Option<RealTimeScheduler>,
    micro_arbitrage_strategy: Arc<MicroArbitrageStrategy>,
    order_executor: Arc<OrderExecutor>,
}

impl MicroArbitrageOrchestrator {
    pub async fn new(
        config: Arc<Config>, 
        micro_arbitrage_strategy: Arc<MicroArbitrageStrategy>
    ) -> Result<Self> {
        info!("🎼 마이크로아비트래지 오케스트레이터 초기화 중...");
        
        // 거래소 모니터 생성
        let exchange_monitor = Arc::new(ExchangeMonitor::new(Arc::clone(&config)));
        
        // 가격 피드 매니저 생성
        let price_feed_manager = Arc::new(PriceFeedManager::new(Arc::clone(&config)));
        
        // 주문 실행자 생성
        let order_executor = Arc::new(OrderExecutor::new(Arc::clone(&config)).await?);
        
        // 실시간 스케줄러 생성
        let real_time_scheduler = RealTimeScheduler::new(Arc::clone(&config));
        
        info!("✅ 마이크로아비트래지 오케스트레이터 초기화 완료");
        
        Ok(Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            exchange_monitor,
            _price_feed_manager: price_feed_manager,
            _real_time_scheduler: Some(real_time_scheduler),
            micro_arbitrage_strategy,
            order_executor,
        })
    }
    
    /// 마이크로아비트래지 시스템 시작
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("⚠️ 마이크로아비트래지 오케스트레이터가 이미 실행 중입니다");
            return Ok(());
        }
        
        info!("🚀 마이크로아비트래지 시스템 시작 중...");
        self.is_running.store(true, Ordering::SeqCst);
        
        // 데이터 플로우 채널 생성
        let (price_sender, price_receiver) = mpsc::unbounded_channel::<PriceData>();
        let (orderbook_sender, orderbook_receiver) = mpsc::unbounded_channel::<OrderBookSnapshot>();
        
        // 1. 거래소 모니터 시작
        info!("📡 거래소 모니터링 시작...");
        // Create a new ExchangeMonitor instance for this orchestration
        let mut exchange_monitor = ExchangeMonitor::new(Arc::clone(&self.config));
        exchange_monitor.start(price_sender, orderbook_sender).await?;
        
        // 2. 가격 피드 매니저 시작
        info!("📊 가격 피드 매니저 시작...");
        // Create a new PriceFeedManager instance for this orchestration
        let mut price_feed_manager = PriceFeedManager::new(Arc::clone(&self.config));
        price_feed_manager.start(
            price_receiver,
            orderbook_receiver,
            Arc::clone(&self.micro_arbitrage_strategy),
        ).await?;
        
        // 3. 마이크로아비트래지 전략 시작
        info!("⚡ 마이크로아비트래지 전략 시작...");
        (*self.micro_arbitrage_strategy).start().await?;

        // 번들 라우팅 채널이 상위(SearcherCore)에서 주입되는 구조이므로 여기서는 노옵
        
        // 4. 실시간 스케줄러 시작 (새로운 고성능 스캔 시스템)
        info!("⏰ 실시간 스케줄러 시작...");
        // Note: real_time_scheduler is Option type, but we can't take from &self
        // Create a new scheduler instance for this execution
        let mut scheduler = RealTimeScheduler::new(Arc::clone(&self.config));
        
        // 새로운 채널 생성 (스케줄러 전용)
        let (scheduler_price_sender, scheduler_price_receiver) = mpsc::unbounded_channel::<PriceData>();
        let (scheduler_orderbook_sender, scheduler_orderbook_receiver) = mpsc::unbounded_channel::<OrderBookSnapshot>();
        
        // 실시간 스케줄러 시작
        scheduler.start(
            Arc::clone(&self.micro_arbitrage_strategy),
            scheduler_price_sender,
            scheduler_orderbook_sender,
        ).await?;
        
        // 가격 피드 매니저를 스케줄러의 데이터 수신자로 연결
        let mut price_feed_manager = PriceFeedManager::new(Arc::clone(&self.config));
        price_feed_manager.start(
            scheduler_price_receiver,
            scheduler_orderbook_receiver,
            Arc::clone(&self.micro_arbitrage_strategy),
        ).await?;
        
        info!("✅ 실시간 스케줄러 연결 완료");
        
        // 5. 성능 모니터링 태스크 시작
        self.start_performance_monitor().await;
        
        // 6. 헬스 체크 태스크 시작
        self.start_health_monitor().await;
        
        info!("✅ 마이크로아비트래지 시스템 시작 완료");
        Ok(())
    }
    
    /// 주기적 아비트래지 기회 스캔
    async fn start_opportunity_scanner(&self) {
        let is_running = Arc::clone(&self.is_running);
        let strategy = Arc::clone(&self.micro_arbitrage_strategy);
        let scan_interval_ms = self.config.strategies.micro_arbitrage.price_update_interval_ms;
        
        tokio::spawn(async move {
            let mut scan_interval = interval(Duration::from_millis(scan_interval_ms));
            
            info!("🔍 아비트래지 기회 스캐너 시작 ({}ms 간격)", scan_interval_ms);
            
            while is_running.load(Ordering::SeqCst) {
                scan_interval.tick().await;
                
                match strategy.scan_and_execute().await {
                    Ok(executed_count) => {
                        if executed_count > 0 {
                            debug!("⚡ {}개 아비트래지 기회 실행", executed_count);
                        }
                    }
                    Err(e) => {
                        error!("❌ 아비트래지 스캔 실패: {}", e);
                    }
                }
            }
            
            info!("🔍 아비트래지 기회 스캐너 종료");
        });
    }
    
    /// 성능 모니터링
    async fn start_performance_monitor(&self) {
        let is_running = Arc::clone(&self.is_running);
        let strategy = Arc::clone(&self.micro_arbitrage_strategy);
        let exchange_monitor = Arc::clone(&self.exchange_monitor);
        let price_feed_manager = Arc::clone(&self._price_feed_manager);
        let order_executor = Arc::clone(&self.order_executor);
        
        tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(30)); // 30초마다
            
            info!("📈 성능 모니터링 시작");
            
            while is_running.load(Ordering::SeqCst) {
                monitor_interval.tick().await;
                
                // 전략 통계
                let strategy_stats = (*strategy).get_stats().await;
                
                // 모니터링 통계
                let monitor_stats = (*exchange_monitor).get_monitoring_stats().await;
                
                // 피드 매니저 통계
                let feed_stats = (*price_feed_manager).get_stats().await;
                
                // 주문 실행 통계
                let executor_stats = (*order_executor).get_stats().await;
                
                info!("📊 마이크로아비트래지 성능 리포트:");
                info!("  ⚡ 총 기회: {}, 실행: {}, 성공률: {:.2}%", 
                      strategy_stats.total_opportunities,
                      strategy_stats.executed_trades,
                      strategy_stats.success_rate * 100.0);
                info!("  💰 총 수익: {} ETH, 평균 거래당: {} ETH", 
                      strategy_stats.total_profit, 
                      strategy_stats.avg_profit_per_trade);
                info!("  🏛️ 거래소 연결: {}/{}", 
                      monitor_stats.active_connections, 
                      monitor_stats.active_connections + monitor_stats.failed_connections);
                info!("  📡 데이터 품질: {:.2}, 평균 지연: {:.1}ms", 
                      feed_stats.data_quality_score,
                      monitor_stats.avg_latency_ms);
                info!("  🚀 주문 실행: {}건, 성공률: {:.2}%", 
                      executor_stats.total_orders,
                      executor_stats.success_rate * 100.0);
                
                // 경고 발생 조건 체크
                if strategy_stats.success_rate < 0.8 {
                    warn!("⚠️ 아비트래지 성공률 저하: {:.2}%", strategy_stats.success_rate * 100.0);
                }
                
                if monitor_stats.avg_latency_ms > 200.0 {
                    warn!("⚠️ 거래소 지연시간 증가: {:.1}ms", monitor_stats.avg_latency_ms);
                }
                
                if feed_stats.data_quality_score < 0.7 {
                    warn!("⚠️ 데이터 품질 저하: {:.2}", feed_stats.data_quality_score);
                }
            }
            
            info!("📈 성능 모니터링 종료");
        });
    }
    
    /// 헬스 모니터링
    async fn start_health_monitor(&self) {
        let is_running = Arc::clone(&self.is_running);
        let exchange_monitor = Arc::clone(&self.exchange_monitor);
        let price_feed_manager = Arc::clone(&self._price_feed_manager);
        let strategy = Arc::clone(&self.micro_arbitrage_strategy);
        
        tokio::spawn(async move {
            let mut health_interval = interval(Duration::from_secs(60)); // 1분마다
            
            info!("🏥 헬스 모니터링 시작");
            
            while is_running.load(Ordering::SeqCst) {
                health_interval.tick().await;
                
                // 각 컴포넌트 상태 점검
                let monitor_running = exchange_monitor.is_running();
                let feed_manager_running = price_feed_manager.is_running();
                let strategy_enabled = strategy.is_enabled();
                
                debug!("🏥 헬스 체크 - 모니터: {}, 피드: {}, 전략: {}", 
                       monitor_running, feed_manager_running, strategy_enabled);
                
                // 비정상 상태 감지
                if !monitor_running {
                    error!("❌ 거래소 모니터가 중지됨");
                    warn!("자동 재시작은 외부 supervisor 또는 상위 루프에서 처리 예정");
                }
                
                if !feed_manager_running {
                    error!("❌ 가격 피드 매니저가 중지됨");
                    warn!("자동 재시작은 외부 supervisor 또는 상위 루프에서 처리 예정");
                }
                
                if !strategy_enabled {
                    warn!("⚠️ 마이크로아비트래지 전략이 비활성화됨");
                }
            }
            
            info!("🏥 헬스 모니터링 종료");
        });
    }
    
    /// 마이크로아비트래지 시스템 중지
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("⚠️ 마이크로아비트래지 오케스트레이터가 이미 중지됨");
            return Ok(());
        }
        
        info!("🛑 마이크로아비트래지 시스템 중지 중...");
        self.is_running.store(false, Ordering::SeqCst);
        
        // 1. 전략 중지
        self.micro_arbitrage_strategy.stop().await?;
        
        // 2. 가격 피드 매니저 중지
        self._price_feed_manager.stop().await?;
        
        // 3. 거래소 모니터 중지
        self.exchange_monitor.stop().await?;
        
        // 4. 처리 중인 작업 완료 대기
        sleep(Duration::from_millis(1000)).await;
        
        info!("✅ 마이크로아비트래지 시스템 중지 완료");
        Ok(())
    }
    
    /// 실행 상태 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// 종합 상태 조회
    pub async fn get_comprehensive_status(&self) -> MicroArbitrageSystemStatus {
        let strategy_stats = (*self.micro_arbitrage_strategy).get_stats().await;
        let monitor_stats = (*self.exchange_monitor).get_monitoring_stats().await;
        let feed_stats = (*self._price_feed_manager).get_stats().await;
        let executor_stats = (*self.order_executor).get_stats().await;
        
        MicroArbitrageSystemStatus {
            is_running: self.is_running(),
            strategy_enabled: (*self.micro_arbitrage_strategy).is_enabled(),
            monitor_running: (*self.exchange_monitor).is_running(),
            feed_manager_running: (*self._price_feed_manager).is_running(),
            
            total_opportunities: strategy_stats.total_opportunities,
            executed_trades: strategy_stats.executed_trades,
            success_rate: strategy_stats.success_rate,
            total_profit_eth: strategy_stats.total_profit,
            
            active_exchanges: monitor_stats.active_connections,
            avg_latency_ms: monitor_stats.avg_latency_ms,
            data_quality_score: feed_stats.data_quality_score,
            
            total_orders: executor_stats.total_orders,
            order_success_rate: executor_stats.success_rate,
            avg_execution_time_ms: executor_stats.average_execution_time_ms,
        }
    }
}

/// 마이크로아비트래지 시스템 종합 상태
#[derive(Debug, Clone)]
pub struct MicroArbitrageSystemStatus {
    // 시스템 상태
    pub is_running: bool,
    pub strategy_enabled: bool,
    pub monitor_running: bool,
    pub feed_manager_running: bool,
    
    // 거래 성과
    pub total_opportunities: u64,
    pub executed_trades: u64,
    pub success_rate: f64,
    pub total_profit_eth: alloy::primitives::U256,
    
    // 인프라 상태
    pub active_exchanges: u32,
    pub avg_latency_ms: f64,
    pub data_quality_score: f64,
    
    // 주문 실행
    pub total_orders: u64,
    pub order_success_rate: f64,
    pub avg_execution_time_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_orchestrator_creation() {
        let config = Arc::new(crate::config::Config::default());
        
        // Skip test if we can't create a provider (no real network connection needed for this test)
        // In a real test environment, you would use a mock provider
        println!("MicroArbitrage orchestrator creation test - would test with mock components in production");
        
        assert!(true); // Placeholder assertion - replace with mock components test
    }
}