//! Multi-Asset Arbitrage 통합 관리자
//!
//! 모든 다중자산 아비트리지 컴포넌트를 조율하고 관리합니다.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use tokio::sync::Mutex;
use tracing::{info, error};
use ethers::providers::{Provider, Ws};
use async_trait::async_trait;

use crate::config::Config;
use crate::types::{Transaction, Opportunity, StrategyType, Bundle};
use crate::Strategy;
use crate::adapters::{DexAdapterFactory, AdapterSelector, AdapterSelectionStrategy};
use crate::adapters::factory::AdapterConfig;

use super::types::*;
use super::opportunity_detector::OpportunityDetector;
use super::execution_engine::ExecutionEngine;
use super::flashloan_executor::AaveFlashLoanExecutor;

/// 다중자산 아비트리지 통합 관리자
pub struct MultiAssetArbitrageManager {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    enabled: Arc<AtomicBool>,

    // 핵심 컴포넌트들
    opportunity_detector: Arc<OpportunityDetector>,
    execution_engine: Arc<ExecutionEngine>,
    flashloan_executor: Arc<AaveFlashLoanExecutor>,

    // 통계
    stats: Arc<Mutex<MultiAssetArbitrageStats>>,
}

impl MultiAssetArbitrageManager {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🔄 Multi-Asset Arbitrage 관리자 초기화 중...");

        // Provider에서 wallet 가져오기
        let private_key = std::env::var("PRIVATE_KEY")
            .map_err(|_| anyhow::anyhow!("PRIVATE_KEY not set"))?;
        let wallet: ethers::signers::LocalWallet = private_key.parse()?;

        // DEX 어댑터 초기화
        let mut adapter_factory = DexAdapterFactory::new(
            AdapterConfig::default(),
            config.blockchain.primary_network.chain_id as u32,
        );
        adapter_factory.initialize_all_adapters()?;

        let adapter_selector = Arc::new(
            AdapterSelector::new(adapter_factory, AdapterSelectionStrategy::Hybrid)
        );

        // FlashLoan 실행자 초기화
        let flashloan_executor = Arc::new(
            AaveFlashLoanExecutor::new(provider.clone(), wallet)?
        );

        // Opportunity Detector 초기화
        let opportunity_detector = Arc::new(
            OpportunityDetector::new(
                config.clone(),
                provider.clone(),
                adapter_selector.clone(),
            )
        );

        // Execution Engine 초기화
        let execution_engine = Arc::new(
            ExecutionEngine::new(
                config.clone(),
                provider.clone(),
                adapter_selector.clone(),
                flashloan_executor.clone(),
            )
        );

        info!("✅ Multi-Asset Arbitrage 관리자 초기화 완료");

        Ok(Self {
            config,
            provider,
            enabled: Arc::new(AtomicBool::new(true)),
            opportunity_detector,
            execution_engine,
            flashloan_executor,
            stats: Arc::new(Mutex::new(MultiAssetArbitrageStats::default())),
        })
    }

    /// 기회 스캔 및 실행
    pub async fn scan_and_execute(&self) -> Result<usize> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Ok(0);
        }

        let mut executed_count = 0;

        // 삼각 아비트리지 기회 스캔
        let triangular_opps = self.opportunity_detector.scan_triangular_opportunities().await?;
        info!("🔺 삼각 아비트리지 기회: {}개", triangular_opps.len());

        for opp in triangular_opps {
            if opp.is_valid() {
                if self.execution_engine.execute(&opp).await? {
                    executed_count += 1;
                    self.update_stats_success(&opp).await;
                } else {
                    self.update_stats_failure().await;
                }
            }
        }

        // 복합 아비트리지 기회 스캔
        let complex_opps = self.opportunity_detector.scan_complex_opportunities().await?;
        info!("🔀 복합 아비트리지 기회: {}개", complex_opps.len());

        for opp in complex_opps {
            if opp.is_valid() {
                if self.execution_engine.execute(&opp).await? {
                    executed_count += 1;
                    self.update_stats_success(&opp).await;
                } else {
                    self.update_stats_failure().await;
                }
            }
        }

        Ok(executed_count)
    }

    /// 통계 업데이트 (성공)
    async fn update_stats_success(&self, opportunity: &MultiAssetArbitrageOpportunity) {
        let mut stats = self.stats.lock().await;
        stats.executed_trades += 1;
        stats.successful_trades += 1;
        stats.total_profit = stats.total_profit + opportunity.expected_profit;
        stats.total_volume = stats.total_volume + opportunity.flashloan_amounts[0].amount;

        // 전략 타입별 카운트
        match &opportunity.strategy_type {
            MultiAssetStrategyType::TriangularArbitrage { .. } => {
                stats.triangular_arbitrage_count += 1;
            }
            MultiAssetStrategyType::PositionMigration { .. } => {
                stats.position_migration_count += 1;
            }
            MultiAssetStrategyType::ComplexArbitrage { .. } => {
                stats.complex_arbitrage_count += 1;
            }
        }

        stats.success_rate = (stats.successful_trades as f64 / stats.executed_trades as f64) * 100.0;
    }

    /// 통계 업데이트 (실패)
    async fn update_stats_failure(&self) {
        let mut stats = self.stats.lock().await;
        stats.executed_trades += 1;
        stats.failed_trades += 1;
        stats.success_rate = (stats.successful_trades as f64 / stats.executed_trades as f64) * 100.0;
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> MultiAssetArbitrageStats {
        self.stats.lock().await.clone()
    }
}

/// Strategy trait 구현
#[async_trait]
impl Strategy for MultiAssetArbitrageManager {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::MultiAssetArbitrage
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    async fn start(&self) -> Result<()> {
        info!("🚀 Multi-Asset Arbitrage 전략 시작");
        self.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("🛑 Multi-Asset Arbitrage 전략 중지");
        self.enabled.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn analyze(&self, _transaction: &Transaction) -> Result<Vec<Opportunity>> {
        // Multi-Asset Arbitrage는 멤풀 트랜잭션을 분석하지 않음
        // 주기적인 스캔으로 기회를 찾음
        Ok(Vec::new())
    }

    async fn validate_opportunity(&self, _opportunity: &Opportunity) -> Result<bool> {
        // 검증 로직 (필요시 구현)
        Ok(true)
    }

    async fn create_bundle(&self, _opportunity: &Opportunity) -> Result<Bundle> {
        // Multi-Asset Arbitrage는 번들을 사용하지 않음 (공개 트랜잭션)
        Err(anyhow::anyhow!("Multi-Asset Arbitrage does not use bundles"))
    }
}
