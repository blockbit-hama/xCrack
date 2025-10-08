use super::traits::*;
use anyhow::Result;
use ethers::types::{Address, U256};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

/// Uniswap V3 라우터 어댑터
pub struct UniswapV3Adapter {
    config: AdapterConfig,
}

impl UniswapV3Adapter {
    pub fn new(config: AdapterConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl DexAdapter for UniswapV3Adapter {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &str {
        "uniswap_v3"
    }

    fn dex_type(&self) -> DexType {
        DexType::UniswapV3
    }

    async fn quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        Ok(Quote {
            dex_type: DexType::UniswapV3,
            token_in,
            token_out,
            amount_in,
            amount_out: U256::zero(),
            amount_out_min: U256::zero(),
            price_impact_bps: 0,
            price_impact: 0.0,
            gas_estimate: 180000,
            valid_for: 60,
            timestamp: chrono::Utc::now().timestamp() as u64,
            metadata: HashMap::new(),
            route_hash: vec![],
            extra_data: HashMap::new(),
        })
    }

    async fn build_swap_calldata(
        &self,
        quote: &Quote,
        recipient: Address,
        deadline: u64,
    ) -> Result<CalldataBundle, AdapterError> {
        Ok(CalldataBundle {
            target: self.config.router_address,
            calldata: vec![],
            value: U256::zero(),
            data: vec![],
            deadline: deadline,
            gas_estimate: 180000,
            spender: None,
            to: Address::zero(),
        })
    }

    async fn validate_quote(&self, quote: &Quote) -> Result<bool, AdapterError> {
        Ok(true)
    }

    async fn supports_pair(&self, token_in: Address, token_out: Address) -> Result<bool, AdapterError> {
        Ok(true)
    }

    async fn get_min_amount(&self, token: Address) -> Result<U256, AdapterError> {
        Ok(U256::from(1000u64))
    }

    async fn get_fee_info(&self, token_in: Address, token_out: Address) -> Result<FeeInfo, AdapterError> {
        Ok(FeeInfo {
            fee_bps: 30, // 0.3%
            recipient: Some(Address::zero()),
            fee_recipient: Some(Address::zero()),
            platform_fee_bps: 0,
            total_fee_bps: 30,
            trading_fee_bps: 30,
        })
    }
}
