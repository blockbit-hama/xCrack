//! 마이크로아비트리지 통합 관리자
//! 
//! 이 모듈은 마이크로아비트리지 전략의 모든 컴포넌트를
//! 통합하여 관리하는 중앙 관리자를 제공합니다.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::{interval, sleep, Duration};
use tracing::{info, debug, warn, error};
use ethers::types::U256;
use ethers::prelude::*;
use chrono::Utc;

use crate::config::Config;
use crate::exchange::ExchangeClientFactory;
use super::types::{
    MicroArbitrageOpportunity, MicroArbitrageSystemStatus, 
    MicroArbitrageStats, RiskMetrics, ExecutionPriority
};
use super::price_monitor::PriceMonitor;
use super::opportunity_detector::OpportunityDetector;
use super::execution_engine::ExecutionEngine;
use super::risk_manager::RiskManager;
use super::performance_tracker::PerformanceTracker;

/// 마이크로아비트리지 통합 관리자
pub struct MicroArbitrageManager {
    config: Arc<Config>,
    is_running: Arc<AtomicBool>,
    
    // 핵심 컴포넌트들
    price_monitor: Arc<PriceMonitor>,
    opportunity_detector: Arc<OpportunityDetector>,
    execution_engine: Arc<ExecutionEngine>,
    risk_manager: Arc<RiskManager>,
    performance_tracker: Arc<PerformanceTracker>,
    
    // 채널
    price_sender: Option<mpsc::UnboundedSender<super::types::PriceData>>,
    orderbook_sender: Option<mpsc::UnboundedSender<super::types::OrderBookSnapshot>>,
    
    // 설정
    scan_interval_ms: u64,
    max_concurrent_trades: usize,
    min_profit_threshold: f64,
}

impl MicroArbitrageManager {
    /// 새로운 마이크로아비트리지 관리자 생성
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        info!("🎼 마이크로아비트리지 통합 관리자 초기화 중...");

        let micro_config = &config.strategies.micro_arbitrage;

        // Provider와 Wallet 초기화
        let rpc_ws_url = std::env::var("ETH_RPC_WS_URL")
            .map_err(|_| anyhow!("ETH_RPC_WS_URL not set in environment"))?;

        let provider = Arc::new(
            Provider::<Ws>::connect(&rpc_ws_url)
                .await
                .map_err(|e| anyhow!("Failed to connect to Ethereum node: {}", e))?
        );

        let private_key = std::env::var("PRIVATE_KEY")
            .map_err(|_| anyhow!("PRIVATE_KEY not set in environment"))?;

        let wallet: LocalWallet = private_key
            .parse()
            .map_err(|e| anyhow!("Failed to parse private key: {}", e))?;

        info!("✅ Provider와 Wallet 초기화 완료");

        // 가격 모니터 초기화
        let price_monitor = Arc::new(PriceMonitor::new(Arc::clone(&config)).await?);

        // 기회 탐지기 초기화
        let opportunity_detector = Arc::new(OpportunityDetector::new(Arc::clone(&config)));

        // 실행 엔진 초기화 (provider와 wallet 전달)
        let execution_engine = Arc::new(
            ExecutionEngine::new(Arc::clone(&config), provider.clone(), wallet.clone()).await?
        );

        // 위험 관리자 초기화
        let risk_manager = Arc::new(RiskManager::new(Arc::clone(&config)));

        // 성능 추적기 초기화
        let performance_tracker = Arc::new(PerformanceTracker::new(Arc::clone(&config)));

        info!("✅ 마이크로아비트리지 통합 관리자 초기화 완료");

