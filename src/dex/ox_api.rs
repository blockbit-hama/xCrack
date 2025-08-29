use std::collections::HashMap;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use alloy::primitives::{Address, U256};
use async_trait::async_trait;

use super::{DexAggregator, DexType, SwapQuote, SwapParams, SwapSource};

/// 0x Protocol API Integration
pub struct ZeroXAggregator {
    client: Client,
    api_key: Option<String>,
    base_url: String,
    chain_id: u64,
}

#[derive(Debug, Deserialize)]
struct ZeroXQuoteResponse {
    #[serde(rename = "sellTokenAddress")]
    sell_token_address: String,
    #[serde(rename = "buyTokenAddress")]
    buy_token_address: String,
    #[serde(rename = "sellAmount")]
    sell_amount: String,
    #[serde(rename = "buyAmount")]
    buy_amount: String,
    #[serde(rename = "buyAmountMin", default)]
    buy_amount_min: Option<String>,
    #[serde(rename = "allowanceTarget")]
    allowance_target: String,
    to: String,
    data: String,
    value: String,
    gas: String,
    #[serde(rename = "gasPrice")]
    gas_price: String,
    #[serde(rename = "estimatedPriceImpact", default)]
    estimated_price_impact: Option<String>,
    sources: Vec<ZeroXSource>,
    #[serde(rename = "estimatedGas", default)]
    estimated_gas: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ZeroXSource {
    name: String,
    proportion: String,
}

#[derive(Debug, Deserialize)]
struct ZeroXPriceResponse {
    price: String,
    #[serde(rename = "estimatedPriceImpact")]
    estimated_price_impact: String,
}

impl ZeroXAggregator {
    pub fn new(api_key: Option<String>, chain_id: u64) -> Self {
        let base_url = match chain_id {
            1 => "https://api.0x.org".to_string(),
            137 => "https://polygon.api.0x.org".to_string(),
            56 => "https://bsc.api.0x.org".to_string(),
            42161 => "https://arbitrum.api.0x.org".to_string(),
            10 => "https://optimism.api.0x.org".to_string(),
            _ => "https://api.0x.org".to_string(),
        };
        
        Self {
            client: Client::new(),
            api_key,
            base_url,
            chain_id,
        }
    }
    
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        
        if let Some(ref api_key) = self.api_key {
            headers.insert("0x-api-key", api_key.parse().unwrap());
        }
        
        headers.insert(
            "Content-Type", 
            "application/json".parse().unwrap()
        );
        
        headers
    }
    
