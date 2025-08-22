use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug, error, warn};
use alloy::primitives::U256;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{RwLock, mpsc, Mutex};
use std::time::{Instant, Duration};
use ethers::providers::{Provider, Ws};

use crate::config::Config;
use crate::types::{PerformanceMetrics, Transaction, Opportunity, Bundle};
use crate::strategies::StrategyManager;
use crate::mocks::{is_mock_mode, MockFlashbotsClient, MockRpcProvider, MockMempoolMonitor};
use super::{
    BundleManager, 
    CoreMempoolMonitor, 
    PerformanceTracker,
    MicroArbitrageOrchestrator,
    MicroArbitrageSystemStatus,
};

#[derive(Debug, Clone)]
pub struct SearcherStatus {
    pub is_running: bool,
    pub active_opportunities: usize,
    pub submitted_bundles: usize,
    pub performance_metrics: PerformanceMetrics,
    pub uptime_seconds: u64,
    pub micro_arbitrage_status: Option<MicroArbitrageSystemStatus>,
}

pub struct SearcherCore {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    is_running: Arc<AtomicBool>,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    
    // 핵심 컴포넌트들
    pub(crate) strategy_manager: Arc<StrategyManager>,
    bundle_manager: Arc<BundleManager>,
    mempool_monitor: Arc<CoreMempoolMonitor>,
    performance_tracker: Arc<PerformanceTracker>,
    
    // 마이크로아비트래지 시스템 (옵셔널)
    micro_arbitrage_orchestrator: Option<Arc<Mutex<MicroArbitrageOrchestrator>>>,
    
    // 채널들
    tx_sender: Option<mpsc::UnboundedSender<Transaction>>,
    opportunity_sender: Option<mpsc::UnboundedSender<Opportunity>>,
    bundle_sender: Option<mpsc::UnboundedSender<Bundle>>,
}

impl SearcherCore {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🔧 SearcherCore 초기화 중...");
        
        let initial_metrics = PerformanceMetrics {
            transactions_processed: 0,
            opportunities_found: 0,
            bundles_submitted: 0,
            bundles_included: 0,
            total_profit: U256::ZERO,
            total_gas_spent: U256::ZERO,
            avg_analysis_time: 0.0,
            avg_submission_time: 0.0,
            success_rate: 0.0,
            uptime: 0,
        };
        
        // 핵심 컴포넌트들 초기화
        let strategy_manager = Arc::new(StrategyManager::new(Arc::clone(&config), Arc::clone(&provider)).await?);
        let bundle_manager = Arc::new(BundleManager::new(Arc::clone(&config)).await?);
        let mempool_monitor = Arc::new(CoreMempoolMonitor::new(Arc::clone(&config), Arc::clone(&provider)).await?);
        let performance_tracker = Arc::new(PerformanceTracker::new(Arc::clone(&config)).await?);
        
        // 마이크로아비트래지 시스템 초기화 (활성화된 경우)
        let micro_arbitrage_orchestrator = if config.strategies.micro_arbitrage.enabled {
            info!("🎼 마이크로아비트래지 시스템 초기화 중...");
            
            // 타입 안전한 핸들로 직접 가져오기
            if let Some(micro_strategy) = strategy_manager.get_micro_arbitrage_strategy() {
                match MicroArbitrageOrchestrator::new(Arc::clone(&config), micro_strategy).await {
                    Ok(orchestrator) => {
                        info!("✅ 마이크로아비트래지 오케스트레이터 초기화 완료");
                        Some(Arc::new(Mutex::new(orchestrator)))
                    }
                    Err(e) => {
                        error!("❌ 마이크로아비트래지 오케스트레이터 초기화 실패: {}", e);
                        None
                    }
                }
            } else {
                warn!("⚠️ 마이크로아비트래지 전략을 찾을 수 없음");
                None
            }
        } else {
            None
        };
        
        info!("✅ SearcherCore 초기화 완료");
        
