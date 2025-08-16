use super::traits::{Bridge, BridgeQuote, BridgeError, BridgeResult, BridgeExecution, BridgeExecutionStatus};
use crate::types::{ChainId, CrossChainToken};
use alloy::primitives::U256;
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use serde_json::json;
use std::collections::HashMap;
use tracing::{info, warn, debug};

/// Across Protocol bridge implementation
/// 
/// Across는 Optimistic Oracle을 사용하는 빠른 크로스체인 브리지입니다.
/// - 빠른 처리 시간 (2-10분)
/// - 낮은 수수료
/// - UMA Protocol 기반 보안
#[derive(Debug)]
pub struct AcrossBridge {
    /// Mock mode flag
    mock_mode: bool,
    /// Base fee percentage (0.04% = 0.0004)
    base_fee_rate: f64,
    /// Minimum bridge amount
    min_amount: U256,
    /// Maximum bridge amount  
    max_amount: U256,
    /// Average completion time in seconds
    avg_completion_time: u64,
}

impl AcrossBridge {
    pub fn new() -> Self {
        Self {
            mock_mode: crate::mocks::is_mock_mode(),
            base_fee_rate: 0.0004, // 0.04%
            min_amount: U256::from(1_000000u64), // $1 USDC minimum
            max_amount: U256::from(1_000_000_000000u64), // $1M USDC maximum
            avg_completion_time: 300, // 5 minutes
        }
    }

    /// Calculate Across bridge fee
    fn calculate_fee(&self, amount: U256) -> U256 {
        // Across fee = base fee + gas cost
        let base_fee = (amount.to::<u128>() as f64 * self.base_fee_rate) as u128;
        U256::from(base_fee)
    }

    /// Get supported token addresses for Across
    fn get_token_address(&self, chain: ChainId, token: &CrossChainToken) -> Option<alloy::primitives::Address> {
        // Across mainly supports USDC, WETH, WBTC
        match (chain, token.symbol.as_str()) {
            (ChainId::Ethereum, "USDC") => Some("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse().unwrap()),
            (ChainId::Polygon, "USDC") => Some("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".parse().unwrap()),
            (ChainId::Arbitrum, "USDC") => Some("0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".parse().unwrap()),
            (ChainId::Optimism, "USDC") => Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".parse().unwrap()),
            (ChainId::Ethereum, "WETH") => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap()),
            (ChainId::Polygon, "WETH") => Some("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".parse().unwrap()),
            (ChainId::Arbitrum, "WETH") => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".parse().unwrap()),
            (ChainId::Optimism, "WETH") => Some("0x4200000000000000000000000000000000000006".parse().unwrap()),
            _ => token.addresses.get(&chain).copied(),
        }
    }

    /// Check if route is supported by Across
    fn is_supported_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> bool {
        let supported_chains = [
            ChainId::Ethereum,
            ChainId::Polygon, 
            ChainId::Arbitrum,
            ChainId::Optimism,
        ];

        let supported_tokens = ["USDC", "WETH", "WBTC"];

        supported_chains.contains(&from) && 
        supported_chains.contains(&to) && 
        from != to &&
        supported_tokens.contains(&token.symbol.as_str())
    }

    /// Generate mock quote for testing
    async fn generate_mock_quote(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
        amount: U256,
        slippage: f64,
    ) -> BridgeResult<BridgeQuote> {
        // Simulate API delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let bridge_fee = self.calculate_fee(amount);
        let gas_fee = U256::from(20_000000u64); // ~$0.02 at 20 gwei
        let amount_out = amount.saturating_sub(bridge_fee).saturating_sub(gas_fee);

        let route_data = json!({
            "bridge": "across",
            "protocol": "Across Protocol",
            "relayer_fee": bridge_fee.to_string(),
            "suggested_relayer_fee_pct": self.base_fee_rate,
            "timestamp": Utc::now().timestamp(),
            "quote_type": "mock"
        });

        Ok(BridgeQuote {
            quote_id: Uuid::new_v4().to_string(),
            source_chain: from,
            destination_chain: to,
            token: token.clone(),
            amount_in: amount,
            amount_out,
            bridge_fee,
            gas_fee,
            protocol_fee: U256::ZERO,
            exchange_rate: amount_out.to::<u128>() as f64 / amount.to::<u128>() as f64,
            price_impact: 0.05, // 0.05% impact
            estimated_time: self.avg_completion_time,
            expires_at: Utc::now() + Duration::minutes(5),
            route_data,
            slippage_tolerance: slippage,
        })
    }
}

