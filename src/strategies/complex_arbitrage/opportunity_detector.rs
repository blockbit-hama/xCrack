//! Multi-Asset Arbitrage 기회 탐지기
//!
//! 삼각 아비트리지, 포지션 마이그레이션, 복합 아비트리지 기회를 탐지합니다.

use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, debug};
use ethers::types::{Address, U256};
use ethers::providers::{Provider, Ws};
use rust_decimal::Decimal;
use chrono::Utc;

use crate::config::Config;
use crate::adapters::AdapterSelector;
use super::types::*;

pub struct OpportunityDetector {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    adapter_selector: Arc<AdapterSelector>,
    min_profit_percentage: f64,
    min_profit_usd: Decimal,
}

impl OpportunityDetector {
    pub fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        adapter_selector: Arc<AdapterSelector>,
    ) -> Self {
        let min_profit_usd = config.strategies.micro_arbitrage.min_profit_usd
            .parse::<f64>()
            .map(Decimal::from_f64_retain)
            .unwrap_or_else(|_| Some(Decimal::from(10)))
            .unwrap_or(Decimal::from(10));

        Self {
            config: config.clone(),
            provider,
            adapter_selector,
            min_profit_percentage: config.strategies.micro_arbitrage.min_profit_percentage,
            min_profit_usd,
        }
    }

    /// 삼각 아비트래지 기회 탐지
    pub async fn scan_triangular_opportunities(&self) -> Result<Vec<MultiAssetArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // 주요 토큰 페어들
        let token_pairs = vec![
            ("WETH", "USDC"),
            ("WETH", "DAI"),
            ("USDC", "DAI"),
            ("WETH", "WBTC"),
            ("USDC", "WBTC"),
        ];

        for (token_a, token_b) in token_pairs {
            if let Some(opportunity) = self.find_triangular_arbitrage_opportunity(token_a, token_b).await? {
                opportunities.push(opportunity);
            }
        }

        Ok(opportunities)
    }

    /// 특정 토큰 페어에 대한 삼각 아비트래지 기회 찾기
    async fn find_triangular_arbitrage_opportunity(
        &self,
        token_a: &str,
        token_b: &str,
    ) -> Result<Option<MultiAssetArbitrageOpportunity>> {
        // 중간 토큰 후보들
        let intermediate_tokens = vec!["USDC", "DAI", "WETH"];

        for intermediate in intermediate_tokens {
            if intermediate == token_a || intermediate == token_b {
                continue;
            }

            if let Some(opportunity) = self.calculate_triangular_profitability(
                token_a, token_b, intermediate
            ).await? {
                return Ok(Some(opportunity));
            }
        }

        Ok(None)
    }

    /// 삼각 아비트리지 수익성 계산
    async fn calculate_triangular_profitability(
        &self,
        token_a: &str,
        token_b: &str,
        token_c: &str,
    ) -> Result<Option<MultiAssetArbitrageOpportunity>> {
        // 토큰 주소 가져오기
        let addr_a = self.get_token_address(token_a)?;
        let addr_b = self.get_token_address(token_b)?;
        let addr_c = self.get_token_address(token_c)?;

        // 초기 금액 (예: 10 ETH)
        let initial_amount = U256::from(10u64) * U256::exp10(18);

        // 경로: A → C → B → A
        // Step 1: A → C
        let amount_c = self.get_swap_quote(addr_a, addr_c, initial_amount).await?;

        // Step 2: C → B
        let amount_b = self.get_swap_quote(addr_c, addr_b, amount_c).await?;

        // Step 3: B → A
        let final_amount = self.get_swap_quote(addr_b, addr_a, amount_b).await?;

        // 수익 계산
        if final_amount <= initial_amount {
            return Ok(None);
        }

        let profit = final_amount - initial_amount;
        let profit_percentage = ((final_amount.as_u128() as f64 / initial_amount.as_u128() as f64) - 1.0) * 100.0;

        // 최소 수익률 확인
        if profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }

        // 가스 비용 추정
        let gas_estimate = 500_000u64; // 삼각 아비트리지는 약 500K 가스

        // FlashLoan 금액 설정
        let flashloan_amounts = vec![
            FlashLoanAmount {
                asset: addr_a,
                amount: initial_amount,
                premium: initial_amount * U256::from(9) / U256::from(10000), // 0.09%
            }
        ];

        // 신뢰도 점수 계산
        let confidence_score = self.calculate_confidence_score(profit_percentage, gas_estimate).await?;

        let opportunity = MultiAssetArbitrageOpportunity {
            id: format!("tri_arb_{}_{}", Utc::now().timestamp_millis(), fastrand::u32(..)),
            strategy_type: MultiAssetStrategyType::TriangularArbitrage {
                token_a: addr_a,
                token_b: addr_b,
                token_c: addr_c,
                amount_a: initial_amount,
                amount_b,
            },
            expected_profit: profit,
            expected_profit_usd: Decimal::from_f64_retain(profit_percentage * 10.0).unwrap_or_default(),
            profit_percentage,
            total_gas_estimate: gas_estimate,
            execution_deadline: Utc::now() + chrono::Duration::seconds(30),
            confidence_score,
            flashloan_amounts,
            created_at: Utc::now(),
        };

        debug!("🔺 삼각 아비트리지 기회 발견: {} → {} → {} (수익률: {:.2}%)",
            token_a, token_c, token_b, profit_percentage);

        Ok(Some(opportunity))
    }

    /// 토큰 주소 가져오기
    fn get_token_address(&self, symbol: &str) -> Result<Address> {
        self.config.get_token_address(symbol)
            .ok_or_else(|| anyhow!("Token not found: {}", symbol))
            .and_then(|addr| Ok(Address::from_slice(addr.as_bytes())))
    }

    /// DEX에서 스왑 견적 가져오기
    async fn get_swap_quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<U256> {
        // 실제 구현에서는 어댑터를 통해 실제 DEX 견적을 가져옴
        // 지금은 간단한 시뮬레이션
        let slippage = 0.997; // 0.3% 슬리피지 가정
        let amount_out = (amount_in.as_u128() as f64 * slippage) as u128;

        Ok(U256::from(amount_out))
    }

    /// 신뢰도 점수 계산
    async fn calculate_confidence_score(&self, profit_percentage: f64, gas_estimate: u64) -> Result<f64> {
        let mut score = 0.5; // 기본 점수

        // 수익률에 따라 점수 증가
        if profit_percentage > 2.0 {
            score += 0.3;
        } else if profit_percentage > 1.0 {
            score += 0.2;
        } else if profit_percentage > 0.5 {
            score += 0.1;
        }

        // 가스 비용이 낮으면 점수 증가
        if gas_estimate < 400_000 {
            score += 0.1;
        }

        // 시장 변동성 확인
        let volatility = self.calculate_market_volatility().await?;
        if volatility < 0.05 {
            score += 0.1;
        }

        Ok(score.min(1.0))
    }

    /// 시장 변동성 계산
    async fn calculate_market_volatility(&self) -> Result<f64> {
        // 실제 구현에서는 최근 가격 변동성을 계산
        // 지금은 임의값 반환
        Ok(0.03) // 3% 변동성
    }

    /// 복합 아비트리지 기회 탐지 (여러 DEX를 거친 경로)
    pub async fn scan_complex_opportunities(&self) -> Result<Vec<MultiAssetArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // 복잡한 경로 찾기 (예: Uniswap → SushiSwap → Curve)
        if let Some(opportunity) = self.find_best_route_parallel().await? {
            opportunities.push(opportunity);
        }

        Ok(opportunities)
    }

    /// 병렬로 최적 경로 찾기
    async fn find_best_route_parallel(&self) -> Result<Option<MultiAssetArbitrageOpportunity>> {
        // 실제 구현에서는 여러 DEX를 병렬로 쿼리하여 최적 경로 찾기
        // 지금은 None 반환
        Ok(None)
    }
}
