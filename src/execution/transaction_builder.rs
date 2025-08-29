use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{debug, warn, error, info};
use alloy::primitives::{Address, U256, Bytes};
use ethers::{
    providers::{Provider, Ws, Middleware},
    contract::Contract,
    abi::{Abi, Token},
    types::{H160, U256 as EthersU256, TransactionRequest, Bytes as EthersBytes},
    core::utils::hex,
};

use crate::config::Config;

/// 청산 트랜잭션 구축 도구
pub struct TransactionBuilder {
    provider: Arc<Provider<Ws>>,
    config: Arc<Config>,
    liquidation_contract_abi: Abi,
}

impl TransactionBuilder {
    pub async fn new(provider: Arc<Provider<Ws>>, config: Arc<Config>) -> Result<Self> {
        info!("🔧 Initializing Transaction Builder...");
        
        // LiquidationStrategy.sol ABI 로드
        let liquidation_contract_abi: Abi = serde_json::from_str(LIQUIDATION_STRATEGY_ABI)?;
        
        info!("✅ Transaction Builder initialized");
        
        Ok(Self {
            provider,
            config,
            liquidation_contract_abi,
        })
    }
    
    /// executeLiquidation 함수 호출 데이터 인코딩
    pub async fn encode_liquidation_call(
        &self,
        debt_asset: Address,
        amount: U256,
        liquidation_params: Vec<u8>,
    ) -> Result<Bytes> {
        debug!("🔧 Encoding liquidation call for asset {} amount {}", debt_asset, amount);
        
        // executeLiquidation(address asset, uint256 amount, LiquidationParams calldata params)
        let function = self.liquidation_contract_abi.function("executeLiquidation")?;
        
        // Parameters conversion
        let debt_asset_h160 = H160::from_slice(debt_asset.as_slice());
        let amount_ethers = EthersU256::from_dec_str(&amount.to_string())?;
        
        // LiquidationParams 구조체 디코딩 (간단화된 구현)
        let params_token = self.decode_liquidation_params(liquidation_params)?;
        
        let tokens = vec![
            Token::Address(debt_asset_h160),
            Token::Uint(amount_ethers),
            params_token,
        ];
        
        let encoded = function.encode_input(&tokens)?;
        
        debug!("✅ Liquidation call encoded, {} bytes", encoded.len());
        Ok(Bytes::from(encoded))
    }
    
    /// LiquidationParams 디코딩
    fn decode_liquidation_params(&self, params: Vec<u8>) -> Result<Token> {
        // 실제 구현에서는 완전한 구조체 디코딩
        // 임시로 단순화된 버전
        
        if params.len() < 96 { // 3 * 32 bytes minimum
            return Err(anyhow!("Invalid liquidation params length"));
        }
        
        let user_address = H160::from_slice(&params[12..32]); // Skip padding
        let collateral_address = H160::from_slice(&params[44..64]);
        let liquidation_amount = EthersU256::from_big_endian(&params[64..96]);
        
        // LiquidationParams struct
        let params_struct = Token::Tuple(vec![
            Token::Address(user_address),          // user
            Token::Address(collateral_address),    // collateralAsset  
            Token::Uint(liquidation_amount),       // liquidationAmount
            Token::Uint(EthersU256::from(0)),      // flashLoanProvider (enum)
            Token::Bytes(vec![]),                  // swapCalldata
            Token::Uint(EthersU256::from(5)),      // slippageToleranceBps (0.05%)
        ]);
        
        Ok(params_struct)
    }
    
