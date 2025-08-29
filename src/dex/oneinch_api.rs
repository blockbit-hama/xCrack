use std::collections::HashMap;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use alloy::primitives::{Address, U256};
use async_trait::async_trait;

use super::{DexAggregator, DexType, SwapQuote, SwapParams, SwapSource};

/// 1inch API Integration
pub struct OneInchAggregator {
    client: Client,
    api_key: Option<String>,
    base_url: String,
    chain_id: u64,
}

#[derive(Debug, Deserialize)]
struct OneInchQuoteResponse {
    #[serde(rename = "fromToken")]
    from_token: OneInchToken,
    #[serde(rename = "toToken")]
    to_token: OneInchToken,
    #[serde(rename = "toTokenAmount")]
    to_token_amount: String,
    #[serde(rename = "fromTokenAmount")]
    from_token_amount: String,
    protocols: Vec<Vec<Vec<OneInchProtocol>>>,
    #[serde(rename = "estimatedGas")]
    estimated_gas: u64,
}

#[derive(Debug, Deserialize)]
struct OneInchSwapResponse {
    #[serde(rename = "fromToken")]
    from_token: OneInchToken,
    #[serde(rename = "toToken")]
    to_token: OneInchToken,
    #[serde(rename = "toTokenAmount")]
    to_token_amount: String,
    #[serde(rename = "fromTokenAmount")]
    from_token_amount: String,
    protocols: Vec<Vec<Vec<OneInchProtocol>>>,
    tx: OneInchTransaction,
}

#[derive(Debug, Deserialize)]
struct OneInchToken {
    symbol: String,
    name: String,
    address: String,
    decimals: u8,
}

#[derive(Debug, Deserialize)]
struct OneInchProtocol {
    name: String,
    part: f64,
    #[serde(rename = "fromTokenAddress")]
    from_token_address: String,
    #[serde(rename = "toTokenAddress")]
    to_token_address: String,
}

#[derive(Debug, Deserialize)]
struct OneInchTransaction {
    from: String,
    to: String,
    data: String,
    value: String,
    gas: u64,
    #[serde(rename = "gasPrice")]
    gas_price: String,
}

