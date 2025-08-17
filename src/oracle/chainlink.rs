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
use tracing::{info, warn, debug};

use super::price_oracle::{PriceOracle, PriceSource, PriceData};

/// Chainlink 가격 오라클
pub struct ChainlinkOracle {
    provider: Arc<Provider<Http>>,
    price_feeds: HashMap<Address, Address>,  // token => price feed address
    eth_usd_feed: Address,
    decimals_cache: HashMap<Address, u8>,
}

impl ChainlinkOracle {
    /// 새로운 Chainlink 오라클 생성
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        let mut price_feeds = HashMap::new();
        
        // 메인넷 Chainlink 가격 피드 주소들
        // ETH/USD
        price_feeds.insert(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),  // WETH
            "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".parse().unwrap(),  // ETH/USD feed
        );
        
        // USDC/USD
        price_feeds.insert(
            "0xA0b86991c33E6417f8C681A1fFE6954e127c9cd8e46".parse().unwrap(),  // USDC
            "0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6".parse().unwrap(),  // USDC/USD feed
        );
        
        // USDT/USD
        price_feeds.insert(
            "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(),  // USDT
            "0x3E7d1eAB13ad0104d2750B8863b489D65364e32D".parse().unwrap(),  // USDT/USD feed
        );
        
        // DAI/USD
        price_feeds.insert(
            "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(),  // DAI
            "0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9".parse().unwrap(),  // DAI/USD feed
        );
        
        // WBTC/USD
        price_feeds.insert(
            "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap(),  // WBTC
            "0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c".parse().unwrap(),  // BTC/USD feed
        );
        
        Self {
            provider,
            price_feeds,
            eth_usd_feed: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".parse().unwrap(),
            decimals_cache: HashMap::new(),
        }
    }
    
    /// 가격 피드 추가
    pub fn add_price_feed(&mut self, token: Address, feed: Address) {
        self.price_feeds.insert(token, feed);
    }
    
    /// Chainlink aggregator에서 최신 가격 가져오기
    async fn get_latest_price(&self, feed_address: Address) -> Result<(Decimal, u64)> {
        let abi_json = r#"[
            {
                "inputs": [],
                "name": "latestRoundData",
                "outputs": [
                    {"internalType": "uint80", "name": "roundId", "type": "uint80"},
                    {"internalType": "int256", "name": "answer", "type": "int256"},
                    {"internalType": "uint256", "name": "startedAt", "type": "uint256"},
                    {"internalType": "uint256", "name": "updatedAt", "type": "uint256"},
                    {"internalType": "uint80", "name": "answeredInRound", "type": "uint80"}
                ],
                "stateMutability": "view",
                "type": "function"
            },
            {
                "inputs": [],
                "name": "decimals",
                "outputs": [{"internalType": "uint8", "name": "", "type": "uint8"}],
                "stateMutability": "view",
                "type": "function"
            }
        ]"#;
        
        let abi: Abi = serde_json::from_str(abi_json)?;
        let contract = Contract::new(feed_address, abi, self.provider.clone());
        
        // decimals 가져오기
        let decimals: u8 = contract.method("decimals", ())?.call().await?;
        
        // 최신 라운드 데이터 가져오기
        let (_, answer, _, updated_at, _): (u128, i256, U256, U256, u128) = 
            contract.method("latestRoundData", ())?.call().await?;
        
        // i256을 Decimal로 변환
        let price = if answer >= i256::zero() {
            let answer_u256 = U256::from(answer.unsigned_abs());
            let divisor = U256::from(10u64).pow(U256::from(decimals));
            let price_raw = answer_u256.as_u128() as f64 / divisor.as_u128() as f64;
            Decimal::try_from(price_raw)?
        } else {
            return Err(anyhow::anyhow!("Negative price from Chainlink"));
        };
        
        Ok((price, updated_at.as_u64()))
    }
    
    /// ETH 가격 가져오기 (USD)
    async fn get_eth_price(&self) -> Result<Decimal> {
        let (price, _) = self.get_latest_price(self.eth_usd_feed).await?;
        Ok(price)
    }
}

/// i256 타입 정의 (ethers에 없는 경우)
type i256 = ethers::types::I256;

#[async_trait]
impl PriceOracle for ChainlinkOracle {
    async fn get_price_usd(&self, token: Address) -> Result<PriceData> {
        // 가격 피드 주소 찾기
        let feed_address = self.price_feeds.get(&token)
            .ok_or_else(|| anyhow::anyhow!("No Chainlink price feed for token: {:?}", token))?;
        
        // 가격 가져오기
        let (price_usd, timestamp) = self.get_latest_price(*feed_address).await?;
        
        // ETH 가격 가져오기
        let eth_price = self.get_eth_price().await?;
        let price_eth = price_usd / eth_price;
        
        let mut price_data = PriceData::new(
            token,
            price_usd,
            price_eth,
            PriceSource::Chainlink,
        );
        
        price_data.timestamp = timestamp;
        price_data.confidence = 0.95;  // Chainlink은 높은 신뢰도
        
        debug!("Chainlink price for {:?}: ${}", token, price_usd);
        
        Ok(price_data)
    }
    
    async fn get_price_eth(&self, token: Address) -> Result<PriceData> {
        let price_data_usd = self.get_price_usd(token).await?;
        Ok(price_data_usd)
    }
    
    async fn get_price_ratio(&self, token_a: Address, token_b: Address) -> Result<Decimal> {
        let price_a = self.get_price_usd(token_a).await?;
        let price_b = self.get_price_usd(token_b).await?;
        
        Ok(price_a.price_usd / price_b.price_usd)
    }
    
    async fn get_prices_batch(&self, tokens: &[Address]) -> Result<Vec<PriceData>> {
        let mut prices = Vec::new();
        
        for token in tokens {
            match self.get_price_usd(*token).await {
                Ok(price) => prices.push(price),
                Err(e) => {
                    warn!("Failed to get Chainlink price for {:?}: {}", token, e);
                    // 실패한 토큰은 건너뛰기
                }
            }
        }
        
        Ok(prices)
    }
    
    async fn get_twap(&self, token: Address, _period_seconds: u64) -> Result<PriceData> {
        // Chainlink은 TWAP을 직접 제공하지 않으므로 현재 가격 반환
        // 실제로는 여러 라운드의 데이터를 가져와 계산해야 함
        self.get_price_usd(token).await
    }
    
    fn source_type(&self) -> PriceSource {
        PriceSource::Chainlink
    }
    
    fn reliability_score(&self) -> f64 {
        0.95  // Chainlink은 매우 신뢰할 수 있는 오라클
    }
    
    fn update_frequency(&self) -> u64 {
        3600  // Chainlink은 일반적으로 1시간마다 업데이트 (실제로는 더 자주)
    }
}