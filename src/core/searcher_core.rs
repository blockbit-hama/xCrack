use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug, error, warn};
use ethers::types::U256;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{RwLock, mpsc};
use std::time::{Instant, Duration};
use ethers::providers::{Provider, Ws};

use crate::config::Config;
use crate::types::{PerformanceMetrics, Transaction, Opportunity, Bundle};
use super::{
    StrategyManager, 
    BundleManager, 
    CoreMempoolMonitor, 
    PerformanceTracker
};

#[derive(Debug, Clone)]
pub struct SearcherStatus {
    pub is_running: bool,
    pub active_opportunities: usize,
    pub submitted_bundles: usize,
    pub performance_metrics: PerformanceMetrics,
    pub uptime_seconds: u64,
}

pub struct SearcherCore {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    is_running: Arc<AtomicBool>,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    
    // 핵심 컴포넌트들
    strategy_manager: Arc<StrategyManager>,
    bundle_manager: Arc<BundleManager>,
    mempool_monitor: Arc<CoreMempoolMonitor>,
    performance_tracker: Arc<PerformanceTracker>,
    
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
            total_profit: U256::zero(),
            total_gas_spent: U256::zero(),
            avg_analysis_time: 0.0,
            avg_submission_time: 0.0,
            success_rate: 0.0,
            uptime: 0,
        };
        
        // 핵심 컴포넌트들 초기화
        let strategy_manager = Arc::new(StrategyManager::new(Arc::clone(&config)).await?);
        let bundle_manager = Arc::new(BundleManager::new(Arc::clone(&config)).await?);
        let mempool_monitor = Arc::new(CoreMempoolMonitor::new(Arc::clone(&config), Arc::clone(&provider)).await?);
        let performance_tracker = Arc::new(PerformanceTracker::new(Arc::clone(&config)).await?);
        
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
        
        // 채널 전송자들 저장
        let mut self_mut = unsafe { &mut *(self as *const _ as *mut Self) };
        self_mut.tx_sender = Some(tx_sender);
        self_mut.opportunity_sender = Some(opportunity_sender);
        self_mut.bundle_sender = Some(bundle_sender);
        
        // 3. 멤풀 모니터링 시작
        info!("📡 멤풀 모니터링 시작 중...");
        self.mempool_monitor.start(self_mut.tx_sender.as_ref().unwrap().clone()).await?;
        
        // 4. 메인 처리 루프 실행
        info!("🔄 메인 처리 루프 시작 중...");
        self.run_main_loop(
            tx_receiver,
            opportunity_receiver,
            bundle_receiver
        ).await?;
        
        Ok(())
    }

    /// 메인 처리 루프
    async fn run_main_loop(
        &self,
        mut tx_receiver: mpsc::UnboundedReceiver<Transaction>,
        mut opportunity_receiver: mpsc::UnboundedReceiver<Opportunity>,
        mut bundle_receiver: mpsc::UnboundedReceiver<Bundle>,
    ) -> Result<()> {
        info!("🎯 메인 처리 루프 실행 중...");
        
        // 트랜잭션 처리 태스크
        let strategy_manager = Arc::clone(&self.strategy_manager);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let opportunity_sender = self.opportunity_sender.as_ref().unwrap().clone();
        
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
                    if let Err(e) = opportunity_sender.send(opportunity) {
                        error!("❌ 기회 전송 실패: {}", e);
                    }
                }
            }
        });
        
        // 기회 검증 및 번들 생성 태스크
        let strategy_manager = Arc::clone(&self.strategy_manager);
        let bundle_manager = Arc::clone(&self.bundle_manager);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        let bundle_sender = self.bundle_sender.as_ref().unwrap().clone();
        
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
                            if let Err(e) = bundle_sender.send(bundle) {
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
        
        Ok(SearcherStatus {
            is_running: self.is_running.load(Ordering::SeqCst),
            active_opportunities: mempool_stats.transactions_processed as usize,
            submitted_bundles: bundle_stats.total_submitted as usize,
            performance_metrics: metrics,
            uptime_seconds: metrics.uptime,
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
    pub async fn get_strategy_stats(&self) -> std::collections::HashMap<crate::types::StrategyType, super::strategy_manager::StrategyStats> {
        self.strategy_manager.get_strategy_stats().await
    }

    /// 번들 통계 조회
    pub async fn get_bundle_stats(&self) -> super::bundle_manager::BundleStats {
        self.bundle_manager.get_bundle_stats().await
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
    use ethers::types::{H256, H160, U256};
    use chrono::Utc;

    #[tokio::test]
    async fn test_searcher_core_creation() {
        let config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let core = SearcherCore::new(config, provider).await;
        // assert!(core.is_ok());
    }

    #[tokio::test]
    async fn test_searcher_status() {
        let config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let core = SearcherCore::new(config, provider).await.unwrap();
        // let status = core.get_status().await;
        // assert!(status.is_ok());
    }
}