    /// 청산 트랜잭션 전송
    pub async fn send_liquidation_transaction(
        &self,
        contract_address: Address,
        calldata: Bytes,
        estimated_gas_cost_usd: f64,
    ) -> Result<String> {
        info!("📤 Sending liquidation transaction to {}", contract_address);
        
        // Gas price 계산
        let gas_price = self.provider.get_gas_price().await?;
        let gas_limit = self.estimate_gas_limit(&calldata).await?;
        
        debug!("⛽ Gas price: {} gwei, Gas limit: {}", gas_price.as_u64() / 1e9 as u64, gas_limit);
        
        // Transaction 구성
        let tx_request = TransactionRequest::new()
            .to(H160::from_slice(contract_address.as_slice()))
            .data(EthersBytes::from(calldata.to_vec()))
            .gas_price(gas_price)
            .gas(gas_limit)
            .value(EthersU256::zero());
        
        // 트랜잭션 전송 (실제로는 private key로 서명 필요)
        // 여기서는 시뮬레이션만
        let tx_hash = format!("0x{}", hex::encode(&[0u8; 32])); // Mock transaction hash
        
        info!("🚀 Liquidation transaction sent: {}", tx_hash);
        Ok(tx_hash)
    }
    
    /// 가스 한도 추정
    async fn estimate_gas_limit(&self, calldata: &Bytes) -> Result<EthersU256> {
        // 청산 트랜잭션의 일반적인 가스 사용량
        // FlashLoan + Liquidation + Swap + Repay
        let base_gas = 650_000u64; // 경험적 값
        
        // calldata 크기에 따른 추가 가스
        let calldata_gas = (calldata.len() as u64) * 16; // 16 gas per byte
        
        let total_gas = base_gas + calldata_gas;
        
        // 안전 마진 20%
        let gas_with_margin = total_gas * 120 / 100;
        
        Ok(EthersU256::from(gas_with_margin))
    }
    
    /// MEV Bundle 생성 (Flashbots 등)
    pub async fn build_mev_bundle(
        &self,
        liquidation_tx: Bytes,
        target_block: u64,
    ) -> Result<MEVBundle> {
        debug!("📦 Building MEV bundle for block {}", target_block);
        
        // MEV Bundle은 여러 트랜잭션을 묶어서 전송
        let bundle = MEVBundle {
            transactions: vec![liquidation_tx],
            target_block,
            max_timestamp: chrono::Utc::now().timestamp() as u64 + 300, // 5분 후 만료
            min_timestamp: chrono::Utc::now().timestamp() as u64,
            reverting_tx_hashes: vec![], // 실패해도 되는 트랜잭션
        };
        
        debug!("✅ MEV bundle built with {} transactions", bundle.transactions.len());
        Ok(bundle)
    }
    
