use ethers::types::Bytes;
/// 가격 조회 모듈
///
/// 역할: 자산 가격 조회 및 DEX 견적 가져오기
/// - Chainlink Oracle을 통한 실시간 가격
/// - 0x API를 통한 스왑 견적
/// - 1inch API를 통한 스왑 견적
/// - 가격 캐시 관리
/// - 멀티소스 가격 집계

use std::sync::Arc;
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use ethers::types::{Address, U256, I256};
use ethers::providers::{Provider, Ws, Middleware};
use ethers::contract::Contract;
use ethers::abi::Abi;
use std::collections::HashMap;
use tracing::{info, warn, error, debug};
use reqwest::Client;
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::strategies::liquidation::types::{AssetPrice, ZeroExQuote, PriceSource};

#[derive(Clone)]
pub struct PriceOracle {
    asset_prices: Arc<Mutex<HashMap<Address, AssetPrice>>>,
    http_client: Client,
    provider: Option<Arc<Provider<Ws>>>, // Ethereum provider for on-chain calls
    chainlink_feeds: HashMap<Address, Address>, // asset -> chainlink feed address
    update_interval: Duration,
    last_update: Arc<Mutex<Instant>>,
}

impl PriceOracle {
    pub fn new(asset_prices: Arc<Mutex<HashMap<Address, AssetPrice>>>) -> Self {
        let mut chainlink_feeds = HashMap::new();

        // 주요 자산들의 Chainlink 피드 주소 설정 (Ethereum Mainnet)
        chainlink_feeds.insert(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(), // WETH
            "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".parse().unwrap(), // ETH/USD
        );
        chainlink_feeds.insert(
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(), // USDC
            "0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6".parse().unwrap(), // USDC/USD
        );
        chainlink_feeds.insert(
            "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(), // USDT
            "0x3E7d1eAB13ad0104d2750B3323F15B332821eBe7".parse().unwrap(), // USDT/USD
        );
        chainlink_feeds.insert(
            "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(), // DAI
            "0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9".parse().unwrap(), // DAI/USD
        );

        Self {
            asset_prices,
            http_client: Client::new(),
            provider: None, // Will be set with with_provider()
            chainlink_feeds,
            update_interval: Duration::from_secs(30), // 30초마다 업데이트
            last_update: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Set the Ethereum provider for on-chain calls
    pub fn with_provider(mut self, provider: Arc<Provider<Ws>>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 자산 가격 초기화
    pub async fn initialize(&self) -> Result<()> {
        info!("💱 자산 가격 초기화 중...");
        
        let mut prices = self.asset_prices.lock().await;
        
        // 주요 자산들의 가격 설정 (실제로는 오라클에서 가져와야 함)
        let assets = vec![
            ("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, 2800.0), // WETH
            ("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?, 1.0),    // USDC
            ("0xdAC17F958D2ee523a2206206994597C13D831ec7".parse()?, 1.0),    // USDT
            ("0x6B175474E89094C44Da98b954EedeAC495271d0F".parse()?, 1.0),    // DAI
        ];
        
        for (asset, price_usd) in assets {
            prices.insert(asset, crate::strategies::liquidation::types::AssetPrice {
                asset,
                price_usd,
                price_eth: price_usd / 2800.0,
                last_updated: std::time::Instant::now(),
                source: crate::strategies::liquidation::types::PriceSource::Manual,
            });
        }
        
        info!("✅ {} 개 자산 가격 초기화 완료", prices.len());
        
        // Chainlink Oracle에서 실시간 가격 조회
        self.update_prices_from_chainlink().await?;
        
        // 백업으로 DEX API에서도 가격 조회
        self.update_prices_from_dex().await?;
        
        Ok(())
    }

    /// Chainlink Oracle에서 가격 업데이트
    pub async fn update_prices_from_chainlink(&self) -> Result<()> {
        info!("🔗 Chainlink Oracle에서 가격 업데이트 중...");
        
        let mut prices = self.asset_prices.lock().await;
        
        for (asset, feed_address) in &self.chainlink_feeds {
            match self.get_chainlink_price(*feed_address).await {
                Ok(price_usd) => {
                    let eth_price = prices.get(&"0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap())
                        .map(|p| p.price_usd)
                        .unwrap_or(2800.0);
                    
                    prices.insert(*asset, AssetPrice {
                        asset: *asset,
                        price_usd,
                        price_eth: price_usd / eth_price,
                        last_updated: Instant::now(),
                        source: PriceSource::Chainlink,
                    });
                    
                    info!("📊 {} 가격 업데이트: ${:.2}", 
                          self.get_asset_symbol(*asset), price_usd);
                }
                Err(e) => {
                    warn!("⚠️ Chainlink 가격 조회 실패 {}: {}", 
                          self.get_asset_symbol(*asset), e);
                }
            }
        }
        
        Ok(())
    }

    /// DEX API에서 가격 업데이트
    pub async fn update_prices_from_dex(&self) -> Result<()> {
        info!("🔄 DEX API에서 가격 업데이트 중...");
        
        let mut prices = self.asset_prices.lock().await;
        
        // WETH 가격을 기준으로 다른 자산들의 가격 조회
        let weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let eth_amount = U256::from(1000000000000000000u64); // 1 ETH
        
        for asset in self.chainlink_feeds.keys() {
            if *asset == weth_address {
                continue; // WETH는 이미 Chainlink에서 조회됨
            }
            
            match self.get_uniswap_price(weth_address, *asset, eth_amount).await {
                Ok(price_usd) => {
                    let eth_price = prices.get(&weth_address)
                        .map(|p| p.price_usd)
                        .unwrap_or(2800.0);
                    
                    prices.insert(*asset, AssetPrice {
                        asset: *asset,
                        price_usd,
                        price_eth: price_usd / eth_price,
                        last_updated: Instant::now(),
                        source: PriceSource::Uniswap,
                    });
                    
                    info!("📊 {} DEX 가격 업데이트: ${:.2}", 
                          self.get_asset_symbol(*asset), price_usd);
                }
                Err(e) => {
                    warn!("⚠️ DEX 가격 조회 실패 {}: {}", 
                          self.get_asset_symbol(*asset), e);
                }
            }
        }
        
        Ok(())
    }

    /// 0x API를 통한 견적 조회
    pub async fn get_0x_quote(
        &self,
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
    ) -> Result<Option<ZeroExQuote>> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.0x.org/swap/v1/quote?sellToken={}&buyToken={}&sellAmount={}",
            format!("{:#x}", sell_token),
            format!("{:#x}", buy_token),
            sell_amount.to_string()
        );
        
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        
        #[derive(serde::Deserialize, Default)]
        struct ZeroExQuoteWire {
            to: String,
            data: String,
            value: Option<String>,
            #[serde(rename = "allowanceTarget")]
            allowance_target: Option<String>,
        }
        
        let q: ZeroExQuoteWire = resp.json().await.unwrap_or_default();
        if q.to.is_empty() || q.data.is_empty() {
            return Ok(None);
        }
        
        let to: Address = q.to.parse()?;
        let data_bytes = hex::decode(q.data.trim_start_matches("0x")).unwrap_or_default();
        let value = if let Some(v) = q.value {
            Some(U256::from_str_radix(&v, 10).unwrap_or(U256::zero()))
        } else {
            None
        };
        let allowance_target = q.allowance_target.and_then(|s| s.parse::<Address>().ok());
        
        Ok(Some(ZeroExQuote { 
            to, 
            data: Bytes::from(data_bytes), 
            value, 
            allowance_target 
        }))
    }

    /// 1inch API를 통한 견적 조회
    pub async fn get_1inch_quote(
        &self,
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
    ) -> Result<Option<ZeroExQuote>> {
        let url = format!(
            "https://api.1inch.dev/swap/v5.2/1/quote?src={}&dst={}&amount={}",
            format!("{:#x}", sell_token),
            format!("{:#x}", buy_token),
            sell_amount.to_string()
        );
        
        let client = reqwest::Client::new();
        let mut req = client.get(&url).header("accept", "application/json");
        
        // API 키가 있으면 추가
        if let Ok(key) = std::env::var("ONEINCH_API_KEY") {
            if !key.trim().is_empty() {
                req = req
                    .header("Authorization", format!("Bearer {}", key))
                    .header("apikey", key);
            }
        }
        
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        
        #[derive(serde::Deserialize, Default)]
        struct OneInchQuoteWire {
            to: Option<String>,
            data: Option<String>,
            value: Option<String>,
        }
        
        let q: OneInchQuoteWire = resp.json().await.unwrap_or_default();
        let to_str = match q.to { 
            Some(t) if !t.is_empty() => t, 
            _ => return Ok(None) 
        };
        let data_str = match q.data { 
            Some(d) if !d.is_empty() => d, 
            _ => return Ok(None) 
        };
        
        let to: Address = to_str.parse()?;
        let data_bytes = hex::decode(data_str.trim_start_matches("0x")).unwrap_or_default();
        let value = if let Some(v) = q.value { 
            Some(U256::from_str_radix(&v, 10).unwrap_or(U256::zero())) 
        } else { 
            None 
        };
        
        Ok(Some(ZeroExQuote { 
            to, 
            data: Bytes::from(data_bytes), 
            value, 
            allowance_target: None // 1inch는 allowanceTarget을 제공하지 않음
        }))
    }

    /// 자산 가격 조회 (캐시)
    pub async fn get_price(&self, asset: Address) -> Option<AssetPrice> {
        let prices = self.asset_prices.lock().await;
        prices.get(&asset).cloned()
    }

    /// 자산 가격 업데이트
    pub async fn update_price(&self, asset: Address, price: AssetPrice) {
        let mut prices = self.asset_prices.lock().await;
        prices.insert(asset, price);
    }

    /// Chainlink Oracle에서 가격 조회
    async fn get_chainlink_price(&self, feed_address: Address) -> Result<f64> {
        // Provider가 없으면 CoinGecko 폴백
        let provider = match &self.provider {
            Some(p) => p.clone(),
            None => {
                tracing::warn!("⚠️ Provider not set, falling back to CoinGecko for Chainlink price");
                return self.get_coingecko_fallback().await;
            }
        };

        // Chainlink ABI 로드
        let abi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("abi")
            .join("ChainlinkAggregator.json");

        let abi_bytes = tokio::fs::read(&abi_path).await.map_err(|e| {
            anyhow::anyhow!("Failed to read Chainlink ABI: {}", e)
        })?;

        let abi: ethers::abi::Abi = serde_json::from_slice(&abi_bytes)?;

        // Contract 인스턴스 생성
        let contract = Contract::new(feed_address, abi, provider);

        // latestRoundData() 호출
        let result: (u80, I256, U256, U256, u80) = contract
            .method::<_, (u80, I256, U256, U256, u80)>("latestRoundData", ())?
            .call()
            .await
            .map_err(|e| {
                tracing::warn!("⚠️ Chainlink latestRoundData failed: {}, using CoinGecko fallback", e);
                e
            })?;

        let (_round_id, answer, _started_at, updated_at, _answered_in_round) = result;

        // 가격 검증: updated_at이 1시간 이상 오래되었으면 경고
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let updated_at_secs = updated_at.as_u64();
        if now - updated_at_secs > 3600 {
            tracing::warn!("⚠️ Chainlink price data is stale (updated {} seconds ago)", now - updated_at_secs);
        }

        // answer는 int256이고 보통 8 decimals
        // answer를 f64로 변환 (8 decimals 가정)
        let price = if answer.is_negative() {
            tracing::error!("❌ Chainlink returned negative price: {}", answer);
            return Err(anyhow::anyhow!("Negative price from Chainlink"));
        } else {
            let answer_u256 = answer.into_raw();
            let price_f64 = answer_u256.as_u128() as f64 / 1e8;
            price_f64
        };

        tracing::debug!("✅ Chainlink price for {:?}: ${:.2}", feed_address, price);
        Ok(price)
    }

    /// CoinGecko 폴백 가격 조회
    async fn get_coingecko_fallback(&self) -> Result<f64> {
        let url = "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd";
        let response = self.http_client.get(url).send().await?;

        if response.status().is_success() {
            let data: Value = response.json().await?;
            if let Some(eth_data) = data.get("ethereum") {
                if let Some(price) = eth_data.get("usd") {
                    return Ok(price.as_f64().unwrap_or(2800.0));
                }
            }
        }

        // 최종 폴백: 기본 가격
        Ok(2800.0)
    }

    /// Uniswap에서 가격 조회
    async fn get_uniswap_price(&self, token_in: Address, token_out: Address, amount_in: U256) -> Result<f64> {
        // Uniswap V3 Subgraph API 사용
        let query = format!(
            r#"{{
                "query": "query GetPrice($tokenIn: String!, $tokenOut: String!, $amountIn: String!) {{
                    token0: token(id: $tokenIn) {{ symbol, decimals }}
                    token1: token(id: $tokenOut) {{ symbol, decimals }}
                    pools(where: {{ token0: $tokenIn, token1: $tokenOut }}) {{
                        token0Price
                        token1Price
                    }}
                }}",
                "variables": {{
                    "tokenIn": "{:#x}",
                    "tokenOut": "{:#x}",
                    "amountIn": "{}"
                }}
            }}"#,
            token_in, token_out, amount_in
        );

        let response = self.http_client
            .post("https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v3")
            .header("Content-Type", "application/json")
            .body(query)
            .send()
            .await?;

        if response.status().is_success() {
            let data: Value = response.json().await?;
            if let Some(pools) = data["data"]["pools"].as_array() {
                if let Some(pool) = pools.first() {
                    if let Some(price) = pool["token0Price"].as_str() {
                        return Ok(price.parse().unwrap_or(1.0));
                    }
                }
            }
        }

        // 폴백: 기본 가격
        Ok(1.0)
    }

    /// 자산 심볼 조회
    fn get_asset_symbol(&self, asset: Address) -> &'static str {
        match format!("{:#x}", asset).as_str() {
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2" => "WETH",
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48" => "USDC",
            "0xdAC17F958D2ee523a2206206994597C13D831ec7" => "USDT",
            "0x6B175474E89094C44Da98b954EedeAC495271d0F" => "DAI",
            _ => "UNKNOWN",
        }
    }

    /// 주기적 가격 업데이트 시작
    pub async fn start_price_updater(&self) -> Result<()> {
        let oracle = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Chainlink에서 가격 업데이트
                if let Err(e) = oracle.update_prices_from_chainlink().await {
                    error!("Chainlink 가격 업데이트 실패: {}", e);
                }
                
                // DEX에서 가격 업데이트
                if let Err(e) = oracle.update_prices_from_dex().await {
                    error!("DEX 가격 업데이트 실패: {}", e);
                }
            }
        });
        
        Ok(())
    }
}
