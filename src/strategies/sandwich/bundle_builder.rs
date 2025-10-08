use super::types::{SandwichOpportunity, SandwichBundle};
use anyhow::{Result, anyhow};
use ethers::prelude::*;
use ethers::types::{Address, U256, Bytes, H256};
use ethers::abi::{encode, Token};
use std::sync::Arc;
use tracing::{info, debug};

/// 샌드위치 번들 빌더 - MEV 번들 생성
pub struct SandwichBundleBuilder {
    contract_address: Address,
    chain_id: u64,
}

impl SandwichBundleBuilder {
    pub fn new(contract_address: Address, chain_id: u64) -> Self {
        info!("🏗️ 샌드위치 번들 빌더 초기화 (contract: {:?})", contract_address);
        Self {
            contract_address,
            chain_id,
        }
    }

    /// 샌드위치 번들 생성
    pub async fn build_bundle(
        &self,
        opportunity: &SandwichOpportunity,
        block_number: u64,
    ) -> Result<SandwichBundle> {
        info!("🏗️ 샌드위치 번들 생성 시작");
        debug!("   Front-run: {} ETH", format_eth(opportunity.front_run_amount));
        debug!("   Target: {:?}", opportunity.target_tx_hash);

        // 1. Front-run 트랜잭션 생성
        let front_run_tx = self.build_front_run_transaction(opportunity)?;

        // 2. Back-run 트랜잭션 생성
        let back_run_tx = self.build_back_run_transaction(opportunity)?;

        // 3. 번들 생성
        let bundle = SandwichBundle {
            opportunity: opportunity.clone(),
            front_run_tx,
            target_tx_hash: opportunity.target_tx_hash,
            back_run_tx,
            bundle_hash: None, // Flashbots 제출 후 설정
            estimated_profit: opportunity.estimated_profit,
            total_gas_cost: opportunity.gas_cost,
            net_profit: opportunity.net_profit,
            success_probability: opportunity.success_probability,
            submitted_at: block_number,
        };

        info!("✅ 샌드위치 번들 생성 완료");
        Ok(bundle)
    }

    /// Front-run 트랜잭션 생성
    fn build_front_run_transaction(&self, opp: &SandwichOpportunity) -> Result<Bytes> {
        // SandwichAttackStrategy.sol의 executeFrontRun 호출
        // executeFrontRun(address tokenIn, address tokenOut, address router, uint256 amount, bytes calldata swapData)

        let function_selector = &[0xa1, 0xb2, 0xc3, 0xd4]; // executeFrontRun selector (mock)

        // 스왑 calldata 생성
        let swap_calldata = self.encode_swap_calldata(
            opp.token_in,
            opp.token_out,
            opp.front_run_amount,
            opp.dex_type,
        )?;

        let params = encode(&[
            Token::Address(opp.token_in.into()),
            Token::Address(opp.token_out.into()),
            Token::Address(opp.dex_router.into()),
            Token::Uint(opp.front_run_amount.into()),
            Token::Bytes(swap_calldata.to_vec()),
        ]);

        let mut calldata = function_selector.to_vec();
        calldata.extend_from_slice(&params);

        Ok(Bytes::from(calldata))
    }

    /// Back-run 트랜잭션 생성
    fn build_back_run_transaction(&self, opp: &SandwichOpportunity) -> Result<Bytes> {
        // SandwichAttackStrategy.sol의 executeBackRun 호출
        // executeBackRun(address tokenIn, address tokenOut, address router, uint256 minProfitWei, bytes calldata swapData)

        let function_selector = &[0xe5, 0xf6, 0xa7, 0xb8]; // executeBackRun selector (mock)

        // 역방향 스왑 calldata 생성 (token_out → token_in)
        let swap_calldata = self.encode_swap_calldata(
            opp.token_out,
            opp.token_in,
            opp.back_run_amount,
            opp.dex_type,
        )?;

        let params = encode(&[
            Token::Address(opp.token_out.into()),
            Token::Address(opp.token_in.into()),
            Token::Address(opp.dex_router.into()),
            Token::Uint(opp.net_profit.into()), // minProfitWei
            Token::Bytes(swap_calldata.to_vec()),
        ]);

        let mut calldata = function_selector.to_vec();
        calldata.extend_from_slice(&params);

        Ok(Bytes::from(calldata))
    }

