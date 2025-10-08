use super::types::{
    SandwichOpportunity, KellyCriterionParams, KellyCriterionResult,
    CompetitionLevel, TargetTransaction, DexType,
};
use super::target_analyzer::TargetAnalysis;
use anyhow::{Result, anyhow};
use ethers::types::{Address, U256};
use std::sync::Arc;
use tracing::{info, debug};

/// 샌드위치 수익성 분석기
pub struct ProfitabilityAnalyzer {
    min_profit_wei: U256,
    min_profit_percentage: f64,
    max_price_impact: f64,
    risk_factor: f64, // Kelly Criterion 조정 계수 (0.5 = Half Kelly)
}

impl ProfitabilityAnalyzer {
    pub fn new(
        min_profit_eth: f64,
        min_profit_percentage: f64,
        max_price_impact: f64,
        risk_factor: f64,
    ) -> Self {
        info!("💰 수익성 분석기 초기화");
        info!("   최소 수익: {} ETH ({:.2}%)", min_profit_eth, min_profit_percentage * 100.0);
        info!("   최대 가격 영향: {:.2}%", max_price_impact * 100.0);
        info!("   위험 계수 (Kelly): {}", risk_factor);

        Self {
            min_profit_wei: U256::from((min_profit_eth * 1e18) as u64),
            min_profit_percentage,
            max_price_impact,
            risk_factor,
        }
    }

    /// 샌드위치 기회 분석
    pub async fn analyze_opportunity(
        &self,
        target: &TargetAnalysis,
        current_gas_price: U256,
    ) -> Result<Option<SandwichOpportunity>> {
        debug!("🔍 수익성 분석 시작");

        // 1. 가격 영향 필터링
        if target.estimated_price_impact > self.max_price_impact {
            debug!("   ❌ 가격 영향 초과: {:.2}% > {:.2}%",
                   target.estimated_price_impact * 100.0,
                   self.max_price_impact * 100.0);
            return Ok(None);
        }

        // 2. Kelly Criterion으로 최적 포지션 크기 계산
        let kelly_params = KellyCriterionParams {
            success_probability: target.competition_level.success_probability(),
            price_impact_bps: (target.estimated_price_impact * 10000.0) as u32,
            available_capital: target.amount_in * 2, // 타겟 금액의 200%까지 사용 가능
            risk_factor: self.risk_factor,
        };

        let kelly_result = self.calculate_kelly_criterion(&kelly_params)?;

        // 3. Front-run 금액 결정
        let front_run_amount = kelly_result.optimal_size;
        if front_run_amount.is_zero() {
            debug!("   ❌ Kelly Criterion: 포지션 크기 0");
            return Ok(None);
        }

        // 4. 예상 수익 계산
        let estimated_profit = self.estimate_profit(
            front_run_amount,
            target.amount_in,
            target.estimated_price_impact,
            target.dex_type,
        )?;

        // 5. 가스 비용 계산
        let gas_cost = self.estimate_gas_cost(
            current_gas_price,
            target.competition_level,
        );

        // 6. 순이익 계산
        if estimated_profit <= gas_cost {
            debug!("   ❌ 순이익 음수: profit={} ETH, gas={} ETH",
                   format_eth(estimated_profit), format_eth(gas_cost));
            return Ok(None);
        }

        let net_profit = estimated_profit - gas_cost;

        // 7. 최소 수익 필터링
        if net_profit < self.min_profit_wei {
            debug!("   ❌ 최소 수익 미달: {} ETH < {} ETH",
                   format_eth(net_profit), format_eth(self.min_profit_wei));
            return Ok(None);
        }

        // 8. 수익률 계산
        let profit_percentage = net_profit.as_u128() as f64 / front_run_amount.as_u128() as f64;
        if profit_percentage < self.min_profit_percentage {
            debug!("   ❌ 최소 수익률 미달: {:.2}% < {:.2}%",
                   profit_percentage * 100.0, self.min_profit_percentage * 100.0);
            return Ok(None);
        }

        // 9. 샌드위치 기회 생성
        let opportunity = SandwichOpportunity {
            target_tx_hash: target.tx.hash,
            target_tx: target.tx.clone(),
            dex_router: target.router_address,
            dex_type: target.dex_type,
            token_in: target.token_in,
            token_out: target.token_out,
            amount_in: target.amount_in,
            expected_amount_out: target.amount_out_min,
            front_run_amount,
            back_run_amount: front_run_amount, // 동일하게 되팔기
            estimated_profit,
            gas_cost,
            net_profit,
            profit_percentage,
            success_probability: kelly_result.expected_value,
            price_impact: target.estimated_price_impact,
            slippage_tolerance: 0.01, // 1%
            optimal_size_kelly: kelly_result.optimal_size,
            competition_level: target.competition_level,
            detected_at: target.tx.block_number.unwrap_or(0),
        };

        info!("✅ 샌드위치 기회 발견!");
        info!("   Front-run: {} ETH", format_eth(front_run_amount));
        info!("   예상 수익: {} ETH", format_eth(estimated_profit));
        info!("   가스 비용: {} ETH", format_eth(gas_cost));
        info!("   순이익: {} ETH ({:.2}%)", format_eth(net_profit), profit_percentage * 100.0);
        info!("   성공 확률: {:.1}%", kelly_result.expected_value * 100.0);

        Ok(Some(opportunity))
    }

