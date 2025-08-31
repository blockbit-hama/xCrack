use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use anyhow::Result;
use tracing::{info, warn};
use alloy::primitives::{Address, U256};
use ethers::providers::{Provider, Ws};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::protocols::{MultiProtocolScanner, LiquidatableUser};
use crate::dex::{DexAggregator, DexType};
use crate::utils::profitability::{ProfitabilityCalculator, LiquidationProfitabilityAnalysis};
use crate::mev::{FlashbotsClient, BundleStatus};
use super::liquidation_bundle_builder::{LiquidationBundleBuilder, LiquidationBundle, LiquidationScenario};

/// 청산 전략 매니저 - 전체 청산 전략 조율
pub struct LiquidationStrategyManager {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    protocol_scanner: Arc<Mutex<MultiProtocolScanner>>,
    profitability_calculator: ProfitabilityCalculator,
    bundle_builder: LiquidationBundleBuilder,
    flashbots_client: FlashbotsClient,
    dex_aggregators: HashMap<DexType, Box<dyn DexAggregator>>,
    
    // 성능 메트릭
    performance_metrics: Arc<tokio::sync::RwLock<PerformanceMetrics>>,
    
    // 실행 상태
    is_running: Arc<tokio::sync::RwLock<bool>>,
}

/// 청산 기회
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationOpportunity {
    pub user: LiquidatableUser,
    pub liquidation_amount: U256,
    pub profitability_analysis: LiquidationProfitabilityAnalysis,
    pub priority_score: f64,
    pub estimated_execution_time: Duration,
    pub confidence_score: f64,
}

/// 성능 메트릭
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceMetrics {
    pub total_opportunities_detected: u64,
    pub profitable_opportunities: u64,
    pub bundles_created: u64,
    pub bundles_submitted: u64,
    pub bundles_included: u64,
    pub total_profit: U256,
    pub avg_profit_per_liquidation: U256,
    pub success_rate: f64,
    pub avg_execution_time_ms: u64,
    pub last_scan_duration_ms: u64,
}

