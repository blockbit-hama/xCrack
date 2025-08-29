use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn, error};
use alloy::primitives::{Address, U256, Bytes};
use ethers::providers::{Provider, Ws};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dex::{DexAggregator, SwapQuote, SwapParams, DexType};
use crate::protocols::{LiquidatableUser, ProtocolType};
use crate::mev::{Bundle, BundleBuilder, BundleType, PriorityLevel};
use crate::utils::profitability::LiquidationProfitabilityAnalysis;

/// 청산 번들 빌더 - MEV 번들 생성 및 최적화
pub struct LiquidationBundleBuilder {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    dex_aggregators: std::collections::HashMap<DexType, Box<dyn DexAggregator>>,
    bundle_builder: BundleBuilder,
}

/// 청산 시나리오
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationScenario {
    pub user: LiquidatableUser,
    pub liquidation_amount: U256,
    pub profitability_analysis: LiquidationProfitabilityAnalysis,
    pub swap_quote: SwapQuote,
    pub execution_priority: PriorityLevel,
    pub estimated_gas: u64,
    pub max_gas_price: U256,
}

/// 청산 번들
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationBundle {
    pub scenario: LiquidationScenario,
    pub bundle: Bundle,
    pub estimated_profit: U256,
    pub success_probability: f64,
    pub competition_level: CompetitionLevel,
}

/// 경쟁 수준
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompetitionLevel {
    Low,      // 낮은 경쟁
    Medium,   // 중간 경쟁
    High,     // 높은 경쟁
    Critical, // 치열한 경쟁
}

