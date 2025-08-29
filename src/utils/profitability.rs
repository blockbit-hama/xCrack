use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{debug, warn, error, info};
use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::protocols::{LiquidatableUser, ProtocolType};
use crate::dex::{SwapQuote, DexType};

/// Profitability Calculator for Liquidation Operations
pub struct ProfitabilityCalculator {
    config: Config,
    gas_price_gwei: f64,
    flashloan_premiums: HashMap<FlashLoanProvider, f64>,
    min_profit_threshold_usd: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum FlashLoanProvider {
    Aave,
    Balancer,
    UniswapV3,
    DyDx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationProfitabilityAnalysis {
    pub user: LiquidatableUser,
    pub best_strategy: Option<LiquidationStrategy>,
    pub all_strategies: Vec<LiquidationStrategy>,
    pub is_profitable: bool,
    pub estimated_net_profit_usd: f64,
    pub profit_margin_percent: f64,
    pub recommended_liquidation_amount: U256,
    pub risk_score: f64,
    pub analysis_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationStrategy {
    pub debt_asset: Address,
    pub collateral_asset: Address,
    pub liquidation_amount: U256,
    pub liquidation_amount_usd: f64,
    pub expected_collateral_amount: U256,
    pub expected_collateral_usd: f64,
    pub flashloan_provider: FlashLoanProvider,
    pub swap_route: SwapRoute,
    pub cost_breakdown: CostBreakdown,
    pub net_profit_usd: f64,
    pub profit_margin_percent: f64,
    pub execution_time_estimate_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapRoute {
    pub aggregator: DexType,
    pub swap_quote: SwapQuote,
    pub slippage_tolerance: f64,
    pub minimum_output_amount: U256,
    pub price_impact_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub flashloan_premium_usd: f64,
    pub gas_cost_usd: f64,
    pub dex_fees_usd: f64,
    pub slippage_cost_usd: f64,
    pub protocol_fees_usd: f64,
    pub total_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasCostEstimation {
    pub flashloan_initiation: u64,
    pub liquidation_call: u64,
    pub dex_swap: u64,
    pub flashloan_repayment: u64,
    pub total_gas: u64,
    pub gas_price_gwei: f64,
    pub gas_cost_eth: f64,
    pub gas_cost_usd: f64,
}

impl ProfitabilityCalculator {
    pub fn new(config: Config) -> Self {
        let mut flashloan_premiums = HashMap::new();
        
        // FlashLoan 수수료 (basis points)
        flashloan_premiums.insert(FlashLoanProvider::Aave, 0.0009);        // 9 bps
        flashloan_premiums.insert(FlashLoanProvider::Balancer, 0.0);       // 0 bps
        flashloan_premiums.insert(FlashLoanProvider::UniswapV3, 0.0);      // 0 bps  
        flashloan_premiums.insert(FlashLoanProvider::DyDx, 0.0002);        // 2 bps
        
        Self {
            config: config.clone(),
            gas_price_gwei: config.execution.gas_price_gwei.unwrap_or(20.0),
            flashloan_premiums,
            min_profit_threshold_usd: config.liquidation.min_profit_threshold_usd.unwrap_or(50.0),
        }
    }
    
    /// 주요 수익성 분석 함수
    pub async fn analyze_liquidation_profitability(
        &self,
        user: &LiquidatableUser,
        available_swap_quotes: &HashMap<(Address, Address), Vec<SwapQuote>>,
        eth_price_usd: f64,
    ) -> Result<LiquidationProfitabilityAnalysis> {
        info!("📊 Starting profitability analysis for user {}", user.address);
        
        let mut strategies = Vec::new();
        
        // 각 부채 자산에 대해 청산 전략 분석
        for debt_position in &user.debt_positions {
            let debt_asset = debt_position.asset;
            let max_liquidatable = user.max_liquidatable_debt.get(&debt_asset)
                .copied()
                .unwrap_or(debt_position.amount);
            
            // 각 담보 자산에 대해 전략 생성
            for collateral_position in &user.collateral_positions {
                let collateral_asset = collateral_position.asset;
                
                // 스왑 경로가 있는지 확인
                if let Some(quotes) = available_swap_quotes.get(&(collateral_asset, debt_asset)) {
                    for quote in quotes {
                        // 각 FlashLoan 제공자에 대해 전략 계산
                        for (&flashloan_provider, _) in &self.flashloan_premiums {
                            if let Ok(strategy) = self.calculate_strategy(
                                debt_asset,
                                collateral_asset,
                                max_liquidatable,
                                debt_position.price_usd,
                                collateral_position.price_usd,
                                flashloan_provider,
                                quote,
                                user,
                                eth_price_usd,
                            ).await {
                                strategies.push(strategy);
                            }
                        }
                    }
                }
            }
        }
        
        // 전략을 수익성 순으로 정렬
        strategies.sort_by(|a, b| b.net_profit_usd.partial_cmp(&a.net_profit_usd).unwrap_or(std::cmp::Ordering::Equal));
        
        let best_strategy = strategies.first().cloned();
        let is_profitable = best_strategy
            .as_ref()
            .map(|s| s.net_profit_usd > self.min_profit_threshold_usd)
            .unwrap_or(false);
        
        let estimated_net_profit_usd = best_strategy
            .as_ref()
            .map(|s| s.net_profit_usd)
            .unwrap_or(0.0);
        
        let profit_margin_percent = best_strategy
            .as_ref()
            .map(|s| s.profit_margin_percent)
            .unwrap_or(0.0);
        
        let recommended_liquidation_amount = best_strategy
            .as_ref()
            .map(|s| s.liquidation_amount)
            .unwrap_or(U256::ZERO);
        
        let risk_score = self.calculate_risk_score(user, &best_strategy);
        
        info!("✅ Profitability analysis complete: {} strategies, best profit: ${:.2}", 
              strategies.len(), estimated_net_profit_usd);
        
        Ok(LiquidationProfitabilityAnalysis {
            user: user.clone(),
            best_strategy,
            all_strategies: strategies,
            is_profitable,
            estimated_net_profit_usd,
            profit_margin_percent,
            recommended_liquidation_amount,
            risk_score,
            analysis_timestamp: chrono::Utc::now(),
        })
    }
    
    /// 개별 청산 전략 계산
    async fn calculate_strategy(
        &self,
        debt_asset: Address,
        collateral_asset: Address,
        max_liquidatable_amount: U256,
        debt_price_usd: f64,
        collateral_price_usd: f64,
        flashloan_provider: FlashLoanProvider,
        swap_quote: &SwapQuote,
        user: &LiquidatableUser,
        eth_price_usd: f64,
    ) -> Result<LiquidationStrategy> {
        // 1. 청산 가능 금액 계산 (Close Factor 적용)
        let liquidation_amount = self.calculate_optimal_liquidation_amount(
            max_liquidatable_amount,
            debt_price_usd,
            user.protocol.clone(),
        )?;
        
        let liquidation_amount_usd = (liquidation_amount.to::<u128>() as f64 / 1e18) * debt_price_usd;
        
        // 2. 예상 담보 획득량 계산 (Liquidation Bonus 적용)
        let liquidation_bonus = user.liquidation_bonus.get(&debt_asset).copied().unwrap_or(0.05); // 5% 기본
        let expected_collateral_usd = liquidation_amount_usd * (1.0 + liquidation_bonus);
        let expected_collateral_amount = U256::from((expected_collateral_usd * 1e18 / collateral_price_usd) as u128);
        
        // 3. 스왑 경로 분석
        let swap_route = SwapRoute {
            aggregator: swap_quote.aggregator.clone(),
            swap_quote: swap_quote.clone(),
            slippage_tolerance: 0.005, // 0.5% slippage
            minimum_output_amount: swap_quote.buy_amount_min,
            price_impact_percent: swap_quote.price_impact * 100.0,
        };
        
        // 4. 비용 분석
        let cost_breakdown = self.calculate_costs(
            liquidation_amount,
            liquidation_amount_usd,
            flashloan_provider,
            &swap_route,
            eth_price_usd,
        ).await?;
        
        // 5. 순이익 계산
        let swap_output_usd = (swap_quote.buy_amount.to::<u128>() as f64 / 1e18) * debt_price_usd;
        let net_profit_usd = swap_output_usd - liquidation_amount_usd - cost_breakdown.total_cost_usd;
        let profit_margin_percent = (net_profit_usd / liquidation_amount_usd) * 100.0;
        
        // 6. 실행 시간 추정
        let execution_time_estimate_ms = swap_quote.estimated_execution_time_ms + 2000; // FlashLoan overhead
        
        Ok(LiquidationStrategy {
            debt_asset,
            collateral_asset,
            liquidation_amount,
            liquidation_amount_usd,
            expected_collateral_amount,
            expected_collateral_usd,
            flashloan_provider,
            swap_route,
            cost_breakdown,
            net_profit_usd,
            profit_margin_percent,
            execution_time_estimate_ms,
        })
    }
    
    /// 최적 청산 금액 계산
    fn calculate_optimal_liquidation_amount(
        &self,
        max_liquidatable: U256,
        debt_price_usd: f64,
        protocol: ProtocolType,
    ) -> Result<U256> {
        let max_liquidatable_usd = (max_liquidatable.to::<u128>() as f64 / 1e18) * debt_price_usd;
        
        // 프로토콜별 최적 청산 전략
        let optimal_amount_usd = match protocol {
            ProtocolType::Aave => {
                // Aave: Close Factor 50%, 전액 청산 가능하면 전액
                if max_liquidatable_usd <= 1000.0 {
                    max_liquidatable_usd // 작은 포지션은 전액
                } else {
                    max_liquidatable_usd * 0.5 // 큰 포지션은 50%
                }
            }
            ProtocolType::CompoundV2 => {
                // Compound: Close Factor 50%
                max_liquidatable_usd * 0.5
            }
            ProtocolType::CompoundV3 => {
                // Compound V3: 다양한 Close Factor
                max_liquidatable_usd * 0.4
            }
            ProtocolType::MakerDAO => {
                // MakerDAO: 전액 청산 가능
                max_liquidatable_usd
            }
        };
        
        Ok(U256::from((optimal_amount_usd * 1e18 / debt_price_usd) as u128))
    }
    
    /// 비용 분석
    async fn calculate_costs(
        &self,
        liquidation_amount: U256,
        liquidation_amount_usd: f64,
        flashloan_provider: FlashLoanProvider,
        swap_route: &SwapRoute,
        eth_price_usd: f64,
    ) -> Result<CostBreakdown> {
        // 1. FlashLoan 수수료
        let flashloan_premium_rate = self.flashloan_premiums.get(&flashloan_provider).copied().unwrap_or(0.0009);
        let flashloan_premium_usd = liquidation_amount_usd * flashloan_premium_rate;
        
        // 2. 가스 비용 추정
        let gas_estimation = self.estimate_gas_costs(eth_price_usd).await?;
        let gas_cost_usd = gas_estimation.gas_cost_usd;
        
        // 3. DEX 수수료 (0x는 수수료 없음, 1inch는 포함됨)
        let dex_fees_usd = match swap_route.aggregator {
            DexType::ZeroX => 0.0,
            DexType::OneInch => liquidation_amount_usd * 0.0003, // 약 3 bps 추정
            DexType::UniswapV2 => liquidation_amount_usd * 0.003, // 30 bps
            DexType::UniswapV3 => liquidation_amount_usd * 0.0005, // 5 bps 추정
            _ => liquidation_amount_usd * 0.001, // 기본값
        };
        
        // 4. 슬리피지 비용
        let slippage_cost_usd = liquidation_amount_usd * swap_route.slippage_tolerance;
        
        // 5. 프로토콜 수수료 (보통 0)
        let protocol_fees_usd = 0.0;
        
        let total_cost_usd = flashloan_premium_usd + gas_cost_usd + dex_fees_usd + slippage_cost_usd + protocol_fees_usd;
        
        Ok(CostBreakdown {
            flashloan_premium_usd,
            gas_cost_usd,
            dex_fees_usd,
            slippage_cost_usd,
            protocol_fees_usd,
            total_cost_usd,
        })
    }
    
    /// 가스 비용 추정
    async fn estimate_gas_costs(&self, eth_price_usd: f64) -> Result<GasCostEstimation> {
        // 가스 사용량 추정 (실제 측정 데이터 기반)
        let flashloan_initiation = 150_000u64;    // FlashLoan 시작
        let liquidation_call = 200_000u64;        // 청산 호출
        let dex_swap = 180_000u64;                // DEX 스왑
        let flashloan_repayment = 100_000u64;     // FlashLoan 상환
        
        let total_gas = flashloan_initiation + liquidation_call + dex_swap + flashloan_repayment;
        
        let gas_cost_eth = (total_gas as f64 * self.gas_price_gwei * 1e-9);
        let gas_cost_usd = gas_cost_eth * eth_price_usd;
        
        debug!("⛽ Gas estimation: {} gas units, {:.6} ETH, ${:.2}", 
               total_gas, gas_cost_eth, gas_cost_usd);
        
        Ok(GasCostEstimation {
            flashloan_initiation,
            liquidation_call,
            dex_swap,
            flashloan_repayment,
            total_gas,
            gas_price_gwei: self.gas_price_gwei,
            gas_cost_eth,
            gas_cost_usd,
        })
    }
    
    /// 위험 점수 계산
    fn calculate_risk_score(&self, user: &LiquidatableUser, strategy: &Option<LiquidationStrategy>) -> f64 {
        let mut risk_score = 0.0;
        
        // 1. Health Factor 위험도
        if user.account_data.health_factor < 1.01 {
            risk_score += 0.1; // 매우 위험
        } else if user.account_data.health_factor < 1.05 {
            risk_score += 0.05; // 위험
        }
        
        // 2. 청산 금액 크기
        if let Some(strategy) = strategy {
            if strategy.liquidation_amount_usd > 100_000.0 {
                risk_score += 0.2; // 큰 금액
            } else if strategy.liquidation_amount_usd > 10_000.0 {
                risk_score += 0.1; // 중간 금액
            }
            
            // 3. 슬리피지 위험
            if strategy.swap_route.price_impact_percent > 2.0 {
                risk_score += 0.15; // 높은 슬리피지
            } else if strategy.swap_route.price_impact_percent > 0.5 {
                risk_score += 0.05; // 중간 슬리피지
            }
        }
        
        // 4. 프로토콜별 위험도
        match user.protocol {
            ProtocolType::Aave => risk_score += 0.0,        // 낮은 위험
            ProtocolType::CompoundV2 => risk_score += 0.05,  // 중간 위험
            ProtocolType::CompoundV3 => risk_score += 0.02,  // 낮은 위험
            ProtocolType::MakerDAO => risk_score += 0.03,    // 중간 위험
        }
        
        risk_score.min(1.0) // 최대 1.0
    }
    
    /// 수익성 필터링
    pub fn filter_profitable_strategies(&self, strategies: Vec<LiquidationStrategy>) -> Vec<LiquidationStrategy> {
        strategies.into_iter()
            .filter(|strategy| {
                strategy.net_profit_usd > self.min_profit_threshold_usd &&
                strategy.profit_margin_percent > 5.0 // 최소 5% 마진
            })
            .collect()
    }
    
    /// FlashLoan 제공자별 가용성 체크
    pub async fn check_flashloan_availability(&self, asset: Address, amount: U256) -> HashMap<FlashLoanProvider, bool> {
        let mut availability = HashMap::new();
        
        // 실제로는 각 프로토콜의 유동성을 체크해야 함
        // 여기서는 기본값으로 설정
        availability.insert(FlashLoanProvider::Aave, true);
        availability.insert(FlashLoanProvider::Balancer, true);
        availability.insert(FlashLoanProvider::UniswapV3, true);
        availability.insert(FlashLoanProvider::DyDx, false); // DYDX는 제한적
        
        debug!("🏦 FlashLoan availability check for {} amount {}", asset, amount);
        availability
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::protocols::*;
    
    #[tokio::test]
    async fn test_profitability_calculation() {
        let config = Config::load_test_config();
        let calculator = ProfitabilityCalculator::new(config);
        
        // 테스트 사용자 생성
        let user = create_test_liquidatable_user();
        let swap_quotes = HashMap::new();
        
        let analysis = calculator.analyze_liquidation_profitability(
            &user,
            &swap_quotes,
            3000.0, // ETH price
        ).await;
        
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        println!("Profitability Analysis: {:#?}", result);
    }
    
    #[tokio::test]
    async fn test_gas_cost_estimation() {
        let config = Config::load_test_config();
        let calculator = ProfitabilityCalculator::new(config);
        
        let gas_costs = calculator.estimate_gas_costs(3000.0).await.unwrap();
        
        assert!(gas_costs.total_gas > 0);
        assert!(gas_costs.gas_cost_usd > 0.0);
        println!("Gas costs: {:#?}", gas_costs);
    }
    
    fn create_test_liquidatable_user() -> LiquidatableUser {
        let user_address: Address = "0x742d35Cc6478354Aba7E4F9B29A6848c417b4c8e".parse().unwrap();
        let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let usdc: Address = "0xA0b86a33E6417f8C681A1fE6954e127c9cd8e46".parse().unwrap();
        
        let account_data = UserAccountData {
            user: user_address,
            protocol: ProtocolType::Aave,
            total_collateral_usd: 10000.0,
            total_debt_usd: 8500.0,
            available_borrows_usd: 1000.0,
            current_liquidation_threshold: 0.85,
            ltv: 0.80,
            health_factor: 0.98, // 청산 위험
            last_updated: chrono::Utc::now(),
        };
        
        let collateral_positions = vec![
            CollateralPosition {
                asset: weth,
                amount: U256::from(3_000_000_000_000_000_000u128), // 3 ETH
                usd_value: 10000.0,
                liquidation_threshold: 0.85,
                price_usd: 3333.33,
            }
        ];
        
        let debt_positions = vec![
            DebtPosition {
                asset: usdc,
                amount: U256::from(8500_000_000u128), // 8500 USDC
                usd_value: 8500.0,
                borrow_rate: 0.05,
                price_usd: 1.0,
            }
        ];
        
        let mut max_liquidatable_debt = HashMap::new();
        max_liquidatable_debt.insert(usdc, U256::from(4250_000_000u128)); // 50% 청산 가능
        
        let mut liquidation_bonus = HashMap::new();
        liquidation_bonus.insert(usdc, 0.05); // 5% 보너스
        
        LiquidatableUser {
            address: user_address,
            protocol: ProtocolType::Aave,
            account_data,
            collateral_positions,
            debt_positions,
            max_liquidatable_debt,
            liquidation_bonus,
            priority_score: 8500.0 * 1500.0, // debt * shortfall
        }
    }
}