impl LiquidationStrategyManager {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        protocol_scanner: Arc<Mutex<MultiProtocolScanner>>,
        profitability_calculator: ProfitabilityCalculator,
        bundle_builder: LiquidationBundleBuilder,
        flashbots_client: FlashbotsClient,
        dex_aggregators: HashMap<DexType, Box<dyn DexAggregator>>,
    ) -> Result<Self> {
        info!("🎯 Initializing Liquidation Strategy Manager...");
        
        let performance_metrics = Arc::new(tokio::sync::RwLock::new(PerformanceMetrics::default()));
        let is_running = Arc::new(tokio::sync::RwLock::new(false));
        
        Ok(Self {
            config,
            provider,
            protocol_scanner,
            profitability_calculator,
            bundle_builder,
            flashbots_client,
            dex_aggregators,
            performance_metrics,
            is_running,
        })
    }
    
    /// 메인 청산 전략 실행 루프
    pub async fn run_liquidation_strategy(&mut self) -> Result<()> {
        info!("🚀 Starting liquidation strategy execution...");
        
        // 실행 상태 설정
        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }
        
        while *self.is_running.read().await {
            let scan_start = std::time::Instant::now();
            
            // 1. 청산 기회 탐지
            let opportunities = self.detect_liquidation_opportunities().await?;
            
            // 2. 수익성 있는 기회 필터링
            let profitable_opportunities = self.filter_profitable_opportunities(opportunities).await?;
            
            // 3. 우선순위별 정렬
            let sorted_opportunities = self.sort_opportunities_by_priority(profitable_opportunities);
            
            // 4. 최고 우선순위 기회 실행
            if let Some(best_opportunity) = sorted_opportunities.first() {
                self.execute_liquidation_opportunity(best_opportunity.clone()).await?;
            }
            
            // 5. 성능 메트릭 업데이트
            let scan_duration = scan_start.elapsed();
            self.update_performance_metrics(scan_duration).await;
            
            // 6. 다음 스캔까지 대기
            sleep(Duration::from_secs(5)).await;
        }
        
        info!("🛑 Liquidation strategy execution stopped");
        Ok(())
    }
    
    /// 청산 기회 탐지
    async fn detect_liquidation_opportunities(&self) -> Result<Vec<LiquidationOpportunity>> {
        let start_time = std::time::Instant::now();
        
        // 모든 프로토콜에서 청산 가능한 사용자 스캔
        let liquidatable_users = self.protocol_scanner.lock().await.scan_all_protocols().await?;
        let total_users: usize = liquidatable_users.values().map(|users| users.len()).sum();
        
        info!("🔍 Found {} liquidatable users across all protocols", total_users);
        
        let mut opportunities = Vec::new();
        
        // 각 사용자에 대해 청산 기회 분석
        for (_protocol_type, users) in liquidatable_users {
            for user in users {
                // 최적 청산 금액 계산
                let optimal_liquidation_amount = self.calculate_optimal_liquidation_amount(&user).await?;
                
                // 수익성 분석
                let empty_swap_quotes = HashMap::new(); // TODO: 실제 스왑 시세 데이터 연결
                let eth_price_usd = 2000.0; // TODO: 실제 ETH 가격 데이터 연결
                let profitability_analysis = self.profitability_calculator
                    .analyze_liquidation_profitability(&user, &empty_swap_quotes, eth_price_usd)
                    .await?;
                
                // 우선순위 점수 계산
                let priority_score = self.calculate_priority_score(&user, &profitability_analysis);
                
                // 신뢰도 점수 계산
                let confidence_score = self.calculate_confidence_score(&user, &profitability_analysis);
                
                let opportunity = LiquidationOpportunity {
                    user,
                    liquidation_amount: optimal_liquidation_amount,
                    profitability_analysis,
                    priority_score,
                    estimated_execution_time: Duration::from_secs(12), // 1블록
                    confidence_score,
                };
                
                opportunities.push(opportunity);
            }
        }
        
        let duration = start_time.elapsed();
        info!("✅ Opportunity detection completed in {:?}, found {} opportunities", 
              duration, opportunities.len());
        
        // 메트릭 업데이트
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.total_opportunities_detected += opportunities.len() as u64;
            metrics.last_scan_duration_ms = duration.as_millis() as u64;
        }
        
        Ok(opportunities)
    }
    
    /// 수익성 있는 기회 필터링
    async fn filter_profitable_opportunities(
        &self,
        opportunities: Vec<LiquidationOpportunity>,
    ) -> Result<Vec<LiquidationOpportunity>> {
        let min_profit_threshold_usd = 200.0; // $200 minimum profit (assuming $2000 ETH = 0.1 ETH)
        let total_opportunities = opportunities.len();
        
        let profitable_opportunities: Vec<LiquidationOpportunity> = opportunities
            .into_iter()
            .filter(|opp| {
                opp.profitability_analysis.is_profitable && 
                opp.profitability_analysis.estimated_net_profit_usd > min_profit_threshold_usd
            })
            .collect();
        
        info!("💰 Filtered {} profitable opportunities from {} total", 
              profitable_opportunities.len(), total_opportunities);
        
        // 메트릭 업데이트
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.profitable_opportunities += profitable_opportunities.len() as u64;
        }
        
        Ok(profitable_opportunities)
    }
    
    /// 우선순위별 정렬
    fn sort_opportunities_by_priority(
        &self,
        mut opportunities: Vec<LiquidationOpportunity>,
    ) -> Vec<LiquidationOpportunity> {
        opportunities.sort_by(|a, b| {
            b.priority_score.partial_cmp(&a.priority_score).unwrap()
        });
        
        opportunities
    }
    
    /// 청산 기회 실행
    async fn execute_liquidation_opportunity(&mut self, opportunity: LiquidationOpportunity) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        info!("🎯 Executing liquidation opportunity for user: {:?}", opportunity.user.address);
        
        // 1. 최적 스왑 견적 생성
        let swap_quote = self.get_best_swap_quote(&opportunity).await?;
        
        // 2. 청산 시나리오 생성
        let scenario = LiquidationScenario {
            user: opportunity.user.clone(),
            liquidation_amount: ethers::types::U256::from_little_endian(&opportunity.liquidation_amount.to_le_bytes::<32>()),
            profitability_analysis: opportunity.profitability_analysis.clone(),
            swap_quote,
            execution_priority: self.determine_execution_priority(&opportunity),
            estimated_gas: 500_000, // TODO: 정확한 가스 추정
            max_gas_price: ethers::types::U256::from(200_000_000_000u64), // 200 gwei
        };
        
        // 3. 청산 번들 생성
        let liquidation_bundle = self.bundle_builder.build_liquidation_bundle(scenario).await?;
        
        // 4. MEV 번들 제출
        let submission_result = self.submit_liquidation_bundle(liquidation_bundle).await?;
        
        // 5. 결과 처리
        self.handle_submission_result(submission_result, &opportunity).await?;
        
        let execution_time = start_time.elapsed();
        info!("✅ Liquidation execution completed in {:?}", execution_time);
        
        // 메트릭 업데이트
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.bundles_created += 1;
            metrics.bundles_submitted += 1;
            metrics.avg_execution_time_ms = execution_time.as_millis() as u64;
        }
        
        Ok(())
    }
    
    /// 최적 청산 금액 계산
    async fn calculate_optimal_liquidation_amount(&self, user: &LiquidatableUser) -> Result<U256> {
        // TODO: 실제 최적화 로직 구현
        // 현재는 간단한 휴리스틱 사용
        
        let total_debt = user.account_data.total_debt_usd;
        let max_liquidatable = total_debt * 0.5; // 50% 청산
        
        // USD를 토큰 단위로 변환 (간단화)
        let liquidation_amount = U256::from((max_liquidatable * 1e18) as u64);
        
        Ok(liquidation_amount)
    }
    
    /// 우선순위 점수 계산
    fn calculate_priority_score(&self, user: &LiquidatableUser, analysis: &LiquidationProfitabilityAnalysis) -> f64 {
        let profit_score = analysis.estimated_net_profit_usd / 1e18; // Already in USD
        let urgency_score = if user.account_data.health_factor < 0.95 { 1.0 } else { 0.5 };
        let size_score = user.account_data.total_debt_usd / 1_000_000.0; // 100만 달러 기준
        
        profit_score * 0.5 + urgency_score * 0.3 + size_score * 0.2
    }
    
    /// 신뢰도 점수 계산
    fn calculate_confidence_score(&self, user: &LiquidatableUser, analysis: &LiquidationProfitabilityAnalysis) -> f64 {
        let profit_margin = analysis.profit_margin_percent;
        let health_factor = user.account_data.health_factor;
        
        // 수익 마진이 높고 헬스팩터가 낮을수록 높은 신뢰도
        profit_margin * (2.0 - health_factor)
    }
    
    /// 최적 스왑 견적 생성
    async fn get_best_swap_quote(&self, opportunity: &LiquidationOpportunity) -> Result<crate::dex::SwapQuote> {
        // TODO: 실제 DEX 어그리게이터에서 최적 견적 조회
        // 현재는 더미 데이터 반환
        
        Ok(crate::dex::SwapQuote {
            aggregator: DexType::ZeroX,
            sell_token: opportunity.user.collateral_positions[0].asset,
            buy_token: opportunity.user.debt_positions[0].asset,
            sell_amount: opportunity.liquidation_amount,
            buy_amount: opportunity.liquidation_amount * U256::from(105) / U256::from(100), // 5% 보너스
            buy_amount_min: opportunity.liquidation_amount,
            router_address: Address::ZERO, // TODO: 실제 라우터 주소
            calldata: vec![],
            allowance_target: Address::ZERO,
            gas_estimate: 200_000,
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            price_impact: 0.01,
            sources: vec![],
            estimated_execution_time_ms: 1000,
            quote_timestamp: chrono::Utc::now(),
        })
    }
    
    /// 실행 우선순위 결정
    fn determine_execution_priority(&self, opportunity: &LiquidationOpportunity) -> crate::mev::PriorityLevel {
        if opportunity.user.account_data.health_factor < 0.95 {
            crate::mev::PriorityLevel::Critical
        } else if opportunity.user.account_data.health_factor < 0.98 {
            crate::mev::PriorityLevel::High
        } else {
            crate::mev::PriorityLevel::Medium
        }
    }
    
    /// 청산 번들 제출
    async fn submit_liquidation_bundle(&self, _bundle: LiquidationBundle) -> Result<BundleStatus> {
        info!("📤 Submitting liquidation bundle to Flashbots...");
        
        // TODO: 실제 Flashbots 제출 로직 구현
        // 현재는 더미 응답 반환
        
        Ok(BundleStatus::Pending)
    }
    
    /// 제출 결과 처리
    async fn handle_submission_result(
        &self,
        result: BundleStatus,
        opportunity: &LiquidationOpportunity,
    ) -> Result<()> {
        match result {
            BundleStatus::Included(_) => {
                info!("🎉 Liquidation bundle included in block!");
                
                // 메트릭 업데이트
                {
                    let mut metrics = self.performance_metrics.write().await;
                    metrics.bundles_included += 1;
                    metrics.total_profit += alloy::primitives::U256::from((opportunity.profitability_analysis.estimated_net_profit_usd * 1e18) as u64);
                    metrics.avg_profit_per_liquidation = metrics.total_profit / U256::from(metrics.bundles_included);
                    metrics.success_rate = metrics.bundles_included as f64 / metrics.bundles_submitted as f64;
                }
            },
            BundleStatus::Rejected(_) => {
                warn!("❌ Liquidation bundle rejected");
            },
            BundleStatus::Pending => {
                info!("⏳ Liquidation bundle submitted, waiting for inclusion...");
            },
            BundleStatus::Timeout => {
                warn!("⏰ Liquidation bundle timed out");
            },
            BundleStatus::Replaced => {
                warn!("🔄 Liquidation bundle was replaced by higher bidder");
            },
        }
        
        Ok(())
    }
    
    /// 성능 메트릭 업데이트
    async fn update_performance_metrics(&self, scan_duration: Duration) {
        let mut metrics = self.performance_metrics.write().await;
        metrics.last_scan_duration_ms = scan_duration.as_millis() as u64;
    }
    
    /// 전략 중지
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 Stopping liquidation strategy...");
        
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        
        Ok(())
    }
    
    /// 성능 메트릭 조회
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_strategy_manager_creation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_opportunity_detection() {
        // TODO: 테스트 구현
        assert!(true);
    }
}
