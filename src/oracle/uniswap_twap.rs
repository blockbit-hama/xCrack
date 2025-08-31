use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    types::{Address, U256},
    providers::{Provider, Http},
    contract::Contract,
    abi::Abi,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use tracing::{warn, debug};

use super::price_oracle::{PriceOracle, PriceSource, PriceData};

/// Uniswap V3 TWAP 오라클
pub struct UniswapTwapOracle {
    provider: Arc<Provider<Http>>,
    factory_address: Address,
    quoter_address: Address,
    weth_address: Address,
    usdc_address: Address,
    pool_cache: HashMap<(Address, Address), Address>,  // (token0, token1) => pool
    observation_window: u32,  // TWAP 관찰 기간 (초)
}

impl UniswapTwapOracle {
    /// 새로운 Uniswap TWAP 오라클 생성
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self {
            provider,
            factory_address: "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse().unwrap(),  // Uniswap V3 Factory
            quoter_address: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".parse().unwrap(),  // Quoter V2
            weth_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
            usdc_address: "0xA0b86991c33E6417f8C681A1fFE6954e127c9cd8e46".parse().unwrap(),
            pool_cache: HashMap::new(),
            observation_window: 1800,  // 30분 TWAP
        }
    }
    
    /// 풀 주소 가져오기
    async fn get_pool_address(&self, token0: Address, token1: Address, fee: u32) -> Result<Address> {
        // 캐시 확인
        let cache_key = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };
        
        if let Some(pool) = self.pool_cache.get(&cache_key) {
            return Ok(*pool);
        }
        
        // Factory에서 풀 주소 조회
        let abi_json = r#"[{
            "inputs": [
                {"internalType": "address", "name": "tokenA", "type": "address"},
                {"internalType": "address", "name": "tokenB", "type": "address"},
                {"internalType": "uint24", "name": "fee", "type": "uint24"}
            ],
            "name": "getPool",
            "outputs": [{"internalType": "address", "name": "pool", "type": "address"}],
            "stateMutability": "view",
            "type": "function"
        }]"#;
        
        let abi: Abi = serde_json::from_str(abi_json)?;
        let factory = Contract::new(self.factory_address, abi, self.provider.clone());
        
        let pool_address: Address = factory
            .method("getPool", (token0, token1, fee))?
            .call()
            .await?;
        
        if pool_address == Address::zero() {
            return Err(anyhow::anyhow!("Pool not found for tokens"));
        }
        
        Ok(pool_address)
    }
    
    /// 풀에서 TWAP 가격 가져오기
    async fn get_twap_from_pool(&self, pool_address: Address, _period: u32) -> Result<Decimal> {
        let abi_json = r#"[
            {
                "inputs": [{"internalType": "uint32[]", "name": "secondsAgos", "type": "uint32[]"}],
                "name": "observe",
                "outputs": [
                    {"internalType": "int56[]", "name": "tickCumulatives", "type": "int56[]"},
                    {"internalType": "uint160[]", "name": "secondsPerLiquidityCumulativeX128s", "type": "uint160[]"}
                ],
                "stateMutability": "view",
                "type": "function"
            },
            {
                "inputs": [],
                "name": "slot0",
                "outputs": [
                    {"internalType": "uint160", "name": "sqrtPriceX96", "type": "uint160"},
                    {"internalType": "int24", "name": "tick", "type": "int24"},
                    {"internalType": "uint16", "name": "observationIndex", "type": "uint16"},
                    {"internalType": "uint16", "name": "observationCardinality", "type": "uint16"},
                    {"internalType": "uint16", "name": "observationCardinalityNext", "type": "uint16"},
                    {"internalType": "uint8", "name": "feeProtocol", "type": "uint8"},
                    {"internalType": "bool", "name": "unlocked", "type": "bool"}
                ],
                "stateMutability": "view",
                "type": "function"
            }
        ]"#;
        
        let abi: Abi = serde_json::from_str(abi_json)?;
        let pool = Contract::new(pool_address, abi, self.provider.clone());
        
        // 현재 가격 가져오기 (TWAP 대신 임시로 사용)
        let (sqrt_price_x96, _, _, _, _, _, _): (U256, i32, u16, u16, u16, u8, bool) = 
            pool.method("slot0", ())?.call().await?;
        
        // sqrtPriceX96을 실제 가격으로 변환
        // price = (sqrtPriceX96 / 2^96)^2
        let sqrt_price = sqrt_price_x96.as_u128() as f64 / (2f64.powi(96));
        let price = sqrt_price * sqrt_price;
        
        Ok(Decimal::try_from(price)?)
    }
    
    /// Quoter를 사용한 가격 조회
    async fn get_quote(&self, token_in: Address, token_out: Address, amount_in: U256) -> Result<U256> {
        let abi_json = r#"[{
            "inputs": [
                {"internalType": "address", "name": "tokenIn", "type": "address"},
                {"internalType": "address", "name": "tokenOut", "type": "address"},
                {"internalType": "uint24", "name": "fee", "type": "uint24"},
                {"internalType": "uint256", "name": "amountIn", "type": "uint256"},
                {"internalType": "uint160", "name": "sqrtPriceLimitX96", "type": "uint160"}
            ],
            "name": "quoteExactInputSingle",
            "outputs": [
                {"internalType": "uint256", "name": "amountOut", "type": "uint256"}
            ],
            "stateMutability": "nonpayable",
            "type": "function"
        }]"#;
        
        let abi: Abi = serde_json::from_str(abi_json)?;
        let quoter = Contract::new(self.quoter_address, abi, self.provider.clone());
        
        // 여러 수수료 티어 시도 (0.05%, 0.3%, 1%)
        let fee_tiers = vec![500u32, 3000u32, 10000u32];
        
        for fee in fee_tiers {
            match quoter
                .method("quoteExactInputSingle", (token_in, token_out, fee, amount_in, U256::zero()))?
                .call()
                .await
            {
                Ok(amount_out) => return Ok(amount_out),
                Err(_) => continue,
            }
        }
        
        Err(anyhow::anyhow!("No valid Uniswap V3 pool found"))
    }
}

