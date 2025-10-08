use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug};
use ethers::types::{Address, U256, Bytes};
use ethers::providers::{Provider, Ws};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dex::{DexAggregator, SwapQuote, DexType};
use crate::protocols::{LiquidatableUser, ProtocolType};
use crate::mev::{Bundle, BundleBuilder, PriorityLevel, LiquidationParams};
use crate::blockchain::BlockchainClient;
use ethers::signers::LocalWallet;
use crate::LiquidationProfitabilityAnalysis;

/// 청산 번들 빌더 - MEV 번들 생성 및 최적화
pub struct LiquidationBundleBuilder {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    dex_aggregators: std::collections::HashMap<DexType, Box<dyn DexAggregator>>,
    bundle_builder: BundleBuilder,
}

/// 청산 시나리오
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
        
        // Create dummy blockchain client and wallet for mock mode
        let blockchain_client = Arc::new(BlockchainClient::new("http://localhost:8545", None).await?);
        let wallet = LocalWallet::new(&mut rand::thread_rng());
        let bundle_builder = BundleBuilder::new(blockchain_client, wallet);
        
        Ok(Self {
            config,
            provider,
            dex_aggregators,
            bundle_builder,
        })
    }
    
    /// 청산 번들 생성
    pub async fn build_liquidation_bundle(
        &mut self,
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
        let health_factor = scenario.user.account_data.health_factor;
        let profit_margin = scenario.profitability_analysis.profit_margin_percent / 100.0;

        // 멤풀에서 동일한 대상에 대한 청산 시도 확인
        let pending_liquidations = self.check_pending_liquidations_count(scenario).await?;

        // 경쟁 수준 결정 로직
        let competition_level = if health_factor < 0.95 && profit_margin > 0.1 {
            // 매우 위험한 포지션 + 높은 수익 → 많은 경쟁자 예상
            if pending_liquidations > 5 {
                CompetitionLevel::Critical
            } else {
                CompetitionLevel::High
            }
        } else if health_factor < 0.98 && profit_margin > 0.05 {
            // 위험한 포지션 + 중간 수익
            if pending_liquidations > 3 {
                CompetitionLevel::High
            } else {
                CompetitionLevel::Medium
            }
        } else if health_factor < 0.99 && profit_margin > 0.02 {
            // 경계선 포지션 + 낮은 수익
            CompetitionLevel::Medium
        } else {
            CompetitionLevel::Low
        };

        debug!("Competition level: {:?} (HF: {:.3}, Profit: {:.2}%, Mempool: {})",
               competition_level, health_factor, profit_margin * 100.0, pending_liquidations);

        Ok(competition_level)
    }

    /// 멤풀에서 동일 대상 청산 시도 확인
    async fn check_pending_liquidations_count(&self, scenario: &LiquidationScenario) -> Result<usize> {
        // 실제로는 멤풀 모니터링을 통해 동일 사용자 청산 트랜잭션 수 확인
        // 현재는 health_factor 기반 추정
        let estimated_count = if scenario.user.account_data.health_factor < 0.95 {
            5 // 매우 위험 → 많은 봇들이 감지
        } else if scenario.user.account_data.health_factor < 0.98 {
            2 // 위험 → 일부 봇들이 감지
        } else {
            0 // 경계선 → 거의 없음
        };

        Ok(estimated_count)
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
    async fn create_mev_bundle(&mut self, scenario: &LiquidationScenario) -> Result<Bundle> {
        // 청산 트랜잭션 생성
        let _liquidation_tx = self.create_liquidation_transaction(scenario).await?;
        
        // 청산 파라미터 생성 (mock implementation)
        let liquidation_params = LiquidationParams {
            protocol_contract: Address::zero(), // dummy address
            liquidation_calldata: Bytes::from(vec![0x30, 0x78]), // "0x" in bytes
            gas_limit: U256::from(200000),
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            expected_profit: U256::from(scenario.profitability_analysis.estimated_net_profit_usd as u64 * 1e18 as u64),
            auto_sell: true,
            sell_contract: None,
            sell_calldata: None,
            use_flash_loan: true,
            flash_loan_amount: Some(scenario.profitability_analysis.recommended_liquidation_amount),
        };
        
        // 번들 빌드
        let bundle = self.bundle_builder
            .create_liquidation_bundle(liquidation_params, 0) // target_block = 0 for mock
            .await?;
        
        Ok(bundle)
    }
    
    /// 청산 트랜잭션 생성
    async fn create_liquidation_transaction(&self, scenario: &LiquidationScenario) -> Result<Bytes> {
        // 프로토콜별 청산 컨트랙트 주소
        let protocol_contract = scenario.user.protocol.clone();

        // 청산 대상 정보
        let target_user = scenario.user.address;
        let debt_to_cover = scenario.liquidation_amount;
        let collateral_asset = scenario.user.address; // 간단화

        // 플래시론 사용 여부 결정
        let use_flash_loan = false; // 간단화
        let flash_loan_amount = if use_flash_loan {
            Some(debt_to_cover)
        } else {
            None
        };

        // 청산 파라미터 구성
        let liquidation_params = LiquidationParams {
            protocol_contract: ethers::types::H160::from_slice(&scenario.user.address.as_bytes()),
            liquidation_calldata: Bytes::new(), // 아래에서 생성
            gas_limit: U256::from(scenario.estimated_gas),
            gas_price: scenario.max_gas_price,
            expected_profit: U256::from((scenario.profitability_analysis.estimated_net_profit_usd * 1e18) as u64),
            auto_sell: true, // 담보를 즉시 판매하여 수익 실현
            sell_contract: None, // 0x/1inch 라우터 주소
            sell_calldata: None, // DEX 스왑 calldata
            use_flash_loan,
            flash_loan_amount,
        };

        // 프로토콜별 청산 calldata 생성
        let calldata = self.encode_protocol_liquidation_call(
            &scenario.user,
            ethers::types::H160::from_slice(&target_user.as_bytes()),
            ethers::types::H160::from_slice(&collateral_asset.as_bytes()),
            debt_to_cover,
        ).await?;

        // 트랜잭션 데이터 인코딩
        let tx_data = self.encode_liquidation_transaction(liquidation_params).await?;

        Ok(tx_data)
    }

    /// 프로토콜별 청산 함수 호출 인코딩
    async fn encode_protocol_liquidation_call(
        &self,
        liquidatable_user: &LiquidatableUser,
        user: Address,
        collateral_asset: Address,
        debt_amount: U256,
    ) -> Result<Bytes> {
        use ethers::abi::{encode, Token};

        match liquidatable_user.protocol {
            ProtocolType::Aave => {
                // Aave V3: liquidationCall(address collateralAsset, address debtAsset, address user, uint256 debtToCover, bool receiveAToken)
                let function_selector = &[0xe8, 0xef, 0xa4, 0x40]; // keccak256("liquidationCall(address,address,address,uint256,bool)")[:4]
                let params = encode(&[
                    Token::Address(collateral_asset.into()),
                    Token::Address(ethers::types::H160::from_slice(&liquidatable_user.account_data.user.as_bytes())), // 부채 자산 주소
                    Token::Address(user.into()),
                    Token::Uint(debt_amount.into()),
                    Token::Bool(false), // aToken 받지 않음 (직접 담보 받기)
                ]);

                let mut calldata = function_selector.to_vec();
                calldata.extend_from_slice(&params);
                Ok(Bytes::from(calldata))
            }
            ProtocolType::CompoundV2 | ProtocolType::CompoundV3 => {
                // Compound V3: absorb(address account)
                let function_selector = &[0xf2, 0xf6, 0x56, 0xc2]; // keccak256("absorb(address)")[:4]
                let params = encode(&[Token::Address(user.into())]);

                let mut calldata = function_selector.to_vec();
                calldata.extend_from_slice(&params);
                Ok(Bytes::from(calldata))
            }
            ProtocolType::MakerDAO => {
                // MakerDAO: bark(bytes32 ilk, address urn)
                // ilk는 담보 유형 식별자 (bytes32)
                let function_selector = &[0x8d, 0x41, 0xf8, 0x8e]; // keccak256("bark(bytes32,address)")[:4]
                let ilk = [0u8; 32]; // 실제로는 담보 타입에 맞게 설정
                let params = encode(&[
                    Token::FixedBytes(ilk.to_vec()),
                    Token::Address(user.into()),
                ]);

                let mut calldata = function_selector.to_vec();
                calldata.extend_from_slice(&params);
                Ok(Bytes::from(calldata))
            }
        }
    }
    
    /// 청산 트랜잭션 인코딩
    async fn encode_liquidation_transaction(&self, params: LiquidationParams) -> Result<Bytes> {
        use ethers::abi::{encode, Token};

        // 플래시론 사용 여부에 따라 다른 인코딩
        if params.use_flash_loan {
            // 플래시론 + 청산 실행 래퍼 함수
            // executeFlashLiquidation(address protocol, bytes calldata liquidationData, uint256 flashLoanAmount)
            let function_selector = &[0x4e, 0x71, 0xd9, 0x2d]; // 임의 selector

            let encoded_params = encode(&[
                Token::Address(params.protocol_contract.into()),
                Token::Bytes(params.liquidation_calldata.to_vec()),
                Token::Uint(params.flash_loan_amount.unwrap_or(U256::zero()).into()),
                Token::Uint(params.gas_price.into()),
                Token::Bool(params.auto_sell),
            ]);

            let mut calldata = function_selector.to_vec();
            calldata.extend_from_slice(&encoded_params);
            Ok(Bytes::from(calldata))
        } else {
            // 직접 청산 실행
            // executeLiquidation(address protocol, bytes calldata liquidationData)
            let function_selector = &[0xa4, 0x19, 0xf3, 0x7c]; // 임의 selector

            let encoded_params = encode(&[
                Token::Address(params.protocol_contract.into()),
                Token::Bytes(params.liquidation_calldata.to_vec()),
                Token::Uint(params.gas_price.into()),
                Token::Bool(params.auto_sell),
            ]);

            let mut calldata = function_selector.to_vec();
            calldata.extend_from_slice(&encoded_params);
            Ok(Bytes::from(calldata))
        }
    }
    
    /// 예상 수익 계산
    async fn calculate_estimated_profit(&self, scenario: &LiquidationScenario) -> Result<U256> {
        let net_profit_wei = U256::from((scenario.profitability_analysis.estimated_net_profit_usd * 1e18) as u64);
        
        // 가스 비용 차감
        let gas_cost = scenario.max_gas_price * U256::from(scenario.estimated_gas);
        let final_profit = if net_profit_wei > gas_cost {
            net_profit_wei - gas_cost
        } else {
            U256::from(0)
        };
        
        Ok(final_profit)
    }
}

/// 청산 파라미터
// LiquidationParams is now imported from crate::mev module

/// ETH 금액 포맷팅 헬퍼
fn format_eth_amount(amount: U256) -> String {
    let eth_amount = amount.low_u128() as f64 / 1e18;
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
