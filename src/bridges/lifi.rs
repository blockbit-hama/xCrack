use super::traits::{Bridge, BridgeQuote, BridgeError, BridgeResult, BridgeExecution, BridgeExecutionStatus};
use crate::types::{ChainId, CrossChainToken};
use alloy::primitives::{U256, Address};
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::json;
use reqwest::{Client, header};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};
use anyhow::Result;

/// LI.FI API base URL
const LIFI_API_BASE: &str = "https://li.quest/v1";

/// Supported chains mapping (LI.FI chain IDs)
fn map_chain_id(chain: ChainId) -> u64 {
    match chain {
        ChainId::Ethereum => 1,
        ChainId::Polygon => 137,
        ChainId::BSC => 56,
        ChainId::Arbitrum => 42161,
        ChainId::Optimism => 10,
        ChainId::Avalanche => 43114,
        _ => 1, // Default to Ethereum
    }
}

/// LI.FI Quote Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiFiQuoteRequest {
    from_chain: u64,
    to_chain: u64,
    from_token: String,
    to_token: String,
    from_amount: String,
    from_address: String,
    to_address: String,
    slippage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_bridges: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deny_bridges: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefer_bridges: Option<Vec<String>>,
}

/// LI.FI Quote Response
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiFiQuoteResponse {
    pub id: String,
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub from_token: TokenInfo,
    pub to_token: TokenInfo,
    pub from_amount: String,
    pub to_amount: String,
    pub to_amount_min: String,
    pub estimate: RouteEstimate,
    pub included_steps: Vec<StepInfo>,
    pub transaction_request: Option<TransactionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenInfo {
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub name: String,
    pub chain_id: u64,
    pub price_usd: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteEstimate {
    pub from_amount: String,
    pub to_amount: String,
    pub to_amount_min: String,
    pub approval_address: Option<String>,
    pub execution_duration: u64,
    pub fee_costs: Option<Vec<FeeCost>>,
    pub gas_costs: Option<Vec<GasCost>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeeCost {
    pub name: String,
    pub description: String,
    pub percentage: f64,
    pub amount: String,
    pub included: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GasCost {
    pub type_field: String,
    pub estimate: String,
    pub limit: String,
    pub amount: String,
    pub amount_usd: Option<f64>,
    pub token: TokenInfo,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepInfo {
    pub id: String,
    pub type_field: String,
    pub action: StepAction,
    pub estimate: StepEstimate,
    pub tool: String,
    pub tool_details: ToolDetails,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepAction {
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub from_token: TokenInfo,
    pub to_token: TokenInfo,
    pub from_amount: String,
    pub slippage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepEstimate {
    pub from_amount: String,
    pub to_amount: String,
    pub to_amount_min: String,
    pub execution_duration: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolDetails {
    pub key: String,
    pub name: String,
    pub logo_uri: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionRequest {
    pub data: String,
    pub to: String,
    pub value: String,
    pub from: String,
    pub chain_id: u64,
    pub gas_limit: String,
    pub gas_price: Option<String>,
}

/// LI.FI Status Response
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiFiStatusResponse {
    pub transaction_id: String,
    pub sending: TransactionStatus,
    pub receiving: Option<TransactionStatus>,
    pub status: String,
    pub substatus: Option<String>,
    pub substatus_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionStatus {
    pub tx_hash: String,
    pub chain_id: u64,
    pub amount: String,
    pub token: TokenInfo,
    pub gas_price: String,
    pub gas_used: String,
    pub timestamp: u64,
}

/// LI.FI Bridge implementation
pub struct LiFiBridge {
    client: Client,
    api_key: Option<String>,
    integrator: String,
    /// Mock mode flag
    mock_mode: bool,
    /// Preferred bridges
    preferred_bridges: Vec<String>,
    /// Denied bridges (for safety)
    denied_bridges: Vec<String>,
    /// Max retry attempts
    max_retries: u32,
    /// Request timeout
    timeout: std::time::Duration,
}

impl std::fmt::Debug for LiFiBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiFiBridge")
            .field("integrator", &self.integrator)
            .field("mock_mode", &self.mock_mode)
            .field("preferred_bridges", &self.preferred_bridges)
            .field("denied_bridges", &self.denied_bridges)
            .field("max_retries", &self.max_retries)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl LiFiBridge {
    pub fn new(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            integrator: "xCrack".to_string(),
            mock_mode: crate::mocks::is_mock_mode(),
            preferred_bridges: vec![
                "hop".to_string(),
                "stargate".to_string(),
                "across".to_string(),
                "cbridge".to_string(),
            ],
            denied_bridges: vec![], // Add risky bridges here
            max_retries: 3,
            timeout: std::time::Duration::from_secs(30),
        }
    }

    /// Get quote from LI.FI API
    async fn get_lifi_quote(&self, request: LiFiQuoteRequest) -> Result<LiFiQuoteResponse> {
        if self.mock_mode {
            return self.get_mock_quote(request).await;
        }

        let url = format!("{}/quote", LIFI_API_BASE);
        
        let mut headers = header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse()?);
        if let Some(api_key) = &self.api_key {
            headers.insert("x-lifi-api-key", api_key.parse()?);
        }

        let mut retry_count = 0;
        loop {
            let response = self.client
                .get(&url)
                .headers(headers.clone())
                .query(&request)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let quote: LiFiQuoteResponse = resp.json().await?;
                        return Ok(quote);
                    } else {
                        let status = resp.status();
                        let error_text = resp.text().await.unwrap_or_default();
                        
                        if status.as_u16() == 429 && retry_count < self.max_retries {
                            // Rate limited, wait and retry
                            warn!("LI.FI rate limited, retrying in 2 seconds...");
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            retry_count += 1;
                            continue;
                        }
                        
                        return Err(anyhow::anyhow!("LI.FI API error {}: {}", status, error_text));
                    }
                }
                Err(e) => {
                    if retry_count < self.max_retries {
                        warn!("LI.FI request failed, retrying: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        retry_count += 1;
                        continue;
                    }
                    return Err(anyhow::anyhow!("LI.FI request failed: {}", e));
                }
            }
        }
    }

    /// Get mock quote for testing
    async fn get_mock_quote(&self, request: LiFiQuoteRequest) -> Result<LiFiQuoteResponse> {
        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let to_amount = (request.from_amount.parse::<u128>().unwrap_or(0) * 98 / 100).to_string();
        let to_amount_min = (request.from_amount.parse::<u128>().unwrap_or(0) * 95 / 100).to_string();
        
        Ok(LiFiQuoteResponse {
            id: Uuid::new_v4().to_string(),
            from_chain_id: request.from_chain,
            to_chain_id: request.to_chain,
            from_token: TokenInfo {
                address: request.from_token.clone(),
                symbol: "USDC".to_string(),
                decimals: 6,
                name: "USD Coin".to_string(),
                chain_id: request.from_chain,
                price_usd: Some(1.0),
            },
            to_token: TokenInfo {
                address: request.to_token.clone(),
                symbol: "USDC".to_string(),
                decimals: 6,
                name: "USD Coin".to_string(),
                chain_id: request.to_chain,
                price_usd: Some(1.0),
            },
            from_amount: request.from_amount.clone(),
            to_amount: to_amount.clone(),
            to_amount_min: to_amount_min.clone(),
            estimate: RouteEstimate {
                from_amount: request.from_amount.clone(),
                to_amount,
                to_amount_min,
                approval_address: None,
                execution_duration: 300, // 5 minutes
                fee_costs: Some(vec![]),
                gas_costs: Some(vec![]),
            },
            included_steps: vec![],
            transaction_request: None,
        })
    }

    /// Check transaction status
    async fn check_status(&self, tx_id: &str) -> Result<LiFiStatusResponse> {
        if self.mock_mode {
            return self.get_mock_status(tx_id).await;
        }

        let url = format!("{}/status", LIFI_API_BASE);
        
        let mut headers = header::HeaderMap::new();
        if let Some(api_key) = &self.api_key {
            headers.insert("x-lifi-api-key", api_key.parse()?);
        }

        let response = self.client
            .get(&url)
            .headers(headers)
            .query(&[("txHash", tx_id)])
            .send()
            .await?;

        if response.status().is_success() {
            let status: LiFiStatusResponse = response.json().await?;
            Ok(status)
        } else {
            Err(anyhow::anyhow!("Failed to get status: {}", response.status()))
        }
    }

    /// Get mock status for testing
    async fn get_mock_status(&self, tx_id: &str) -> Result<LiFiStatusResponse> {
        Ok(LiFiStatusResponse {
            transaction_id: tx_id.to_string(),
            sending: TransactionStatus {
                tx_hash: format!("0x{}", "a".repeat(64)),
                chain_id: 1,
                amount: "1000000".to_string(),
                token: TokenInfo {
                    address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
                    symbol: "USDC".to_string(),
                    decimals: 6,
                    name: "USD Coin".to_string(),
                    chain_id: 1,
                    price_usd: Some(1.0),
                },
                gas_price: "20000000000".to_string(),
                gas_used: "100000".to_string(),
                timestamp: Utc::now().timestamp() as u64,
            },
            receiving: None,
            status: "PENDING".to_string(),
            substatus: None,
            substatus_message: None,
        })
    }

    /// Get supported chains
    async fn get_chains(&self) -> Result<Vec<ChainInfo>> {
        if self.mock_mode {
            return Ok(vec![
                ChainInfo { id: 1, name: "Ethereum".to_string() },
                ChainInfo { id: 137, name: "Polygon".to_string() },
                ChainInfo { id: 56, name: "BSC".to_string() },
                ChainInfo { id: 42161, name: "Arbitrum".to_string() },
                ChainInfo { id: 10, name: "Optimism".to_string() },
            ]);
        }

        let url = format!("{}/chains", LIFI_API_BASE);
        let response = self.client.get(&url).send().await?;
        
        if response.status().is_success() {
            let chains: ChainsResponse = response.json().await?;
            Ok(chains.chains)
        } else {
            Err(anyhow::anyhow!("Failed to get chains"))
        }
    }

    /// Get supported tokens for a chain
    async fn get_tokens(&self, chain_id: u64) -> Result<Vec<TokenInfo>> {
        if self.mock_mode {
            return Ok(vec![
                TokenInfo {
                    address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
                    symbol: "USDC".to_string(),
                    decimals: 6,
                    name: "USD Coin".to_string(),
                    chain_id,
                    price_usd: Some(1.0),
                },
                TokenInfo {
                    address: "0xdac17f958d2ee523a2206206994597c13d831ec7".to_string(),
                    symbol: "USDT".to_string(),
                    decimals: 6,
                    name: "Tether USD".to_string(),
                    chain_id,
                    price_usd: Some(1.0),
                },
            ]);
        }

        let url = format!("{}/tokens", LIFI_API_BASE);
        let response = self.client
            .get(&url)
            .query(&[("chains", chain_id.to_string())])
            .send()
            .await?;
        
        if response.status().is_success() {
            let tokens: TokensResponse = response.json().await?;
            Ok(tokens.tokens.get(&chain_id.to_string()).cloned().unwrap_or_default())
        } else {
            Err(anyhow::anyhow!("Failed to get tokens"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainsResponse {
    chains: Vec<ChainInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainInfo {
    id: u64,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokensResponse {
    tokens: HashMap<String, Vec<TokenInfo>>,
}

impl Default for LiFiBridge {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Bridge for LiFiBridge {
    fn name(&self) -> &'static str {
        "LI.FI"
    }
    
    async fn supports_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<bool> {
        // LI.FI supports most major chains
        let supported_chains = [
            ChainId::Ethereum,
            ChainId::Polygon,
            ChainId::BSC,
            ChainId::Arbitrum,
            ChainId::Optimism,
            ChainId::Avalanche,
        ];
        
        Ok(supported_chains.contains(&from) && 
           supported_chains.contains(&to) && 
           from != to)
    }
    
    async fn get_routes(&self, token: &CrossChainToken) -> BridgeResult<Vec<(ChainId, ChainId)>> {
        let supported_chains = [
            ChainId::Ethereum,
            ChainId::Polygon,
            ChainId::BSC,
            ChainId::Arbitrum,
            ChainId::Optimism,
            ChainId::Avalanche,
        ];
        
        let mut routes = Vec::new();
        for &from in &supported_chains {
            for &to in &supported_chains {
                if from != to {
                    routes.push((from, to));
                }
            }
        }
        
        Ok(routes)
    }
    
    async fn get_quote(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
        amount: U256,
        slippage: f64,
    ) -> BridgeResult<BridgeQuote> {
        // Convert amount to string
        let amount_str = amount.to_string();
        
        // Create LI.FI quote request
        let request = LiFiQuoteRequest {
            from_chain: map_chain_id(from),
            to_chain: map_chain_id(to),
            from_token: format!("{:?}", token.addresses.get(&from).unwrap_or(&Address::ZERO)),
            to_token: format!("{:?}", token.addresses.get(&to).unwrap_or(&Address::ZERO)), // Same token on destination
            from_amount: amount_str,
            from_address: "0x0000000000000000000000000000000000000000".to_string(),
            to_address: "0x0000000000000000000000000000000000000000".to_string(),
            slippage,
            integrator: Some(self.integrator.clone()),
            allow_bridges: Some(self.preferred_bridges.clone()),
            deny_bridges: if self.denied_bridges.is_empty() { None } else { Some(self.denied_bridges.clone()) },
            prefer_bridges: None,
        };
        
        // Get quote from LI.FI
        let lifi_quote = self.get_lifi_quote(request).await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;
        
        // Convert to our BridgeQuote format
        let output_amount = U256::from_str_radix(&lifi_quote.to_amount, 10)
            .unwrap_or(U256::ZERO);
        
        let fee_amount = amount.saturating_sub(output_amount);
        let fee_percentage = if amount > U256::ZERO {
            (fee_amount.to::<u128>() as f64 / amount.to::<u128>() as f64) * 100.0
        } else {
            0.0
        };
        
        let quote_id = lifi_quote.id.clone();
        let gas_fee = U256::from(lifi_quote.estimate.gas_costs.as_ref()
            .and_then(|costs| costs.first())
            .map(|g| g.amount.parse::<u64>().unwrap_or(0))
            .unwrap_or(0));
        let estimated_time = lifi_quote.estimate.execution_duration as u64;
        let route_data = serde_json::to_value(&lifi_quote).unwrap_or(serde_json::Value::Null);
        
        Ok(BridgeQuote {
            quote_id,
            source_chain: from,
            destination_chain: to,
            token: token.clone(),
            amount_in: amount,
            amount_out: output_amount,
            bridge_fee: fee_amount,
            gas_fee,
            protocol_fee: U256::ZERO,
            exchange_rate: output_amount.to::<u128>() as f64 / amount.to::<u128>() as f64,
            price_impact: 0.1, // Default 0.1%
            estimated_time,
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            route_data,
            slippage_tolerance: slippage,
        })
    }
    
    async fn execute_bridge(
        &self,
        quote: &BridgeQuote,
    ) -> BridgeResult<BridgeExecution> {
        // In production, this would submit the transaction
        // For now, return a mock execution
        
        info!("Executing bridge via LI.FI: {} from {:?} to {:?}", 
            quote.token.symbol, quote.source_chain, quote.destination_chain);
        
        let execution_id = Uuid::new_v4().to_string();
        
        Ok(BridgeExecution {
            execution_id: execution_id.clone(),
            source_tx_hash: format!("0x{}", "a".repeat(64)),
            destination_tx_hash: None,
            status: BridgeExecutionStatus::Pending,
            amount_received: None,
            fees_paid: U256::ZERO,
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
        })
    }
    
    async fn get_execution_status(
        &self,
        execution_id: &str,
    ) -> BridgeResult<BridgeExecution> {
        // Check status via LI.FI API
        let status = self.check_status(execution_id).await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;
        
        let execution_status = match status.status.as_str() {
            "DONE" => BridgeExecutionStatus::Completed,
            "FAILED" => BridgeExecutionStatus::Failed,
            _ => BridgeExecutionStatus::Pending,
        };
        
        let is_completed = matches!(execution_status, BridgeExecutionStatus::Completed);
        
        Ok(BridgeExecution {
            execution_id: execution_id.to_string(),
            source_tx_hash: status.sending.tx_hash,
            destination_tx_hash: status.receiving.map(|r| r.tx_hash),
            status: execution_status,
            amount_received: None, // Would need to parse from status
            fees_paid: U256::ZERO, // Would need to parse from status
            started_at: DateTime::from_timestamp(status.sending.timestamp as i64, 0).unwrap_or(Utc::now()),
            completed_at: if is_completed {
                Some(Utc::now())
            } else {
                None
            },
            error_message: status.substatus_message,
        })
    }
    
    async fn get_liquidity(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
    ) -> BridgeResult<U256> {
        // LI.FI aggregates multiple bridges, so liquidity is typically high
        // In production, we could query actual liquidity from the API
        
        if self.mock_mode {
            // Return mock liquidity
            Ok(U256::from(10_000_000u64) * U256::from(10u64.pow(token.decimals as u32)))
        } else {
            // In production, query actual available liquidity
            // For now, return a reasonable estimate
            Ok(U256::from(5_000_000u64) * U256::from(10u64.pow(token.decimals as u32)))
        }
    }

    async fn get_success_rate(&self) -> BridgeResult<f64> {
        // LI.FI has a high success rate due to aggregation
        Ok(0.98) // 98% success rate
    }

    async fn get_avg_completion_time(&self, _from: ChainId, _to: ChainId) -> BridgeResult<u64> {
        // Average completion time varies by route and bridge used
        Ok(180) // 3 minutes average
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lifi_bridge_creation() {
        let bridge = LiFiBridge::new(None);
        assert_eq!(bridge.name(), "LI.FI");
    }

    #[tokio::test]
    async fn test_supports_route() {
        let bridge = LiFiBridge::new(None);
        let token = CrossChainToken {
            symbol: "USDC".to_string(),
            address: Address::ZERO,
            decimals: 6,
            chain_id: ChainId::Ethereum,
        };
        
        // Should support Ethereum to Polygon
        let supported = bridge.supports_route(
            ChainId::Ethereum,
            ChainId::Polygon,
            &token
        ).await.unwrap();
        assert!(supported);
        
        // Should not support same chain
        let supported = bridge.supports_route(
            ChainId::Ethereum,
            ChainId::Ethereum,
            &token
        ).await.unwrap();
        assert!(!supported);
    }

    #[tokio::test]
    async fn test_mock_quote() {
        std::env::set_var("API_MODE", "mock");
        let bridge = LiFiBridge::new(None);
        
        let token = CrossChainToken {
            symbol: "USDC".to_string(),
            address: Address::ZERO,
            decimals: 6,
            chain_id: ChainId::Ethereum,
        };
        
        let amount = U256::from(1_000_000u64); // 1 USDC
        
        let quote = bridge.get_quote(
            ChainId::Ethereum,
            ChainId::Polygon,
            &token,
            amount,
            0.5
        ).await.unwrap();
        
        assert_eq!(quote.bridge_name, "LI.FI");
        assert!(quote.output_amount > U256::ZERO);
        assert!(quote.output_amount < amount); // Should have some fees
        std::env::remove_var("API_MODE");
    }
}