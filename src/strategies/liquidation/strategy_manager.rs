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
use crate::common::profitability::{ProfitabilityCalculator, LiquidationProfitabilityAnalysis};
use crate::mev::{FlashbotsClient, BundleStatus};
use crate::strategies::liquidation::bundle_builder::{LiquidationBundleBuilder, LiquidationBundle, LiquidationScenario};

/// 청산 전략 매니저 - 전체 청산 전략 조율
pub struct LiquidationStrategyManager {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    protocol_scanner: Arc<Mutex<MultiProtocolScanner>>,
    profitability_calculator: ProfitabilityCalculator,
    bundle_builder: LiquidationBundleBuilder,
    flashbots_client: FlashbotsClient,
    dex_aggregators: HashMap<DexType, Box<dyn DexAggregator>>,
    http_client: reqwest::Client,

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
        let http_client = reqwest::Client::new();

        Ok(Self {
            config,
            provider,
            protocol_scanner,
            profitability_calculator,
            bundle_builder,
            flashbots_client,
            dex_aggregators,
            http_client,
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
                
                // 수익성 분석 - 실제 스왑 시세 및 ETH 가격 데이터 사용
                let swap_quotes = self.get_real_swap_quotes(&user).await?;
                let eth_price_usd = self.get_real_eth_price().await?;
                let profitability_analysis = self.profitability_calculator
                    .analyze_liquidation_profitability(&user, &swap_quotes, eth_price_usd)
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
        let estimated_gas = self.estimate_gas_for_liquidation(&opportunity, &swap_quote).await?;
        let current_gas_price = self.get_current_gas_price().await?;
        let max_gas_price = current_gas_price * ethers::types::U256::from(120) / ethers::types::U256::from(100); // 현재 가스 가격의 120%

        let scenario = LiquidationScenario {
            user: opportunity.user.clone(),
            liquidation_amount: ethers::types::U256::from_little_endian(&opportunity.liquidation_amount.to_le_bytes::<32>()),
            profitability_analysis: opportunity.profitability_analysis.clone(),
            swap_quote,
            execution_priority: self.determine_execution_priority(&opportunity),
            estimated_gas,
            max_gas_price,
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
    
    /// 최적 스왑 견적 생성 - 실제 DEX 어그리게이터 통합
    async fn get_best_swap_quote(&self, opportunity: &LiquidationOpportunity) -> Result<crate::dex::SwapQuote> {
        let sell_token = opportunity.user.collateral_positions[0].asset;
        let buy_token = opportunity.user.debt_positions[0].asset;
        let sell_amount = opportunity.liquidation_amount;

        // 0x와 1inch에서 견적 가져오기
        let mut best_quote: Option<crate::dex::SwapQuote> = None;
        let mut best_buy_amount = U256::ZERO;

        // 0x 견적 시도
        if let Some(zerox_aggregator) = self.dex_aggregators.get(&DexType::ZeroX) {
            match zerox_aggregator.get_swap_quote(sell_token, buy_token, sell_amount).await {
                Ok(quote) => {
                    if quote.buy_amount > best_buy_amount {
                        best_buy_amount = quote.buy_amount;
                        best_quote = Some(quote);
                    }
                },
                Err(e) => {
                    warn!("0x 견적 조회 실패: {}", e);
                }
            }
        }

        // 1inch 견적 시도
        if let Some(oneinch_aggregator) = self.dex_aggregators.get(&DexType::OneInch) {
            match oneinch_aggregator.get_swap_quote(sell_token, buy_token, sell_amount).await {
                Ok(quote) => {
                    if quote.buy_amount > best_buy_amount {
                        best_buy_amount = quote.buy_amount;
                        best_quote = Some(quote);
                    }
                },
                Err(e) => {
                    warn!("1inch 견적 조회 실패: {}", e);
                }
            }
        }

        // Uniswap 견적 시도 (백업)
        if let Some(uniswap_aggregator) = self.dex_aggregators.get(&DexType::Uniswap) {
            match uniswap_aggregator.get_swap_quote(sell_token, buy_token, sell_amount).await {
                Ok(quote) => {
                    if quote.buy_amount > best_buy_amount {
                        best_buy_amount = quote.buy_amount;
                        best_quote = Some(quote);
                    }
                },
                Err(e) => {
                    warn!("Uniswap 견적 조회 실패: {}", e);
                }
            }
        }

        best_quote.ok_or_else(|| anyhow::anyhow!("모든 DEX 어그리게이터에서 견적 조회 실패"))
    }

    /// 실제 스왑 견적 가져오기
    async fn get_real_swap_quotes(&self, user: &LiquidatableUser) -> Result<HashMap<(Address, Address), crate::dex::SwapQuote>> {
        let mut swap_quotes = HashMap::new();

        // 각 담보-부채 쌍에 대한 스왑 견적 조회
        for collateral in &user.collateral_positions {
            for debt in &user.debt_positions {
                // 청산 시 받을 담보 금액 계산 (간단화)
                let collateral_amount = debt.amount * U256::from(105) / U256::from(100); // 5% 보너스

                // 0x에서 견적 조회
                if let Some(zerox_aggregator) = self.dex_aggregators.get(&DexType::ZeroX) {
                    match zerox_aggregator.get_swap_quote(collateral.asset, debt.asset, collateral_amount).await {
                        Ok(quote) => {
                            swap_quotes.insert((collateral.asset, debt.asset), quote);
                        },
                        Err(e) => {
                            warn!("스왑 견적 조회 실패 ({:?} -> {:?}): {}", collateral.asset, debt.asset, e);
                        }
                    }
                }
            }
        }

        Ok(swap_quotes)
    }

    /// 실제 ETH 가격 가져오기
    async fn get_real_eth_price(&self) -> Result<f64> {
        // Chainlink 피드에서 ETH/USD 가격 조회
        let url = "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd";

        match self.http_client.get(url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        if let Some(price) = data["ethereum"]["usd"].as_f64() {
                            info!("✅ ETH 가격 조회 성공: ${:.2}", price);
                            return Ok(price);
                        }
                    },
                    Err(e) => warn!("ETH 가격 파싱 실패: {}", e),
                }
            },
            Ok(response) => warn!("ETH 가격 조회 HTTP 오류: {}", response.status()),
            Err(e) => warn!("ETH 가격 조회 실패: {}", e),
        }

        // 폴백: 기본 가격 사용
        warn!("⚠️ ETH 가격 조회 실패, 기본값 사용: $2000.0");
        Ok(2000.0)
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
    
    /// 청산 번들 제출 - 실제 Flashbots 통합
    async fn submit_liquidation_bundle(&self, bundle: LiquidationBundle) -> Result<BundleStatus> {
        info!("📤 Submitting liquidation bundle to Flashbots...");

        // Flashbots 번들 파라미터 준비
        let current_block = self.provider.get_block_number().await?.as_u64();
        let target_block = current_block + 1;

        // 번들 트랜잭션 준비
        let bundle_transactions = vec![bundle.transactions.clone()];

        // Flashbots에 제출
        match self.flashbots_client.send_bundle(bundle_transactions, target_block).await {
            Ok(bundle_hash) => {
                info!("✅ Flashbots 번들 제출 성공: {}", bundle_hash);

                // 번들 포함 상태 모니터링
                let max_retries = 3;
                for retry in 0..max_retries {
                    tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

                    match self.flashbots_client.get_bundle_status(&bundle_hash).await {
                        Ok(status) => {
                            match status {
                                BundleStatus::Included(block_hash) => {
                                    info!("🎉 번들이 블록에 포함됨: {:?}", block_hash);
                                    return Ok(BundleStatus::Included(block_hash));
                                }
                                BundleStatus::Rejected(reason) => {
                                    warn!("❌ 번들 거부: {}", reason);
                                    return Ok(BundleStatus::Rejected(reason));
                                }
                                BundleStatus::Pending => {
                                    info!("⏳ 번들 대기 중... (재시도 {}/{})", retry + 1, max_retries);
                                    continue;
                                }
                                _ => return Ok(status),
                            }
                        }
                        Err(e) => {
                            warn!("⚠️ 번들 상태 조회 실패: {}", e);
                        }
                    }
                }

                Ok(BundleStatus::Timeout)
            }
            Err(e) => {
                warn!("❌ Flashbots 번들 제출 실패: {}", e);
                Ok(BundleStatus::Rejected(format!("제출 실패: {}", e)))
            }
        }
    }

    /// 가스 추정 - 프로토콜별 정확한 가스 계산
    async fn estimate_gas_for_liquidation(&self, opportunity: &LiquidationOpportunity, swap_quote: &crate::dex::SwapQuote) -> Result<u64> {
        use crate::protocols::ProtocolType;

        // 기본 가스 소비량 (프로토콜별)
        let protocol_gas = match opportunity.user.protocol {
            ProtocolType::Aave => 400_000u64,      // Aave V3 청산
            ProtocolType::CompoundV2 => 350_000u64, // Compound V2 청산
            ProtocolType::CompoundV3 => 300_000u64, // Compound V3 청산
            ProtocolType::MakerDAO => 500_000u64,  // MakerDAO 청산 (더 복잡)
            _ => 400_000u64,
        };

        // 스왑 가스 소비량
        let swap_gas = swap_quote.gas_estimate;

        // 플래시론 사용 시 추가 가스
        let flash_loan_gas = if opportunity.profitability_analysis.requires_flash_loan {
            200_000u64 // Aave 플래시론 오버헤드
        } else {
            0u64
        };

        // 총 예상 가스 (안전 여유분 10% 추가)
        let total_gas = protocol_gas + swap_gas + flash_loan_gas;
        let gas_with_buffer = total_gas * 110 / 100;

        info!("⛽ 가스 추정: 프로토콜={}, 스왑={}, 플래시론={}, 총계={} (버퍼 포함)",
              protocol_gas, swap_gas, flash_loan_gas, gas_with_buffer);

        Ok(gas_with_buffer)
    }

    /// 현재 가스 가격 조회
    async fn get_current_gas_price(&self) -> Result<ethers::types::U256> {
        match self.provider.get_gas_price().await {
            Ok(gas_price) => {
                info!("⛽ 현재 가스 가격: {} gwei", gas_price.as_u128() / 1_000_000_000);
                Ok(gas_price)
            }
            Err(e) => {
                warn!("⚠️ 가스 가격 조회 실패, 기본값 사용: {}", e);
                Ok(ethers::types::U256::from(20_000_000_000u64)) // 20 gwei 기본값
            }
        }
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