    /// DEX 스왑 calldata 인코딩
    fn encode_swap_calldata(
        &self,
        token_in: Address,
        token_out: Address,
        amount: U256,
        dex_type: super::types::DexType,
    ) -> Result<Bytes> {
        use super::types::DexType;

        match dex_type {
            DexType::UniswapV2 | DexType::SushiSwap => {
                self.encode_uniswap_v2_swap(token_in, token_out, amount)
            }
            DexType::UniswapV3 => {
                self.encode_uniswap_v3_swap(token_in, token_out, amount)
            }
            _ => Err(anyhow!("Unsupported DEX type")),
        }
    }

    fn encode_uniswap_v2_swap(
        &self,
        token_in: Address,
        token_out: Address,
        amount: U256,
    ) -> Result<Bytes> {
        // swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] path, address to, uint deadline)
        let function_selector = &[0x38, 0xed, 0x17, 0x39];

        let path = vec![
            Token::Address(token_in.into()),
            Token::Address(token_out.into()),
        ];

        // 환경변수에서 데드라인 로드
        let deadline_secs = std::env::var("SANDWICH_DEADLINE_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .unwrap_or(300);
        
        let deadline = U256::from(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() + deadline_secs);

        let params = encode(&[
            Token::Uint(amount.into()),
            Token::Uint(U256::zero().into()), // amountOutMin = 0 (샌드위치이므로)
            Token::Array(path),
            Token::Address(self.contract_address.into()),
            Token::Uint(deadline.into()),
        ]);

        let mut calldata = function_selector.to_vec();
        calldata.extend_from_slice(&params);

        Ok(Bytes::from(calldata))
    }

    fn encode_uniswap_v3_swap(
        &self,
        token_in: Address,
        token_out: Address,
        amount: U256,
    ) -> Result<Bytes> {
        // exactInputSingle((address tokenIn, address tokenOut, uint24 fee, address recipient, uint256 deadline, uint256 amountIn, uint256 amountOutMinimum, uint160 sqrtPriceLimitX96))
        let function_selector = &[0xc0, 0x4b, 0x8d, 0x59];

        // 환경변수에서 데드라인 로드
        let deadline_secs = std::env::var("SANDWICH_DEADLINE_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .unwrap_or(300);
        
        let deadline = U256::from(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() + deadline_secs);

        let params = encode(&[
            Token::Address(token_in.into()),
            Token::Address(token_out.into()),
            // 환경변수에서 Uniswap V3 수수료 티어 로드
            Token::Uint(U256::from(
                std::env::var("SANDWICH_UNISWAP_V3_FEE_TIER")
                    .unwrap_or_else(|_| "3000".to_string())
                    .parse::<u32>()
                    .unwrap_or(3000)
            ).into()), // 0.3% fee tier
            Token::Address(self.contract_address.into()),
            Token::Uint(deadline.into()),
            Token::Uint(amount.into()),
            Token::Uint(U256::zero().into()), // amountOutMinimum
            Token::Uint(U256::zero().into()), // sqrtPriceLimitX96 = 0
        ]);

        let mut calldata = function_selector.to_vec();
        calldata.extend_from_slice(&params);

        Ok(Bytes::from(calldata))
    }
}

fn format_eth(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_bundle_builder() {
        let contract = Address::from_str("0x1234567890123456789012345678901234567890").unwrap();
        let builder = SandwichBundleBuilder::new(contract, 1);

        assert_eq!(builder.chain_id, 1);
    }
}
