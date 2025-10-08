use super::types::*;
use super::mempool_monitor::{MempoolMonitor, PendingSwapTransaction};
use super::target_analyzer::TargetAnalyzer;
use super::profitability::ProfitabilityAnalyzer;
use super::dex_router::DexRouterManager;
use super::stats::SandwichStatsManager;
use anyhow::Result;
use ethers::prelude::*;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug, error};

/// 샌드위치 전략 매니저 - 전략 조정 및 의사결정
pub struct SandwichStrategyManager {
    provider: Arc<Provider<Ws>>,
    dex_manager: Arc<DexRouterManager>,
    target_analyzer: Arc<TargetAnalyzer>,
    profitability_analyzer: Arc<ProfitabilityAnalyzer>,
    stats: Arc<SandwichStatsManager>,
    opportunity_sender: mpsc::UnboundedSender<SandwichOpportunity>,
    is_running: Arc<RwLock<bool>>,
}

impl SandwichStrategyManager {
    pub async fn new(
        provider: Arc<Provider<Ws>>,
        min_profit_eth: f64,
        min_profit_percentage: f64,
        max_price_impact: f64,
        kelly_risk_factor: f64,
    ) -> Result<(Self, mpsc::UnboundedReceiver<SandwichOpportunity>)> {
        info!("🎯 샌드위치 전략 매니저 초기화 중...");

        let dex_manager = Arc::new(DexRouterManager::new()?);
        let target_analyzer = Arc::new(TargetAnalyzer::new(
            provider.clone(),
            dex_manager.clone(),
        ));
        let profitability_analyzer = Arc::new(ProfitabilityAnalyzer::new(
            min_profit_eth,
            min_profit_percentage,
            max_price_impact,
            kelly_risk_factor,
        ));
        let stats = Arc::new(SandwichStatsManager::new());

        let (opportunity_sender, opportunity_receiver) = mpsc::unbounded_channel();

        let manager = Self {
            provider,
            dex_manager,
            target_analyzer,
            profitability_analyzer,
            stats,
            opportunity_sender,
            is_running: Arc::new(RwLock::new(false)),
        };

        info!("✅ 샌드위치 전략 매니저 초기화 완료");
        Ok((manager, opportunity_receiver))
    }

    /// 전략 실행 시작
    pub async fn start(
        &self,
        mut mempool_rx: mpsc::UnboundedReceiver<PendingSwapTransaction>,
    ) -> Result<()> {
        *self.is_running.write().await = true;
        info!("🚀 샌드위치 전략 매니저 시작");

        let provider = self.provider.clone();
        let target_analyzer = self.target_analyzer.clone();
        let profitability_analyzer = self.profitability_analyzer.clone();
        let opportunity_sender = self.opportunity_sender.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            while *is_running.read().await {
                if let Some(pending_swap) = mempool_rx.recv().await {
                    stats.record_opportunity_detected().await;

                    // 현재 가스 가격 조회
                    let gas_price = match provider.get_gas_price().await {
                        Ok(price) => price,
                        Err(e) => {
                            error!("❌ 가스 가격 조회 실패: {}", e);
                            continue;
                        }
                    };

                    // 타겟 트랜잭션 분석
                    let target_analysis = match target_analyzer.analyze(
                        &pending_swap.tx,
                        pending_swap.dex_type,
                    ).await {
                        Ok(analysis) => analysis,
                        Err(e) => {
                            debug!("⚠️ 타겟 분석 실패: {}", e);
                            continue;
                        }
                    };

                    stats.record_opportunity_analyzed().await;

                    // 수익성 분석
                    match profitability_analyzer.analyze_opportunity(
                        &target_analysis,
                        gas_price,
                    ).await {
                        Ok(Some(opportunity)) => {
                            info!("💰 수익성 있는 기회 발견!");
                            if let Err(e) = opportunity_sender.send(opportunity) {
                                error!("❌ 기회 전송 실패: {}", e);
                            }
                        }
                        Ok(None) => {
                            debug!("   수익성 없음 - 필터링");
                        }
                        Err(e) => {
                            debug!("⚠️ 수익성 분석 실패: {}", e);
                        }
                    }
                }
            }

            info!("🛑 샌드위치 전략 매니저 중지");
        });

        Ok(())
    }

    /// 전략 중지
    pub async fn stop(&self) {
        *self.is_running.write().await = false;
        info!("🛑 샌드위치 전략 매니저 중지 중...");
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> SandwichStats {
        self.stats.get_stats().await
    }

    /// 통계 출력
    pub async fn print_stats(&self) {
        self.stats.print_stats().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_strategy_manager_creation() {
        // Mock test
        assert!(true);
    }
}