        Ok(Self {
            config,
            provider,
            is_running: Arc::new(AtomicBool::new(false)),
            metrics: Arc::new(RwLock::new(initial_metrics)),
            strategy_manager,
            bundle_manager,
            mempool_monitor,
            performance_tracker,
            micro_arbitrage_orchestrator,
            tx_sender: None,
            opportunity_sender: None,
            bundle_sender: None,
        })
    }

    pub fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            provider: Arc::clone(&self.provider),
            is_running: Arc::clone(&self.is_running),
            metrics: Arc::clone(&self.metrics),
            strategy_manager: Arc::clone(&self.strategy_manager),
            bundle_manager: Arc::clone(&self.bundle_manager),
            mempool_monitor: Arc::clone(&self.mempool_monitor),
            performance_tracker: Arc::clone(&self.performance_tracker),
            micro_arbitrage_orchestrator: self.micro_arbitrage_orchestrator.as_ref().map(Arc::clone),
            tx_sender: self.tx_sender.clone(),
            opportunity_sender: self.opportunity_sender.clone(),
            bundle_sender: self.bundle_sender.clone(),
        }
    }

    /// 서쳐 시작
    pub async fn start(&self) -> Result<()> {
        info!("🚀 SearcherCore 시작 중...");
        
        if self.is_running.load(Ordering::SeqCst) {
            warn!("⚠️ SearcherCore가 이미 실행 중입니다");
            return Ok(());
        }
        
        self.is_running.store(true, Ordering::SeqCst);
        
        // 1. 전략 매니저 초기화
        info!("🎯 전략 매니저 초기화 중...");
        self.strategy_manager.start_all_strategies().await?;
        
        // 2. 채널 생성
        let (tx_sender, mut tx_receiver) = mpsc::unbounded_channel::<Transaction>();
        let (opportunity_sender, mut opportunity_receiver) = mpsc::unbounded_channel::<Opportunity>();
        let (bundle_sender, mut bundle_receiver) = mpsc::unbounded_channel::<Bundle>();
        
        // 채널 저장 (run_main_loop에서 사용하기 위해)
        // Note: 실제로는 Arc<RwLock<>> 패턴이 더 안전하지만, 현재 구조상 mut self가 필요
        // 임시 해결책: 채널을 직접 전달
        
        // 3. 멤풀 모니터링 시작
        info!("📡 멤풀 모니터링 시작 중...");
        self.mempool_monitor.start(tx_sender.clone()).await?;
        
        // 3.1. 마이크로아비트래지 시스템 시작 (활성화된 경우)
        if let Some(orchestrator_arc) = &self.micro_arbitrage_orchestrator {
            info!("⚡ 마이크로아비트래지 시스템 시작 중...");
            let orchestrator_arc = Arc::clone(orchestrator_arc);
            // 별도 태스크에서 구동
            tokio::spawn(async move {
                let guard = orchestrator_arc.lock().await;
                if let Err(e) = guard.start().await {
                    error!("❌ 마이크로아비트래지 오케스트레이터 시작 실패: {}", e);
                }
            });
        }
        
        // 4. 메인 처리 루프 실행
        info!("🔄 메인 처리 루프 시작 중...");
        self.run_main_loop(
            tx_receiver,
            opportunity_receiver,
            bundle_receiver,
            opportunity_sender,
            bundle_sender
        ).await?;
        
        Ok(())
    }

    /// 메인 처리 루프
    async fn run_main_loop(
        &self,
        mut tx_receiver: mpsc::UnboundedReceiver<Transaction>,
        mut opportunity_receiver: mpsc::UnboundedReceiver<Opportunity>,
        mut bundle_receiver: mpsc::UnboundedReceiver<Bundle>,
        opportunity_sender: mpsc::UnboundedSender<Opportunity>,
        bundle_sender: mpsc::UnboundedSender<Bundle>,
    ) -> Result<()> {
        info!("🎯 메인 처리 루프 실행 중...");
        
        // 트랜잭션 처리 태스크
        let strategy_manager = Arc::clone(&self.strategy_manager);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let opportunity_sender_clone = opportunity_sender.clone();
        
        tokio::spawn(async move {
            info!("🔄 트랜잭션 처리 태스크 시작");
            
            while let Some(transaction) = tx_receiver.recv().await {
                let analysis_start = Instant::now();
                
                debug!("📝 트랜잭션 분석 중: {}", transaction.hash);
                
                // 전략 매니저로 트랜잭션 분석
                let opportunities = strategy_manager.analyze_transaction(&transaction).await;
                
                let analysis_duration = analysis_start.elapsed();
                let analysis_time_ms = analysis_duration.as_millis() as f64;
                
                // 성능 추적
                if let Err(e) = performance_tracker.record_transaction_processed(analysis_time_ms).await {
                    error!("❌ 성능 추적 실패: {}", e);
                }
                
                // 발견된 기회들을 전송
                for opportunity in opportunities {
                    if let Err(e) = opportunity_sender_clone.send(opportunity) {
                        error!("❌ 기회 전송 실패: {}", e);
                    }
                }
            }
        });
        
        // 기회 검증 및 번들 생성 태스크
        let strategy_manager = Arc::clone(&self.strategy_manager);
        let bundle_manager = Arc::clone(&self.bundle_manager);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let bundle_sender_clone = bundle_sender.clone();
        
        tokio::spawn(async move {
            info!("✅ 기회 검증 및 번들 생성 태스크 시작");
            
            while let Some(opportunity) = opportunity_receiver.recv().await {
                debug!("🎯 기회 검증 중: {}", opportunity.id);
                
                // 기회 검증
                let validated_opportunities = strategy_manager.validate_opportunities(vec![opportunity]).await;
                
                if !validated_opportunities.is_empty() {
                    // 성능 추적
                    for opp in &validated_opportunities {
                        if let Err(e) = performance_tracker.record_opportunity_found(
                            &opp.strategy.to_string(), 
                            opp.expected_profit
                        ).await {
                            error!("❌ 기회 추적 실패: {}", e);
                        }
                    }
                    
                    // 번들 생성
                    match bundle_manager.create_optimal_bundle(validated_opportunities).await {
                        Ok(Some(bundle)) => {
                            info!("📦 번들 생성됨: {}", bundle.id);
                            if let Err(e) = bundle_sender_clone.send(bundle) {
                                error!("❌ 번들 전송 실패: {}", e);
                            }
                        }
                        Ok(None) => {
                            debug!("📭 유효한 번들 생성 불가");
                        }
                        Err(e) => {
                            error!("❌ 번들 생성 실패: {}", e);
                        }
                    }
                }
            }
        });
        
        // 번들 제출 태스크
        let bundle_manager = Arc::clone(&self.bundle_manager);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        
        tokio::spawn(async move {
            info!("🚀 번들 제출 태스크 시작");
            
            while let Some(bundle) = bundle_receiver.recv().await {
                let submission_start = Instant::now();
                
                info!("📤 번들 제출 중: {}", bundle.id);
                
                // 번들 제출
                match bundle_manager.submit_bundle(bundle.clone()).await {
                    Ok(success) => {
                        let submission_duration = submission_start.elapsed();
                        let submission_time_ms = submission_duration.as_millis() as f64;
                        
                        if success {
                            info!("✅ 번들 제출 성공: {} (제출 시간: {:.2}ms)", 
                                  bundle.id, submission_time_ms);
                            
                            // 성능 추적
                            if let Err(e) = performance_tracker.record_bundle_submitted(submission_time_ms).await {
                                error!("❌ 번들 제출 추적 실패: {}", e);
                            }
                        } else {
                            error!("❌ 번들 제출 실패: {}", bundle.id);
                            
                            // 실패 추적
                            if let Err(e) = performance_tracker.record_bundle_failed().await {
                                error!("❌ 번들 실패 추적 실패: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 번들 제출 오류: {}", e);
                        
                        // 에러 추적
                        if let Err(track_err) = performance_tracker.record_error("bundle_submission", &e.to_string()).await {
                            error!("❌ 에러 추적 실패: {}", track_err);
                        }
                    }
                }
            }
        });
        
        // 성능 모니터링 태스크
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let mempool_monitor = Arc::clone(&self.mempool_monitor);
        let bundle_manager = Arc::clone(&self.bundle_manager);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // 1분마다
            
            loop {
                interval.tick().await;
                
                // 성능 리포트 생성
                match performance_tracker.generate_performance_report().await {
                    Ok(report) => {
                        info!("📊 성능 리포트:");
                        info!("  🔄 트랜잭션 처리: {}", report.summary.transactions_processed);
                        info!("  🎯 기회 발견: {}", report.summary.opportunities_found);
                        info!("  📦 번들 제출: {}", report.summary.bundles_submitted);
                        info!("  ✅ 번들 포함: {}", report.summary.bundles_included);
                        info!("  💰 총 수익: {} ETH", report.summary.total_profit_eth);
                        info!("  📈 성공률: {:.2}%", report.summary.success_rate * 100.0);
                        info!("  ⏱️ 평균 분석 시간: {:.2}ms", report.summary.avg_analysis_time_ms);
                        info!("  🚀 평균 제출 시간: {:.2}ms", report.summary.avg_submission_time_ms);
                        
                        // 권장사항 출력
                        if !report.recommendations.is_empty() {
                            info!("💡 권장사항:");
                            for rec in &report.recommendations {
                                info!("  • {}", rec);
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 성능 리포트 생성 실패: {}", e);
                    }
                }
                
                // 번들 정리
                if let Err(e) = bundle_manager.cleanup_expired_bundles().await {
                    error!("❌ 번들 정리 실패: {}", e);
                }
            }
        });
        
        // 메인 루프 - 서쳐가 실행되는 동안 대기
        info!("🎯 SearcherCore가 성공적으로 시작되었습니다!");
        
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 멤풀 모니터 상태 확인
            if !self.mempool_monitor.is_running().await {
                warn!("⚠️ 멤풀 모니터가 중지됨");
                break;
            }
            
            // 실행 상태 확인
            if !self.is_running.load(Ordering::SeqCst) {
                info!("🛑 종료 신호 수신됨");
                break;
            }
        }
        
        Ok(())
    }

    /// 서쳐 중지
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 SearcherCore 중지 중...");
        
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("⚠️ SearcherCore가 이미 중지됨");
            return Ok(());
        }
        
        self.is_running.store(false, Ordering::SeqCst);
        
        // 모든 전략 중지
        self.strategy_manager.stop_all_strategies().await?;
        
        // 멤풀 모니터 중지
        self.mempool_monitor.stop().await?;
        
        info!("✅ SearcherCore 중지됨");
        Ok(())
    }

    /// 서쳐 상태 조회
    pub async fn get_status(&self) -> Result<SearcherStatus> {
        let metrics = self.performance_tracker.get_metrics().await;
        let mempool_stats = self.mempool_monitor.get_stats().await;
        let bundle_stats = self.bundle_manager.get_bundle_stats().await;
        
        // 마이크로아비트래지 상태 조회 (있는 경우)
        let micro_arbitrage_status = if let Some(ref orchestrator) = self.micro_arbitrage_orchestrator {
            let guard = orchestrator.lock().await;
            Some(guard.get_comprehensive_status().await)
        } else {
            None
        };
        
        Ok(SearcherStatus {
            is_running: self.is_running.load(Ordering::SeqCst),
            active_opportunities: mempool_stats.transactions_processed as usize,
            submitted_bundles: bundle_stats.total_submitted as usize,
            performance_metrics: metrics.clone(),
            uptime_seconds: metrics.uptime,
            micro_arbitrage_status,
        })
    }

    /// 성능 메트릭 조회
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.performance_tracker.get_metrics().await
    }

    /// 상세 통계 조회
    pub async fn get_detailed_stats(&self) -> super::performance_tracker::DetailedStats {
        self.performance_tracker.get_detailed_stats().await
    }

    /// 성능 리포트 생성
    pub async fn generate_performance_report(&self) -> Result<super::performance_tracker::PerformanceReport> {
        self.performance_tracker.generate_performance_report().await
    }

    /// 알림 조회
    pub async fn get_alerts(&self, unacknowledged_only: bool) -> Vec<super::performance_tracker::Alert> {
        self.performance_tracker.get_alerts(unacknowledged_only).await
    }

    /// 알림 확인 처리
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        self.performance_tracker.acknowledge_alert(alert_id).await
    }

    /// 멤풀 상태 조회
    pub async fn get_mempool_status(&self) -> Result<super::mempool_monitor::MempoolStatus> {
        self.mempool_monitor.get_mempool_status().await
    }

    /// 전략 통계 조회
    pub async fn get_strategy_stats(&self) -> std::collections::HashMap<crate::types::StrategyType, crate::strategies::manager::StrategyStats> {
        self.strategy_manager.get_strategy_stats().await
    }

    /// 번들 통계 조회
    pub async fn get_bundle_stats(&self) -> super::bundle_manager::BundleStats {
        self.bundle_manager.get_bundle_stats().await
    }

    /// 제출된 번들 목록 조회(노출용)
    pub async fn list_submitted_bundles(&self) -> Vec<Bundle> {
        self.bundle_manager.get_submitted_bundles().await
    }

    /// 대기 번들 목록 조회(노출용)
    pub async fn list_pending_bundles(&self) -> Vec<Bundle> {
        self.bundle_manager.get_pending_bundles().await
    }

    /// 통계 초기화
    pub async fn reset_stats(&self) -> Result<()> {
        self.performance_tracker.reset_stats().await
    }

    /// 전략 활성화/비활성화
    pub async fn set_strategy_enabled(&self, strategy_type: crate::types::StrategyType, enabled: bool) -> Result<()> {
        self.strategy_manager.set_strategy_enabled(strategy_type, enabled).await
    }

    /// 트랜잭션 검색
    pub async fn search_transactions(&self, criteria: super::mempool_monitor::TransactionSearchCriteria) -> Vec<Transaction> {
        self.mempool_monitor.search_transactions(criteria).await
    }
}