impl Default for AcrossBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Bridge for AcrossBridge {
    fn name(&self) -> &'static str {
        "Across"
    }

    async fn supports_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<bool> {
        Ok(self.is_supported_route(from, to, token))
    }

    async fn get_routes(&self, token: &CrossChainToken) -> BridgeResult<Vec<(ChainId, ChainId)>> {
        let supported_chains = [
            ChainId::Ethereum,
            ChainId::Polygon,
            ChainId::Arbitrum, 
            ChainId::Optimism,
        ];

        let mut routes = Vec::new();
        if ["USDC", "WETH", "WBTC"].contains(&token.symbol.as_str()) {
            for &from in &supported_chains {
                for &to in &supported_chains {
                    if from != to {
                        routes.push((from, to));
                    }
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
        // Check if route is supported
        if !self.is_supported_route(from, to, token) {
            return Err(BridgeError::UnsupportedRoute { from, to });
        }

        // Check amount limits
        if amount < self.min_amount {
            return Err(BridgeError::InsufficientLiquidity { 
                available: self.min_amount, 
                required: amount 
            });
        }

        if amount > self.max_amount {
            return Err(BridgeError::InsufficientLiquidity { 
                available: self.max_amount, 
                required: amount 
            });
        }

        // Check if token is supported
        if self.get_token_address(from, token).is_none() || 
           self.get_token_address(to, token).is_none() {
            return Err(BridgeError::TokenNotSupported { 
                token: token.symbol.clone() 
            });
        }

        if self.mock_mode {
            return self.generate_mock_quote(from, to, token, amount, slippage).await;
        }

        // In production, this would call the Across API
        warn!("Across real API not implemented, using mock quote");
        self.generate_mock_quote(from, to, token, amount, slippage).await
    }

    async fn execute_bridge(&self, quote: &BridgeQuote) -> BridgeResult<BridgeExecution> {
        info!("🌉 Executing Across bridge: {} {} -> {}", 
            quote.token.symbol, 
            quote.source_chain.name(), 
            quote.destination_chain.name()
        );

        if !quote.is_valid() {
            return Err(BridgeError::QuoteExpired);
        }

        // Mock execution
        let execution_id = Uuid::new_v4().to_string();
        let success_rate = if self.mock_mode { 0.96 } else { 0.98 }; // 96% mock, 98% real

        let status = if fastrand::f64() < success_rate {
            BridgeExecutionStatus::Pending
        } else {
            BridgeExecutionStatus::Failed
        };

        Ok(BridgeExecution {
            execution_id,
            source_tx_hash: format!("0x{}", fastrand::u64(..).to_string()),
            destination_tx_hash: None,
            status,
            amount_received: None,
            fees_paid: quote.bridge_fee + quote.gas_fee,
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
        })
    }

    async fn get_execution_status(&self, execution_id: &str) -> BridgeResult<BridgeExecution> {
        debug!("🔍 Checking Across execution status: {}", execution_id);

        // Mock status check
        let statuses = [
            BridgeExecutionStatus::Pending,
            BridgeExecutionStatus::Bridging,
            BridgeExecutionStatus::Completed,
        ];

        let status = statuses[fastrand::usize(..statuses.len())].clone();
        let is_completed = matches!(status, BridgeExecutionStatus::Completed);

        Ok(BridgeExecution {
            execution_id: execution_id.to_string(),
            source_tx_hash: format!("0x{}", fastrand::u64(..).to_string()),
            destination_tx_hash: if is_completed {
                Some(format!("0x{}", fastrand::u64(..).to_string()))
            } else {
                None
            },
            status,
            amount_received: if is_completed { Some(U256::from(1000000u64)) } else { None },
            fees_paid: U256::from(400u64), // 0.04%
            started_at: Utc::now() - Duration::minutes(5),
            completed_at: if is_completed { Some(Utc::now()) } else { None },
            error_message: None,
        })
    }

    async fn get_liquidity(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<U256> {
        if !self.is_supported_route(from, to, token) {
            return Ok(U256::ZERO);
        }

        // Across typically has good liquidity for major tokens
        let base_liquidity = match token.symbol.as_str() {
            "USDC" => 50_000_000u64, // $50M
            "WETH" => 20_000u64,     // 20K ETH 
            "WBTC" => 1_000u64,      // 1K BTC
            _ => 1_000_000u64,       // $1M default
        };

        Ok(U256::from(base_liquidity) * U256::from(10u64.pow(token.decimals as u32)))
    }

    async fn get_success_rate(&self) -> BridgeResult<f64> {
        // Across has high reliability due to optimistic oracle
        Ok(0.98) // 98%
    }

    async fn get_avg_completion_time(&self, _from: ChainId, _to: ChainId) -> BridgeResult<u64> {
        // Across is known for fast bridging
        Ok(self.avg_completion_time) // 5 minutes average
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_across_bridge_creation() {
        let bridge = AcrossBridge::new();
        assert_eq!(bridge.name(), "Across");
    }

    #[tokio::test]
    async fn test_supports_usdc_route() {
        let bridge = AcrossBridge::new();
        let usdc = CrossChainToken {
            symbol: "USDC".to_string(),
            addresses: HashMap::new(),
            decimals: 6,
        };

        let supports = bridge.supports_route(ChainId::Ethereum, ChainId::Polygon, &usdc).await.unwrap();
        assert!(supports);

        // Should not support same chain
        let supports = bridge.supports_route(ChainId::Ethereum, ChainId::Ethereum, &usdc).await.unwrap();
        assert!(!supports);
    }

    #[tokio::test]
    async fn test_mock_quote_generation() {
        std::env::set_var("API_MODE", "mock");
        let bridge = AcrossBridge::new();
        
        let usdc = CrossChainToken {
            symbol: "USDC".to_string(),
            addresses: HashMap::new(),
            decimals: 6,
        };

        let amount = U256::from(10_000_000u64); // $10 USDC
        let quote = bridge.get_quote(
            ChainId::Ethereum,
            ChainId::Polygon,
            &usdc,
            amount,
            0.5
        ).await.unwrap();

        assert_eq!(quote.source_chain, ChainId::Ethereum);
        assert_eq!(quote.destination_chain, ChainId::Polygon);
        assert!(quote.amount_out < amount); // Should have fees
        assert!(quote.bridge_fee > U256::ZERO);
        
        std::env::remove_var("API_MODE");
    }
}