    async fn get_quote_internal(&self, params: SwapParams) -> Result<ZeroXQuoteResponse> {
        let mut query_params = HashMap::new();
        
        query_params.insert("sellToken", format!("{:#x}", params.sell_token));
        query_params.insert("buyToken", format!("{:#x}", params.buy_token));
        query_params.insert("sellAmount", params.sell_amount.to_string());
        
        if params.slippage_tolerance > 0.0 {
            query_params.insert("slippagePercentage", params.slippage_tolerance.to_string());
        }
        
        if let Some(recipient) = params.recipient {
            query_params.insert("takerAddress", format!("{:#x}", recipient));
        }
        
        if !params.exclude_sources.is_empty() {
            query_params.insert("excludedSources", params.exclude_sources.join(","));
        }
        
        if !params.include_sources.is_empty() {
            query_params.insert("includedSources", params.include_sources.join(","));
        }
        
        if let Some(fee_recipient) = params.fee_recipient {
            query_params.insert("feeRecipient", format!("{:#x}", fee_recipient));
        }
        
        if let Some(fee_percentage) = params.buy_token_percentage_fee {
            query_params.insert("buyTokenPercentageFee", fee_percentage.to_string());
        }
        
        let url = format!("{}/swap/v1/quote", self.base_url);
        
        debug!("🔄 Requesting 0x quote: {} -> {} ({})", 
               params.sell_token, params.buy_token, params.sell_amount);
        
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .query(&query_params)
            .send()
            .await
            .map_err(|e| anyhow!("0x API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("0x API error {}: {}", response.status(), error_text));
        }
        
        let quote: ZeroXQuoteResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse 0x response: {}", e))?;
        
        debug!("✅ 0x quote received: {} -> {}", quote.sell_amount, quote.buy_amount);
        
        Ok(quote)
    }
    
    async fn get_price_internal(&self, sell_token: Address, buy_token: Address) -> Result<ZeroXPriceResponse> {
        let mut query_params = HashMap::new();
        query_params.insert("sellToken", format!("{:#x}", sell_token));
        query_params.insert("buyToken", format!("{:#x}", buy_token));
        query_params.insert("sellAmount", "1000000000000000000"); // 1 token (18 decimals)
        
        let url = format!("{}/swap/v1/price", self.base_url);
        
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .query(&query_params)
            .send()
            .await
            .map_err(|e| anyhow!("0x price API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("0x price API error {}: {}", response.status(), error_text));
        }
        
        response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse 0x price response: {}", e))
    }
}

#[async_trait]
impl DexAggregator for ZeroXAggregator {
    async fn get_quote(&self, params: SwapParams) -> Result<SwapQuote> {
        let quote_response = self.get_quote_internal(params.clone()).await?;
        
        // Parse response
        let sell_amount = U256::from_str_radix(&quote_response.sell_amount, 10)
            .map_err(|e| anyhow!("Invalid sell amount: {}", e))?;
            
        let buy_amount = U256::from_str_radix(&quote_response.buy_amount, 10)
            .map_err(|e| anyhow!("Invalid buy amount: {}", e))?;
        
        let buy_amount_min = if let Some(min_str) = quote_response.buy_amount_min {
            U256::from_str_radix(&min_str, 10).unwrap_or(buy_amount)
        } else {
            // Calculate min amount based on slippage tolerance
            let slippage_multiplier = 1.0 - params.slippage_tolerance;
            let min_amount_f64 = buy_amount.to::<u128>() as f64 * slippage_multiplier;
            U256::from(min_amount_f64 as u128)
        };
        
        let router_address = Address::from_hex(&quote_response.to)
            .map_err(|e| anyhow!("Invalid router address: {}", e))?;
            
        let allowance_target = Address::from_hex(&quote_response.allowance_target)
            .map_err(|e| anyhow!("Invalid allowance target: {}", e))?;
        
        // Parse calldata
        let calldata = hex::decode(quote_response.data.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid calldata: {}", e))?;
        
        let gas_estimate = quote_response.gas.parse::<u64>()
            .map_err(|e| anyhow!("Invalid gas estimate: {}", e))?;
        
        let gas_price = U256::from_str_radix(&quote_response.gas_price, 10)
            .map_err(|e| anyhow!("Invalid gas price: {}", e))?;
        
        let price_impact = quote_response.estimated_price_impact
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        // Convert sources
        let sources: Vec<SwapSource> = quote_response.sources
            .into_iter()
            .filter_map(|s| {
                s.proportion.parse::<f64>().ok().map(|prop| SwapSource {
                    name: s.name,
                    proportion: prop,
                })
            })
            .collect();
        
        Ok(SwapQuote {
            aggregator: DexType::ZeroX,
            sell_token: params.sell_token,
            buy_token: params.buy_token,
            sell_amount,
            buy_amount,
            buy_amount_min,
            router_address,
            calldata,
            allowance_target,
            gas_estimate,
            gas_price,
            price_impact,
            sources,
            estimated_execution_time_ms: 3000, // 0x typically executes in ~3s
            quote_timestamp: chrono::Utc::now(),
        })
    }
    
    async fn get_price(&self, sell_token: Address, buy_token: Address) -> Result<f64> {
        let price_response = self.get_price_internal(sell_token, buy_token).await?;
        
        price_response.price.parse::<f64>()
            .map_err(|e| anyhow!("Invalid price format: {}", e))
    }
    
    async fn get_liquidity(&self, _token: Address) -> Result<U256> {
        // 0x doesn't provide direct liquidity data
        // We could estimate based on recent quotes, but for now return 0
        Ok(U256::ZERO)
    }
    
    fn aggregator_type(&self) -> DexType {
        DexType::ZeroX
    }
    
    fn is_available(&self) -> bool {
        // 0x is generally available, could add health checks here
        true
    }
    
    fn supported_networks(&self) -> Vec<u64> {
        vec![1, 137, 56, 42161, 10] // Ethereum, Polygon, BSC, Arbitrum, Optimism
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    
    #[tokio::test]
    async fn test_0x_quote() {
        let aggregator = ZeroXAggregator::new(None, 1);
        
        let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let usdc: Address = "0xA0b86a33E6417f8C681A1fE6954e127c9cd8e46".parse().unwrap();
        
        let params = SwapParams {
            sell_token: weth,
            buy_token: usdc,
            sell_amount: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            slippage_tolerance: 0.01, // 1%
            recipient: None,
            deadline_seconds: None,
            exclude_sources: vec![],
            include_sources: vec![],
            fee_recipient: None,
            buy_token_percentage_fee: None,
        };
        
        match aggregator.get_quote(params).await {
            Ok(quote) => {
                println!("✅ 0x Quote successful:");
                println!("  Buy amount: {} USDC", quote.buy_amount);
                println!("  Gas estimate: {}", quote.gas_estimate);
                println!("  Price impact: {:.4}%", quote.price_impact * 100.0);
            }
            Err(e) => {
                println!("❌ 0x Quote failed: {}", e);
                // Don't fail test as it requires API access
            }
        }
    }
}