        Ok(Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            price_monitor,
            opportunity_detector,
            execution_engine,
            risk_manager,
            performance_tracker,
            price_sender: None,
            orderbook_sender: None,
            scan_interval_ms: micro_config.price_update_interval_ms,
            max_concurrent_trades: micro_config.max_concurrent_trades,
            min_profit_threshold: micro_config.min_profit_percentage,
        })
    }
    
    /// 마이크로아비트리지 시스템 시작
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("⚠️ 마이크로아비트리지 시스템이 이미 실행 중입니다");
            return Ok(());
        }

        info!("🚀 마이크로아비트리지 시스템 시작 중...");

        // 데이터 플로우 채널 생성
        let (price_sender, price_receiver) = mpsc::unbounded_channel();
        let (orderbook_sender, orderbook_receiver) = mpsc::unbounded_channel();

        self.price_sender = Some(price_sender);
        self.orderbook_sender = Some(orderbook_sender);

        // 우선순위 큐 생성
        let opportunity_queue = Arc::new(RwLock::new(std::collections::BinaryHeap::new()));

        // 실행 상태 설정
        self.is_running.store(true, Ordering::SeqCst);

        // 1. 가격 모니터링 시작
        info!("📡 가격 모니터링 시작...");
        self.price_monitor.start(
            self.price_sender.as_ref().unwrap().clone(),
            self.orderbook_sender.as_ref().unwrap().clone(),
        ).await?;

        // 2. 기회 탐지 루프 시작
        self.start_opportunity_detection_loop(Arc::clone(&opportunity_queue)).await;

        // 3. 실행 루프 시작
        self.start_execution_loop(Arc::clone(&opportunity_queue)).await;

        // 4. 성능 모니터링 시작
        self.start_performance_monitoring().await;

        // 5. 위험 관리 모니터링 시작
        self.start_risk_monitoring().await;

        // 6. 헬스 체크 시작
        self.start_health_monitoring().await;

        info!("✅ 마이크로아비트리지 시스템 시작 완료");
        Ok(())
    }
    
    /// 마이크로아비트리지 시스템 중지
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("⚠️ 마이크로아비트리지 시스템이 이미 중지됨");
            return Ok(());
        }
        
        info!("🛑 마이크로아비트리지 시스템 중지 중...");
        
        // 실행 상태 설정
        self.is_running.store(false, Ordering::SeqCst);
        
        // 가격 모니터링 중지
        self.price_monitor.stop().await?;
        
        // 처리 중인 작업 완료 대기
        sleep(Duration::from_millis(1000)).await;
        
        info!("✅ 마이크로아비트리지 시스템 중지 완료");
        Ok(())
    }
    
    /// 기회 탐지 루프 시작
    async fn start_opportunity_detection_loop(
        &self,
        opportunity_queue: Arc<RwLock<std::collections::BinaryHeap<OpportunityWrapper>>>
    ) {
        let is_running = Arc::clone(&self.is_running);
        let opportunity_detector = Arc::clone(&self.opportunity_detector);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let scan_interval_ms = self.scan_interval_ms;
        let price_monitor = Arc::clone(&self.price_monitor);

        tokio::spawn(async move {
            let mut scan_interval = interval(Duration::from_millis(scan_interval_ms));

            info!("🔍 기회 탐지 루프 시작 ({}ms 간격)", scan_interval_ms);

            while is_running.load(Ordering::SeqCst) {
                scan_interval.tick().await;

                // 실제 가격 데이터 수집
                let price_data_map = Self::collect_real_price_data_static(&price_monitor).await;

                // 아비트리지 기회 탐지
                match opportunity_detector.detect_opportunities(&price_data_map).await {
                    Ok(opportunities) => {
                        if !opportunities.is_empty() {
                            debug!("🔍 {}개 아비트리지 기회 탐지됨", opportunities.len());

                            // 기회를 우선순위 큐에 추가
                            let mut queue = opportunity_queue.write().await;
                            for opportunity in opportunities {
                                performance_tracker.record_opportunity(opportunity.clone()).await;
                                queue.push(OpportunityWrapper(opportunity));
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 기회 탐지 실패: {}", e);
                    }
                }
            }

            info!("🔍 기회 탐지 루프 종료");
        });
    }
    
    /// 실행 루프 시작
    async fn start_execution_loop(&self, opportunity_queue: Arc<RwLock<std::collections::BinaryHeap<OpportunityWrapper>>>) {
        let is_running = Arc::clone(&self.is_running);
        let execution_engine = Arc::clone(&self.execution_engine);
        let risk_manager = Arc::clone(&self.risk_manager);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let max_concurrent_trades = self.max_concurrent_trades;

        tokio::spawn(async move {
            let mut execution_interval = interval(Duration::from_millis(1000)); // 1초마다

            info!("⚡ 실행 루프 시작");

            while is_running.load(Ordering::SeqCst) {
                execution_interval.tick().await;

                // 우선순위 큐에서 기회 가져오기 (최대 수익률 순)
                let mut opportunities = Vec::new();
                {
                    let mut queue = opportunity_queue.write().await;
                    for _ in 0..max_concurrent_trades {
                        if let Some(wrapper) = queue.pop() {
                            opportunities.push(wrapper.0);
                        } else {
                            break;
                        }
                    }
                }

                for opportunity in opportunities {
                    // 기회 유효성 재확인
                    if !opportunity.is_valid() {
                        debug!("⚠️ 만료된 기회: {}", opportunity.id);
                        continue;
                    }

                    // 위험 평가
                    match risk_manager.assess_opportunity_risk(&opportunity).await {
                        Ok(assessment) => {
                            if assessment.recommendation != crate::strategies::micro_arbitrage::risk_manager::RiskRecommendation::Reject {
                                // 포지션 열기
                                if let Err(e) = risk_manager.open_position(&opportunity).await {
                                    error!("❌ 포지션 열기 실패: {}", e);
                                    continue;
                                }

                                // 아비트리지 실행
                                match execution_engine.execute_arbitrage(opportunity.clone()).await {
                                    Ok(result) => {
                                        // 실행 결과 기록
                                        performance_tracker.record_execution(result.clone()).await;

                                        // 포지션 관리
                                        if result.success {
                                            risk_manager.close_position(&opportunity.id, result.actual_profit.unwrap_or(U256::zero())).await.unwrap_or_default();
                                        } else {
                                            risk_manager.close_position(&opportunity.id, U256::zero()).await.unwrap_or_default();
                                        }
                                    }
                                    Err(e) => {
                                        error!("❌ 아비트리지 실행 실패: {}", e);
                                        risk_manager.close_position(&opportunity.id, U256::zero()).await.unwrap_or_default();
                                    }
                                }
                            } else {
                                debug!("⚠️ 위험도가 높아 기회 거부: {}", opportunity.id);
                            }
                        }
                        Err(e) => {
                            error!("❌ 위험 평가 실패: {}", e);
                        }
                    }
                }
            }

            info!("⚡ 실행 루프 종료");
        });
    }
    
    /// 성능 모니터링 시작
    async fn start_performance_monitoring(&self) {
        let is_running = Arc::clone(&self.is_running);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let risk_manager = Arc::clone(&self.risk_manager);
        
        tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(30)); // 30초마다
            
            info!("📈 성능 모니터링 시작");
            
            while is_running.load(Ordering::SeqCst) {
                monitor_interval.tick().await;
                
                // 위험 메트릭 업데이트
                risk_manager.update_risk_metrics().await.unwrap_or_default();
                
                // 성능 통계 업데이트
                let stats = performance_tracker.get_stats().await;
                let risk_metrics = performance_tracker.get_risk_metrics().await;
                
                info!("📊 성능 모니터링 리포트:");
                info!("  ⚡ 총 기회: {}, 실행: {}, 성공률: {:.2}%", 
                      stats.total_opportunities,
                      stats.executed_trades,
                      stats.success_rate * 100.0);
                info!("  💰 총 수익: {} ETH, 평균 거래당: {} ETH", 
                      stats.total_profit, 
                      stats.avg_profit_per_trade);
                info!("  ⚠️ 현재 노출도: {} ETH, 일일 PnL: {} ETH", 
                      risk_metrics.current_exposure, 
                      risk_metrics.daily_pnl);
                info!("  📈 승률: {:.2}%, 샤프 비율: {:.2}", 
                      risk_metrics.win_rate * 100.0, 
                      risk_metrics.sharpe_ratio);
            }
            
            info!("📈 성능 모니터링 종료");
        });
    }
    
    /// 위험 관리 모니터링 시작
    async fn start_risk_monitoring(&self) {
        let is_running = Arc::clone(&self.is_running);
        let risk_manager = Arc::clone(&self.risk_manager);
        
        tokio::spawn(async move {
            let mut risk_interval = interval(Duration::from_secs(60)); // 1분마다
            
            info!("⚠️ 위험 관리 모니터링 시작");
            
            while is_running.load(Ordering::SeqCst) {
                risk_interval.tick().await;
                
                // 위험 메트릭 업데이트
                risk_manager.update_risk_metrics().await.unwrap_or_default();
                
                // 위험 임계값 확인
                let risk_metrics = risk_manager.get_risk_metrics().await;
                
                // 일일 손실 한도 확인
                if risk_metrics.daily_pnl < U256::zero() {
                    let loss_amount = -risk_metrics.daily_pnl.as_u128() as f64 / 1e18;
                    if loss_amount > 100.0 { // 100 ETH 손실
                        warn!("⚠️ 일일 손실 한도 초과: {:.2} ETH", loss_amount);
                    }
                }
                
                // 노출도 확인
                let exposure_eth = risk_metrics.current_exposure.as_u128() as f64 / 1e18;
                if exposure_eth > 1000.0 { // 1000 ETH 노출
                    warn!("⚠️ 노출도 한도 초과: {:.2} ETH", exposure_eth);
                }
            }
            
            info!("⚠️ 위험 관리 모니터링 종료");
        });
    }
    
    /// 헬스 체크 시작
    async fn start_health_monitoring(&self) {
        let is_running = Arc::clone(&self.is_running);
        let price_monitor = Arc::clone(&self.price_monitor);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        
        tokio::spawn(async move {
            let mut health_interval = interval(Duration::from_secs(60)); // 1분마다
            
            info!("🏥 헬스 체크 시작");
            
            while is_running.load(Ordering::SeqCst) {
                health_interval.tick().await;
                
                // 각 컴포넌트 상태 확인
                let price_monitor_running = price_monitor.is_running().await;
                let performance_stats = performance_tracker.get_stats().await;
                
                debug!("🏥 헬스 체크 - 가격 모니터: {}, 기회: {}, 실행: {}", 
                       price_monitor_running, 
                       performance_stats.total_opportunities,
                       performance_stats.executed_trades);
                
                // 비정상 상태 감지
                if !price_monitor_running {
                    error!("❌ 가격 모니터가 중지됨");
                }
                
                // 성능 저하 감지
                if performance_stats.success_rate < 0.5 && performance_stats.executed_trades > 10 {
                    warn!("⚠️ 성공률 저하: {:.2}%", performance_stats.success_rate * 100.0);
                }
            }
            
            info!("🏥 헬스 체크 종료");
        });
    }
    
    /// 시스템 상태 조회
    pub async fn get_system_status(&self) -> MicroArbitrageSystemStatus {
        let is_running = self.is_running.load(Ordering::SeqCst);
        let performance_stats = self.performance_tracker.get_stats().await;
        let risk_metrics = self.performance_tracker.get_risk_metrics().await;
        let monitoring_status = self.price_monitor.get_monitoring_status().await;
        let price_feed_status = self.price_monitor.get_price_feed_status().await;
        
        MicroArbitrageSystemStatus {
            is_running,
            strategy_enabled: is_running,
            monitoring_status: crate::strategies::micro_arbitrage::types::MonitoringStatus {
                is_running: monitoring_status.is_running,
                active_exchanges: monitoring_status.active_exchanges,
                failed_exchanges: monitoring_status.failed_exchanges,
                avg_latency_ms: monitoring_status.avg_latency_ms,
                data_quality_score: monitoring_status.data_quality_score,
                last_heartbeat: monitoring_status.last_heartbeat,
                error_count: monitoring_status.error_count,
                last_error: monitoring_status.last_error,
            },
            price_feed_status: crate::strategies::micro_arbitrage::types::PriceFeedStatus {
                is_active: price_feed_status.is_active,
                feeds_count: price_feed_status.feeds_count,
                last_update: price_feed_status.last_update,
                update_frequency_ms: price_feed_status.update_frequency_ms,
                missed_updates: price_feed_status.missed_updates,
                data_freshness_ms: price_feed_status.data_freshness_ms,
            },
            execution_engine_status: crate::strategies::micro_arbitrage::types::ExecutionEngineStatus {
                is_running: is_running,
                active_orders: 0, // 실제로는 실행 엔진에서 가져와야 함
                pending_orders: 0,
                completed_orders: performance_stats.executed_trades,
                failed_orders: performance_stats.failed_trades,
                avg_execution_time_ms: performance_stats.avg_execution_time_ms,
                last_execution: Some(Utc::now()), // 실제로는 마지막 실행 시간
            },
            performance_stats,
            risk_metrics,
            last_health_check: Utc::now(),
        }
    }
    
    /// 성능 리포트 생성
    pub async fn generate_performance_report(&self) -> String {
        self.performance_tracker.generate_performance_report().await
    }
    
    /// 실행 상태 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// 특정 기회 실행
    pub async fn execute_opportunity(
        &self,
        opportunity: MicroArbitrageOpportunity,
    ) -> Result<()> {
        info!("🎯 특정 기회 실행: {}", opportunity.id);
        
        // 위험 평가
        let assessment = self.risk_manager.assess_opportunity_risk(&opportunity).await?;
        
        if assessment.recommendation == crate::strategies::micro_arbitrage::risk_manager::RiskRecommendation::Reject {
            return Err(anyhow!("위험도가 높아 실행 거부됨"));
        }
        
        // 포지션 열기
        self.risk_manager.open_position(&opportunity).await?;
        
        // 아비트리지 실행
        let result = self.execution_engine.execute_arbitrage(opportunity.clone()).await?;
        
        // 실행 결과 기록
        self.performance_tracker.record_execution(result.clone()).await;
        
        // 포지션 닫기
        if result.success {
            self.risk_manager.close_position(
                &opportunity.id,
                result.actual_profit.unwrap_or(U256::zero())
            ).await?;
        }
        
        Ok(())
    }
    
    /// 일일 리셋
    pub async fn daily_reset(&self) -> Result<()> {
        info!("🔄 일일 리셋 시작");
        
        // 위험 관리자 리셋
        self.risk_manager.daily_reset().await?;
        
        info!("✅ 일일 리셋 완료");
        Ok(())
    }
    
    /// 실제 가격 데이터 수집 (정적 메서드)
    async fn collect_real_price_data_static(
        price_monitor: &Arc<PriceMonitor>
    ) -> std::collections::HashMap<String, super::types::PriceData> {
        let mut price_data_map = std::collections::HashMap::new();

        // 환경변수에서 거래 페어 가져오기
        let trading_pairs = std::env::var("MICRO_ARB_TRADING_PAIRS")
            .unwrap_or_else(|_| "ETH/USDT,WBTC/USDT".to_string());

        let pairs: Vec<&str> = trading_pairs.split(',').collect();

        // 각 거래 페어별로 모든 거래소의 가격 데이터 수집
        for pair in pairs {
            let pair = pair.trim();
            let all_prices = price_monitor.get_all_price_data(pair).await;

            for (exchange_name, price_data) in all_prices {
                let key = format!("{}_{}", exchange_name, pair);
                price_data_map.insert(key, price_data);
            }
        }

        price_data_map
    }
}

/// 우선순위 큐를 위한 Wrapper (수익률 기반 정렬)
#[derive(Clone)]
struct OpportunityWrapper(types::MicroArbitrageOpportunity);

impl PartialEq for OpportunityWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.net_profit == other.0.net_profit
    }
}

impl Eq for OpportunityWrapper {}

impl PartialOrd for OpportunityWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OpportunityWrapper {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 높은 순이익이 먼저 나오도록 (max heap)
        self.0.net_profit.cmp(&other.0.net_profit)
    }
}