impl LiquidationBundleBuilder {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        dex_aggregators: std::collections::HashMap<DexType, Box<dyn DexAggregator>>,
    ) -> Result<Self> {
        info!("🔧 Initializing Liquidation Bundle Builder...");
        
        let bundle_builder = BundleBuilder::new(config.clone()).await?;
        
        Ok(Self {
            config,
            provider,
            dex_aggregators,
            bundle_builder,
        })
    }
    
    /// 청산 번들 생성
    pub async fn build_liquidation_bundle(
        &self,
        scenario: LiquidationScenario,
    ) -> Result<LiquidationBundle> {
        info!("🏗️ Building liquidation bundle for user: {:?}", scenario.user.address);
        
        // 1. 경쟁 수준 분석
        let competition_level = self.analyze_competition_level(&scenario).await?;
        
        // 2. 성공 확률 계산
        let success_probability = self.calculate_success_probability(&scenario, &competition_level).await?;
        
        // 3. MEV 번들 생성
        let bundle = self.create_mev_bundle(&scenario).await?;
        
        // 4. 예상 수익 계산
        let estimated_profit = self.calculate_estimated_profit(&scenario).await?;
        
        let liquidation_bundle = LiquidationBundle {
            scenario,
            bundle,
            estimated_profit,
            success_probability,
            competition_level,
        };
        
        info!("✅ Liquidation bundle created with estimated profit: {} ETH", 
              format_eth_amount(estimated_profit));
        
        Ok(liquidation_bundle)
    }
    
    /// 경쟁 수준 분석
    async fn analyze_competition_level(&self, scenario: &LiquidationScenario) -> Result<CompetitionLevel> {
        // TODO: 실제 경쟁자 분석 로직 구현
        // 현재는 간단한 휴리스틱 사용
        
        let health_factor = scenario.user.account_data.health_factor;
        let profit_margin = scenario.profitability_analysis.profit_margin;
        
        let competition_level = if health_factor < 0.95 && profit_margin > 0.1 {
            CompetitionLevel::Critical
        } else if health_factor < 0.98 && profit_margin > 0.05 {
            CompetitionLevel::High
        } else if health_factor < 0.99 && profit_margin > 0.02 {
            CompetitionLevel::Medium
        } else {
            CompetitionLevel::Low
        };
        
        debug!("Competition level: {:?} (HF: {:.3}, Profit: {:.2}%)", 
               competition_level, health_factor, profit_margin * 100.0);
        
        Ok(competition_level)
    }
    
    /// 성공 확률 계산
    async fn calculate_success_probability(
        &self,
        scenario: &LiquidationScenario,
        competition_level: &CompetitionLevel,
    ) -> Result<f64> {
        let base_probability = match competition_level {
            CompetitionLevel::Low => 0.9,
            CompetitionLevel::Medium => 0.7,
            CompetitionLevel::High => 0.5,
            CompetitionLevel::Critical => 0.3,
        };
        
        // 가스 가격 경쟁 요소
        let gas_competition_factor = if scenario.max_gas_price > U256::from(100_000_000_000u64) {
            0.8 // 높은 가스 가격
        } else {
            1.0
        };
        
        // 슬리피지 요소
        let slippage_factor = if scenario.swap_quote.price_impact > 0.05 {
            0.7 // 높은 가격 임팩트
        } else {
            1.0
        };
        
        let success_probability = base_probability * gas_competition_factor * slippage_factor;
        
        debug!("Success probability: {:.2}% (base: {:.2}%, gas: {:.2}%, slippage: {:.2}%)",
               success_probability * 100.0, base_probability * 100.0, 
               gas_competition_factor * 100.0, slippage_factor * 100.0);
        
        Ok(success_probability)
    }
    
    /// MEV 번들 생성
    async fn create_mev_bundle(&self, scenario: &LiquidationScenario) -> Result<Bundle> {
        // 청산 트랜잭션 생성
        let liquidation_tx = self.create_liquidation_transaction(scenario).await?;
        
        // 번들 빌드
        let bundle = self.bundle_builder
            .create_bundle(
                vec![liquidation_tx],
                BundleType::Liquidation,
                scenario.execution_priority.clone(),
            )
            .await?;
        
        Ok(bundle)
    }
    
    /// 청산 트랜잭션 생성
    async fn create_liquidation_transaction(&self, scenario: &LiquidationScenario) -> Result<Bytes> {
        // TODO: 실제 청산 컨트랙트 호출 트랜잭션 생성
        // 현재는 플레이스홀더
        
        let liquidation_params = LiquidationParams {
            protocol: scenario.user.protocol.clone(),
            user: scenario.user.address,
            liquidation_amount: scenario.liquidation_amount,
            swap_quote: scenario.swap_quote.clone(),
        };
        
        // 트랜잭션 데이터 인코딩
        let tx_data = self.encode_liquidation_transaction(liquidation_params).await?;
        
        Ok(tx_data)
    }
    
    /// 청산 트랜잭션 인코딩
    async fn encode_liquidation_transaction(&self, params: LiquidationParams) -> Result<Bytes> {
        // TODO: 실제 ABI 인코딩 구현
        // 현재는 더미 데이터 반환
        
        let dummy_data = format!(
            "0xexecuteLiquidation({},{},{})",
            params.user,
            params.liquidation_amount,
            params.swap_quote.buy_amount
        );
        
        Ok(Bytes::from(dummy_data.as_bytes()))
    }
    
    /// 예상 수익 계산
    async fn calculate_estimated_profit(&self, scenario: &LiquidationScenario) -> Result<U256> {
        let net_profit = scenario.profitability_analysis.net_profit;
        
        // 가스 비용 차감
        let gas_cost = scenario.max_gas_price * U256::from(scenario.estimated_gas);
        let final_profit = if net_profit > gas_cost {
            net_profit - gas_cost
        } else {
            U256::from(0)
        };
        
        Ok(final_profit)
    }
}

/// 청산 파라미터
#[derive(Debug, Clone)]
struct LiquidationParams {
    protocol: ProtocolType,
    user: Address,
    liquidation_amount: U256,
    swap_quote: SwapQuote,
}

/// ETH 금액 포맷팅 헬퍼
fn format_eth_amount(amount: U256) -> String {
    let eth_amount = amount.as_u128() as f64 / 1e18;
    format!("{:.6}", eth_amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_bundle_builder_creation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_competition_level_analysis() {
        // TODO: 테스트 구현
        assert!(true);
    }
}