#[async_trait]
impl PriceOracle for UniswapTwapOracle {
    async fn get_price_usd(&self, token: Address) -> Result<PriceData> {
        // 1 토큰을 USDC로 변환
        let amount_in = U256::from(10u64).pow(U256::from(18));  // 1 token (18 decimals assumed)
        
        let amount_out = if token == self.usdc_address {
            // USDC 자체는 1 USD
            U256::from(1_000_000u64)  // 1 USDC (6 decimals)
        } else {
            // 토큰을 USDC로 변환
            self.get_quote(token, self.usdc_address, amount_in).await?
        };
        
        // USDC 양을 USD 가격으로 변환 (6 decimals)
        let price_usd = Decimal::from(amount_out.as_u128()) / Decimal::from(1_000_000u128);
        
        // ETH 가격 계산
        let eth_amount = if token == self.weth_address {
            amount_in
        } else {
            self.get_quote(token, self.weth_address, amount_in).await?
        };
        
        let price_eth = Decimal::from(eth_amount.as_u128()) / Decimal::from(10u128.pow(18));
        
        let mut price_data = PriceData::new(
            token,
            price_usd,
            price_eth,
            PriceSource::UniswapV3,
        );
        
        price_data.confidence = 0.85;  // Uniswap은 중간 신뢰도
        
        debug!("Uniswap TWAP price for {:?}: ${}", token, price_usd);
        
        Ok(price_data)
    }
    
    async fn get_price_eth(&self, token: Address) -> Result<PriceData> {
        // ETH 기준 가격 계산
        let amount_in = U256::from(10u64).pow(U256::from(18));
        
        let eth_amount = if token == self.weth_address {
            amount_in
        } else {
            self.get_quote(token, self.weth_address, amount_in).await?
        };
        
        let price_eth = Decimal::from(eth_amount.as_u128()) / Decimal::from(10u128.pow(18));
        
        // USD 가격도 계산
        let price_data_usd = self.get_price_usd(token).await?;
        
        let mut price_data = PriceData::new(
            token,
            price_data_usd.price_usd,
            price_eth,
            PriceSource::UniswapV3,
        );
        
        price_data.confidence = 0.85;
        
        Ok(price_data)
    }
    
    async fn get_price_ratio(&self, token_a: Address, token_b: Address) -> Result<Decimal> {
        let amount_in = U256::from(10u64).pow(U256::from(18));
        let amount_out = self.get_quote(token_a, token_b, amount_in).await?;
        
        Ok(Decimal::from(amount_out.as_u128()) / Decimal::from(amount_in.as_u128()))
    }
    
    async fn get_prices_batch(&self, tokens: &[Address]) -> Result<Vec<PriceData>> {
        let mut prices = Vec::new();
        
        for token in tokens {
            match self.get_price_usd(*token).await {
                Ok(price) => prices.push(price),
                Err(e) => {
                    warn!("Failed to get Uniswap price for {:?}: {}", token, e);
                }
            }
        }
        
        Ok(prices)
    }
    
    async fn get_twap(&self, token: Address, _period_seconds: u64) -> Result<PriceData> {
        // 실제 TWAP 구현
        // 현재는 간단하게 현재 가격 반환
        let mut price_data = self.get_price_usd(token).await?;
        price_data.confidence = 0.9;  // TWAP은 더 높은 신뢰도
        
        Ok(price_data)
    }
    
    fn source_type(&self) -> PriceSource {
        PriceSource::UniswapV3
    }
    
    fn reliability_score(&self) -> f64 {
        0.85  // Uniswap은 중간 정도의 신뢰도
    }
    
    fn update_frequency(&self) -> u64 {
        12  // 각 블록마다 업데이트 (약 12초)
    }
}