    /// Kelly Criterion 계산
    pub fn calculate_kelly_criterion(&self, params: &KellyCriterionParams) -> Result<KellyCriterionResult> {
        let p = params.success_probability;
        let q = 1.0 - p;
        let b = params.price_impact_bps as f64 / 10000.0; // basis points → decimal

        if b <= 0.0 {
            return Err(anyhow!("Price impact must be positive"));
        }

        // Kelly Formula: f* = (p * b - q) / b
        let kelly_fraction = if p * b > q {
            (p * b - q) / b
        } else {
            0.0 // 기대값이 음수이면 투자하지 않음
        };

        // 위험 조정 (Half Kelly 등)
        let adjusted_kelly = kelly_fraction * params.risk_factor;

        // 포지션 크기 제한 (1% ~ 25%)
        let clamped_kelly = adjusted_kelly.max(0.01).min(0.25);

        let optimal_size = (params.available_capital.as_u128() as f64 * clamped_kelly) as u128;
        let optimal_size_u256 = U256::from(optimal_size);

        // 기대값 계산
        let expected_value = p * b - q * b;

        // 파산 확률 추정 (간단한 모델)
        let risk_of_ruin = if expected_value > 0.0 {
            (q / p).powf(optimal_size as f64 / params.available_capital.as_u128() as f64)
        } else {
            1.0
        };

        debug!("📊 Kelly Criterion 결과:");
        debug!("   Kelly Fraction: {:.2}%", kelly_fraction * 100.0);
        debug!("   조정된 Kelly: {:.2}%", adjusted_kelly * 100.0);
        debug!("   최종 Kelly: {:.2}%", clamped_kelly * 100.0);
        debug!("   최적 크기: {} ETH", format_eth(optimal_size_u256));
        debug!("   기대값: {:.4}", expected_value);
        debug!("   파산 확률: {:.6}", risk_of_ruin);

        Ok(KellyCriterionResult {
            optimal_size: optimal_size_u256,
            optimal_size_percentage: clamped_kelly,
            kelly_percentage: kelly_fraction,
            adjusted_kelly_percentage: adjusted_kelly,
            expected_value,
            risk_of_ruin,
        })
    }