impl OneInchAggregator {
    pub fn new(api_key: Option<String>, chain_id: u64) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.1inch.io/v5.0".to_string(),
            chain_id,
        }
    }
    
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        
        if let Some(ref api_key) = self.api_key {
            headers.insert("Authorization", format!("Bearer {}", api_key).parse().unwrap());
        }
        
        headers.insert(
            "Accept", 
            "application/json".parse().unwrap()
        );
        
        headers
    }
    
    async fn get_quote_internal(&self, params: SwapParams) -> Result<OneInchQuoteResponse> {
        let mut query_params = HashMap::new();
        
        query_params.insert("fromTokenAddress", format!("{:#x}", params.sell_token));
        query_params.insert("toTokenAddress", format!("{:#x}", params.buy_token));
        query_params.insert("amount", params.sell_amount.to_string());
        
        if let Some(recipient) = params.recipient {
            query_params.insert("fromAddress", format!("{:#x}", recipient));
        }
        
        if params.slippage_tolerance > 0.0 {
            let slippage_percent = (params.slippage_tolerance * 100.0) as u32;
            query_params.insert("slippage", slippage_percent.to_string());
        }
        
        if !params.exclude_sources.is_empty() {
            query_params.insert("disabledProtocols", params.exclude_sources.join(","));
        }
        
        if let Some(fee_recipient) = params.fee_recipient {
            query_params.insert("referrerAddress", format!("{:#x}", fee_recipient));
        }
        
        if let Some(fee_percentage) = params.buy_token_percentage_fee {
            let fee_bps = (fee_percentage * 10000.0) as u32;
            query_params.insert("fee", fee_bps.to_string());
        }
        
        let url = format!("{}/{}/quote", self.base_url, self.chain_id);
        
        debug!("🔄 Requesting 1inch quote: {} -> {} ({})", 
               params.sell_token, params.buy_token, params.sell_amount);
        
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .query(&query_params)
            .send()
            .await
            .map_err(|e| anyhow!("1inch API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("1inch API error {}: {}", response.status(), error_text));
        }
        
        let quote: OneInchQuoteResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse 1inch quote response: {}", e))?;
        
        debug!("✅ 1inch quote received: {} -> {}", quote.from_token_amount, quote.to_token_amount);
        
        Ok(quote)
    }
    
    async fn get_swap_internal(&self, params: SwapParams) -> Result<OneInchSwapResponse> {
        let mut query_params = HashMap::new();
        
        query_params.insert("fromTokenAddress", format!("{:#x}", params.sell_token));
        query_params.insert("toTokenAddress", format!("{:#x}", params.buy_token));
        query_params.insert("amount", params.sell_amount.to_string());
        
        // 1inch requires fromAddress for swap endpoint
        let from_address = params.recipient
            .ok_or_else(|| anyhow!("1inch swap requires fromAddress"))?;
        query_params.insert("fromAddress", format!("{:#x}", from_address));
        
        if params.slippage_tolerance > 0.0 {
            let slippage_percent = (params.slippage_tolerance * 100.0) as u32;
            query_params.insert("slippage", slippage_percent.to_string());
        }
        
        if !params.exclude_sources.is_empty() {
            query_params.insert("disabledProtocols", params.exclude_sources.join(","));
        }
        
        if let Some(fee_recipient) = params.fee_recipient {
            query_params.insert("referrerAddress", format!("{:#x}", fee_recipient));
        }
        
        if let Some(fee_percentage) = params.buy_token_percentage_fee {
            let fee_bps = (fee_percentage * 10000.0) as u32;
            query_params.insert("fee", fee_bps.to_string());
        }
        
        let url = format!("{}/{}/swap", self.base_url, self.chain_id);
        
        debug!("🔄 Requesting 1inch swap data: {} -> {} ({})", 
               params.sell_token, params.buy_token, params.sell_amount);
        
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .query(&query_params)
            .send()
            .await
            .map_err(|e| anyhow!("1inch swap API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("1inch swap API error {}: {}", response.status(), error_text));
        }
        
        let swap: OneInchSwapResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse 1inch swap response: {}", e))?;
        
        debug!("✅ 1inch swap data received");
        
        Ok(swap)
    }
}

#[async_trait]
impl DexAggregator for OneInchAggregator {
    async fn get_quote(&self, params: SwapParams) -> Result<SwapQuote> {
        // First get quote for price info
        let quote_response = self.get_quote_internal(params.clone()).await?;
        
        // Then get swap data for execution
        let swap_response = self.get_swap_internal(params.clone()).await?;
        
        // Parse amounts
        let sell_amount = U256::from_str_radix(&swap_response.from_token_amount, 10)
            .map_err(|e| anyhow!("Invalid sell amount: {}", e))?;
            
        let buy_amount = U256::from_str_radix(&swap_response.to_token_amount, 10)
            .map_err(|e| anyhow!("Invalid buy amount: {}", e))?;
        
        // Calculate minimum amount based on slippage
        let slippage_multiplier = 1.0 - params.slippage_tolerance;
        let min_amount_f64 = buy_amount.to::<u128>() as f64 * slippage_multiplier;
        let buy_amount_min = U256::from(min_amount_f64 as u128);
        
        let router_address = Address::from_hex(&swap_response.tx.to)
            .map_err(|e| anyhow!("Invalid router address: {}", e))?;
        
        // For 1inch, allowance target is usually the same as router
        let allowance_target = router_address;
        
        // Parse calldata
        let calldata = hex::decode(swap_response.tx.data.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid calldata: {}", e))?;
        
        let gas_estimate = swap_response.tx.gas;
        
        let gas_price = U256::from_str_radix(&swap_response.tx.gas_price, 10)
            .map_err(|e| anyhow!("Invalid gas price: {}", e))?;
        
        // Extract protocol sources
        let mut sources = Vec::new();
        for route_group in quote_response.protocols {
            for route in route_group {
                for protocol in route {
                    sources.push(SwapSource {
                        name: protocol.name,
                        proportion: protocol.part / 100.0, // 1inch uses percentage
                    });
                }
            }
        }
        
        // Estimate price impact (1inch doesn't provide direct price impact)
        let price_impact = 0.0; // TODO: Calculate based on quote vs market price
        
        Ok(SwapQuote {
            aggregator: DexType::OneInch,
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
            estimated_execution_time_ms: 5000, // 1inch typically executes in ~5s
            quote_timestamp: chrono::Utc::now(),
        })
    }
    
    async fn get_price(&self, sell_token: Address, buy_token: Address) -> Result<f64> {
        let params = SwapParams {
            sell_token,
            buy_token,
            sell_amount: U256::from(1_000_000_000_000_000_000u128), // 1 token (18 decimals)
            slippage_tolerance: 0.01,
            recipient: Some(Address::ZERO), // Dummy address for price check
            deadline_seconds: None,
            exclude_sources: vec![],
            include_sources: vec![],
            fee_recipient: None,
            buy_token_percentage_fee: None,
        };
        
        let quote = self.get_quote_internal(params).await?;
        
        let sell_amount = quote.from_token_amount.parse::<f64>()
            .map_err(|e| anyhow!("Invalid sell amount: {}", e))?;
            
        let buy_amount = quote.to_token_amount.parse::<f64>()
            .map_err(|e| anyhow!("Invalid buy amount: {}", e))?;
        
        if sell_amount == 0.0 {
            return Err(anyhow!("Zero sell amount"));
        }
        
        Ok(buy_amount / sell_amount)
    }
    
    async fn get_liquidity(&self, _token: Address) -> Result<U256> {
        // 1inch doesn't provide direct liquidity data
        Ok(U256::ZERO)
    }
    
    fn aggregator_type(&self) -> DexType {
        DexType::OneInch
    }
    
    fn is_available(&self) -> bool {
        true
    }
    
    fn supported_networks(&self) -> Vec<u64> {
        vec![1, 56, 137, 43114, 250, 42161, 10] // Ethereum, BSC, Polygon, Avalanche, Fantom, Arbitrum, Optimism
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_1inch_quote() {
        let aggregator = OneInchAggregator::new(None, 1);
        
        let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let usdc: Address = "0xA0b86a33E6417f8C681A1fE6954e127c9cd8e46".parse().unwrap();
        let dummy_recipient: Address = "0x0000000000000000000000000000000000000001".parse().unwrap();
        
        let params = SwapParams {
            sell_token: weth,
            buy_token: usdc,
            sell_amount: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            slippage_tolerance: 0.01, // 1%
            recipient: Some(dummy_recipient),
            deadline_seconds: None,
            exclude_sources: vec![],
            include_sources: vec![],
            fee_recipient: None,
            buy_token_percentage_fee: None,
        };
        
        match aggregator.get_quote(params).await {
            Ok(quote) => {
                println!("✅ 1inch Quote successful:");
                println!("  Buy amount: {} USDC", quote.buy_amount);
                println!("  Gas estimate: {}", quote.gas_estimate);
                println!("  Sources: {:?}", quote.sources);
            }
            Err(e) => {
                println!("❌ 1inch Quote failed: {}", e);
                // Don't fail test as it requires API access
            }
        }
    }
}