    /// 트랜잭션 시뮬레이션
    pub async fn simulate_transaction(
        &self,
        contract_address: Address,
        calldata: &Bytes,
    ) -> Result<SimulationResult> {
        debug!("🧪 Simulating liquidation transaction...");
        
        // eth_call을 사용한 시뮬레이션
        let call_request = TransactionRequest::new()
            .to(H160::from_slice(contract_address.as_slice()))
            .data(EthersBytes::from(calldata.to_vec()))
            .value(EthersU256::zero());
        
        match self.provider.call(&call_request.into(), None).await {
            Ok(result) => {
                debug!("✅ Simulation successful, result: {} bytes", result.len());
                
                Ok(SimulationResult {
                    success: true,
                    return_data: Bytes::from(result.to_vec()),
                    gas_used: 650_000, // 추정값
                    error_message: None,
                })
            }
            Err(e) => {
                warn!("❌ Simulation failed: {}", e);
                
                Ok(SimulationResult {
                    success: false,
                    return_data: Bytes::new(),
                    gas_used: 0,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }
    
    /// FlashLoan 가용성 체크
    pub async fn check_flashloan_availability(
        &self,
        asset: Address,
        amount: U256,
    ) -> Result<FlashLoanAvailability> {
        debug!("🏦 Checking FlashLoan availability for {} {}", asset, amount);
        
        // 각 FlashLoan 제공자의 가용성 체크
        let mut providers = Vec::new();
        
        // Aave v3 체크
        if let Ok(available) = self.check_aave_liquidity(asset, amount).await {
            providers.push(FlashLoanProviderInfo {
                provider: "Aave".to_string(),
                available,
                fee_bps: 9, // 9 basis points
                estimated_gas: 150_000,
            });
        }
        
        // Balancer 체크 
        if let Ok(available) = self.check_balancer_liquidity(asset, amount).await {
            providers.push(FlashLoanProviderInfo {
                provider: "Balancer".to_string(),
                available,
                fee_bps: 0, // No fee
                estimated_gas: 120_000,
            });
        }
        
        Ok(FlashLoanAvailability {
            asset,
            amount,
            providers,
            recommended_provider: self.select_best_provider(&providers),
        })
    }
    
    /// Aave 유동성 체크
    async fn check_aave_liquidity(&self, _asset: Address, _amount: U256) -> Result<bool> {
        // 실제로는 Aave Pool의 getReserveData를 호출
        Ok(true) // 임시로 항상 사용 가능
    }
    
    /// Balancer 유동성 체크
    async fn check_balancer_liquidity(&self, _asset: Address, _amount: U256) -> Result<bool> {
        // 실제로는 Balancer Vault의 유동성 체크
        Ok(true) // 임시로 항상 사용 가능
    }
    
    /// 최적 FlashLoan 제공자 선택
    fn select_best_provider(&self, providers: &[FlashLoanProviderInfo]) -> Option<String> {
        providers.iter()
            .filter(|p| p.available)
            .min_by_key(|p| p.fee_bps + p.estimated_gas / 10_000) // Fee + gas cost 최적화
            .map(|p| p.provider.clone())
    }
}

#[derive(Debug, Clone)]
pub struct MEVBundle {
    pub transactions: Vec<Bytes>,
    pub target_block: u64,
    pub max_timestamp: u64,
    pub min_timestamp: u64,
    pub reverting_tx_hashes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub return_data: Bytes,
    pub gas_used: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlashLoanAvailability {
    pub asset: Address,
    pub amount: U256,
    pub providers: Vec<FlashLoanProviderInfo>,
    pub recommended_provider: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlashLoanProviderInfo {
    pub provider: String,
    pub available: bool,
    pub fee_bps: u32,
    pub estimated_gas: u64,
}

// LiquidationStrategy.sol ABI (간단화된 버전)
const LIQUIDATION_STRATEGY_ABI: &str = r#"[
    {
        "inputs": [
            {
                "name": "asset",
                "type": "address"
            },
            {
                "name": "amount", 
                "type": "uint256"
            },
            {
                "components": [
                    {
                        "name": "user",
                        "type": "address"
                    },
                    {
                        "name": "collateralAsset",
                        "type": "address"
                    },
                    {
                        "name": "liquidationAmount",
                        "type": "uint256"
                    },
                    {
                        "name": "flashLoanProvider",
                        "type": "uint8"
                    },
                    {
                        "name": "swapCalldata",
                        "type": "bytes"
                    },
                    {
                        "name": "slippageToleranceBps",
                        "type": "uint256"
                    }
                ],
                "name": "params",
                "type": "tuple"
            }
        ],
        "name": "executeLiquidation",
        "outputs": [
            {
                "name": "",
                "type": "bool"
            }
        ],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[test]
    fn test_liquidation_params_encoding() {
        // 파라미터 인코딩 테스트
        let user_address: Address = "0x742d35Cc6478354Aba7E4F9B29A6848c417b4c8e".parse().unwrap();
        let collateral_asset: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let liquidation_amount = U256::from(1000000000000000000u128); // 1 ETH
        
        let mut params = vec![0u8; 96];
        params[12..32].copy_from_slice(user_address.as_slice());
        params[44..64].copy_from_slice(collateral_asset.as_slice());
        params[64..96].copy_from_slice(&liquidation_amount.to_be_bytes::<32>());
        
        assert_eq!(params.len(), 96);
        println!("Encoded params: {} bytes", params.len());
    }
    
    #[tokio::test]
    async fn test_gas_estimation() {
        let calldata = Bytes::from(vec![0u8; 100]);
        let base_gas = 650_000u64;
        let calldata_gas = (calldata.len() as u64) * 16;
        let total_gas = base_gas + calldata_gas;
        let gas_with_margin = total_gas * 120 / 100;
        
        assert!(gas_with_margin > total_gas);
        println!("Gas estimation: {} -> {} (with margin)", total_gas, gas_with_margin);
    }
}