    /// 예상 수익 계산
    fn estimate_profit(
        &self,
        front_run_amount: U256,
        target_amount: U256,
        price_impact: f64,
        dex_type: DexType,
    ) -> Result<U256> {
        // 간단한 수익 모델:
        // 1. Front-run으로 가격 상승
        // 2. 타겟 트랜잭션으로 가격 추가 상승
        // 3. Back-run으로 높은 가격에 판매

        let front_run_eth = front_run_amount.as_u128() as f64 / 1e18;
        let target_eth = target_amount.as_u128() as f64 / 1e18;

        // 타겟의 가격 영향을 이용한 수익
        let profit_from_target = front_run_eth * price_impact;

        // 환경변수에서 DEX 수수료 로드
        let fee = match dex_type {
            DexType::UniswapV2 | DexType::SushiSwap => {
                std::env::var("SANDWICH_UNISWAP_V2_FEE")
                    .unwrap_or_else(|_| "0.003".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.003)
            },
            DexType::UniswapV3 => {
                std::env::var("SANDWICH_UNISWAP_V3_FEE")
                    .unwrap_or_else(|_| "0.003".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.003)
            },
            _ => {
                std::env::var("SANDWICH_DEFAULT_FEE")
                    .unwrap_or_else(|_| "0.003".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.003)
            },
        };

        let total_fees = front_run_eth * fee * 2.0; // 2번의 스왑
        let net_profit_eth = profit_from_target - total_fees;

        if net_profit_eth <= 0.0 {
            return Ok(U256::zero());
        }

        let profit_wei = (net_profit_eth * 1e18) as u128;
        Ok(U256::from(profit_wei))
    }

    /// 가스 비용 추정 (실제 가스 가격 사용)
    fn estimate_gas_cost(&self, base_gas_price: U256, competition: CompetitionLevel) -> U256 {
        // 환경변수에서 가스 한도 로드
        let gas_per_tx = std::env::var("SANDWICH_GAS_PER_TX")
            .unwrap_or_else(|_| "200000".to_string())
            .parse::<u64>()
            .unwrap_or(200_000);
        
        // Front-run + Back-run 두 트랜잭션
        let total_gas = U256::from(gas_per_tx) * 2;

        // 경쟁에 따른 가스 가격 조정
        let multiplier = competition.recommended_gas_multiplier();
        let adjusted_gas_price = (base_gas_price.as_u128() as f64 * multiplier) as u128;

        // 환경변수에서 우선순위 수수료 로드
        let priority_fee_gwei = match competition {
            CompetitionLevel::Low => std::env::var("SANDWICH_PRIORITY_FEE_LOW_GWEI")
                .unwrap_or_else(|_| "1".to_string())
                .parse::<u64>()
                .unwrap_or(1),
            CompetitionLevel::Medium => std::env::var("SANDWICH_PRIORITY_FEE_MEDIUM_GWEI")
                .unwrap_or_else(|_| "2".to_string())
                .parse::<u64>()
                .unwrap_or(2),
            CompetitionLevel::High => std::env::var("SANDWICH_PRIORITY_FEE_HIGH_GWEI")
                .unwrap_or_else(|_| "5".to_string())
                .parse::<u64>()
                .unwrap_or(5),
            CompetitionLevel::Critical => std::env::var("SANDWICH_PRIORITY_FEE_CRITICAL_GWEI")
                .unwrap_or_else(|_| "10".to_string())
                .parse::<u64>()
                .unwrap_or(10),
        };

        let priority_fee = U256::from(priority_fee_gwei) * U256::from(1_000_000_000u64);
        let total_gas_price = U256::from(adjusted_gas_price) + priority_fee;
        total_gas_price * total_gas
    }
}

fn format_eth(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelly_criterion() {
        let analyzer = ProfitabilityAnalyzer::new(0.01, 0.02, 0.05, 0.5);

        let params = KellyCriterionParams {
            success_probability: 0.7, // 70% 성공 확률
            price_impact_bps: 200,    // 2% 가격 영향
            available_capital: U256::from(10u128 * 10u128.pow(18)), // 10 ETH
            risk_factor: 0.5,         // Half Kelly
        };

        let result = analyzer.calculate_kelly_criterion(&params).unwrap();

        assert!(result.optimal_size > U256::zero());
        assert!(result.expected_value > 0.0);
        assert!(result.risk_of_ruin < 1.0);
    }

    #[test]
    fn test_profit_estimation() {
        let analyzer = ProfitabilityAnalyzer::new(0.01, 0.02, 0.05, 0.5);

        let front_run = U256::from(5u128 * 10u128.pow(18)); // 5 ETH
        let target = U256::from(50u128 * 10u128.pow(18));  // 50 ETH
        let price_impact = 0.03; // 3%

        let profit = analyzer.estimate_profit(
            front_run,
            target,
            price_impact,
            DexType::UniswapV2,
        ).unwrap();

        assert!(profit > U256::zero());
    }
}