impl Clone for SearcherCore {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            provider: Arc::clone(&self.provider),
            is_running: Arc::clone(&self.is_running),
            metrics: Arc::clone(&self.metrics),
            strategy_manager: Arc::clone(&self.strategy_manager),
            bundle_manager: Arc::clone(&self.bundle_manager),
            mempool_monitor: Arc::clone(&self.mempool_monitor),
            performance_tracker: Arc::clone(&self.performance_tracker),
            micro_arbitrage_orchestrator: self.micro_arbitrage_orchestrator.as_ref().map(Arc::clone),
            tx_sender: self.tx_sender.clone(),
            opportunity_sender: self.opportunity_sender.clone(),
            bundle_sender: self.bundle_sender.clone(),
        }
    }
}

impl std::fmt::Debug for SearcherCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearcherCore")
            .field("config", &"Arc<Config>")
            .field("provider", &"Arc<Provider<Ws>>")
            .field("is_running", &self.is_running.load(Ordering::SeqCst))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use ethers::types::{H256, H160, U256};
    // use chrono::Utc;

    #[tokio::test]
    async fn test_searcher_core_creation() {
        let _config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let core = SearcherCore::new(config, provider).await;
        // assert!(core.is_ok());
    }

    #[tokio::test]
    async fn test_searcher_status() {
        let _config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let core = SearcherCore::new(config, provider).await.unwrap();
        // let status = core.get_status().await;
        // assert!(status.is_ok());
    }
}
