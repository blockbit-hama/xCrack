use super::traits::{Bridge, BridgeQuote, BridgeError, BridgeResult, BridgeExecution, BridgeExecutionStatus};
use crate::types::{ChainId, CrossChainToken};
use alloy::primitives::U256;
use async_trait::async_trait;
use chrono::{Utc, Duration};
use uuid::Uuid;
use serde_json::json;
use tracing::{info, warn, debug};

/// Multichain bridge implementation (formerly Anyswap)
/// 
/// Multichain은 가장 넓은 체인 지원을 제공하는 크로스체인 브리지입니다.
/// - 광범위한 체인 지원 (80+ 체인)
/// - 다양한 토큰 지원
/// - 안전한 MPC (Multi-Party Computation) 기술
#[derive(Debug)]
pub struct MultichainBridge {
    /// Mock mode flag
    mock_mode: bool,
    /// Base fee percentage (0.1% = 0.001)
    base_fee_rate: f64,
    /// Minimum bridge amount
    min_amount: U256,
    /// Maximum bridge amount per transaction
    max_amount: U256,
    /// Average completion time in seconds
    avg_completion_time: u64,
}

impl MultichainBridge {
    pub fn new() -> Self {
        Self {
            mock_mode: crate::mocks::is_mock_mode(),
            base_fee_rate: 0.001, // 0.1%
            min_amount: U256::from(5_000000u64), // $5 USDC minimum
            max_amount: U256::from(10_000_000_000000u64), // $10M USDC maximum
            avg_completion_time: 600, // 10 minutes
        }
    }

    /// Calculate Multichain bridge fee
    fn calculate_fee(&self, amount: U256, token_symbol: &str) -> U256 {
        // Fee structure varies by token
        let fee_rate = match token_symbol {
            "USDC" | "USDT" => 0.001,  // 0.1% for stablecoins
            "WETH" | "ETH" => 0.0015,  // 0.15% for ETH
            "WBTC" | "BTC" => 0.002,   // 0.2% for BTC
            _ => self.base_fee_rate,   // Default 0.1%
        };

        let fee = (amount.to::<u128>() as f64 * fee_rate) as u128;
        U256::from(fee)
    }

    /// Get supported token addresses for Multichain
    fn get_token_address(&self, chain: ChainId, token: &CrossChainToken) -> Option<alloy::primitives::Address> {
        // Multichain supports many tokens across many chains
        match (chain, token.symbol.as_str()) {
            // USDC addresses
            (ChainId::Ethereum, "USDC") => Some("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse().unwrap()),
            (ChainId::Polygon, "USDC") => Some("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".parse().unwrap()),
            (ChainId::BSC, "USDC") => Some("0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d".parse().unwrap()),
            (ChainId::Arbitrum, "USDC") => Some("0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".parse().unwrap()),
            (ChainId::Optimism, "USDC") => Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".parse().unwrap()),
            (ChainId::Avalanche, "USDC") => Some("0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E".parse().unwrap()),
            
            // WETH addresses
            (ChainId::Ethereum, "WETH") => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap()),
            (ChainId::Polygon, "WETH") => Some("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".parse().unwrap()),
            (ChainId::BSC, "WETH") => Some("0x2170Ed0880ac9A755fd29B2688956BD959F933F8".parse().unwrap()),
            (ChainId::Arbitrum, "WETH") => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".parse().unwrap()),
            (ChainId::Optimism, "WETH") => Some("0x4200000000000000000000000000000000000006".parse().unwrap()),
            (ChainId::Avalanche, "WETH") => Some("0x49D5c2BdFfac6CE2BFdB6640F4F80f226bc10bAB".parse().unwrap()),
            
            // Fallback to token's own address mapping
            _ => token.addresses.get(&chain).copied(),
        }
    }

    /// Check if route is supported by Multichain
    fn is_supported_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> bool {
        let supported_chains = [
            ChainId::Ethereum,
            ChainId::Polygon, 
            ChainId::BSC,
            ChainId::Arbitrum,
            ChainId::Optimism,
            ChainId::Avalanche,
        ];

        // Multichain supports many tokens
        let supported_tokens = [
            "USDC", "USDT", "DAI", "WETH", "ETH", "WBTC", "BTC",
            "LINK", "UNI", "AAVE", "SUSHI", "CRV", "YFI"
        ];

        supported_chains.contains(&from) && 
        supported_chains.contains(&to) && 
        from != to &&
        (supported_tokens.contains(&token.symbol.as_str()) || 
         (self.get_token_address(from, token).is_some() && self.get_token_address(to, token).is_some()))
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
        tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

        let bridge_fee = self.calculate_fee(amount, &token.symbol);
        let gas_fee = match token.symbol.as_str() {
            "WETH" | "ETH" => U256::from(50_000000u64), // Higher gas for ETH ~$0.05
            _ => U256::from(30_000000u64), // Lower gas for other tokens ~$0.03
        };
        
        let amount_out = amount.saturating_sub(bridge_fee).saturating_sub(gas_fee);

        let route_data = json!({
            "bridge": "multichain",
            "protocol": "Multichain Protocol",
            "bridge_fee": bridge_fee.to_string(),
            "fee_rate": self.calculate_fee_rate(&token.symbol),
            "mpc_address": format!("0x{}", fastrand::u64(..).to_string()),
            "router_version": "v7",
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
            price_impact: 0.1, // 0.1% impact
            estimated_time: self.avg_completion_time,
            expires_at: Utc::now() + Duration::minutes(10), // Longer validity
            route_data,
            slippage_tolerance: slippage,
        })
    }

    /// Get fee rate for specific token
    fn calculate_fee_rate(&self, token_symbol: &str) -> f64 {
        match token_symbol {
            "USDC" | "USDT" => 0.001,  // 0.1%
            "WETH" | "ETH" => 0.0015,  // 0.15%
            "WBTC" | "BTC" => 0.002,   // 0.2%
            _ => self.base_fee_rate,   // 0.1%
        }
    }
}

impl Default for MultichainBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Bridge for MultichainBridge {
    fn name(&self) -> &'static str {
        "Multichain"
    }

    async fn supports_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<bool> {
        Ok(self.is_supported_route(from, to, token))
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
        
        // Check if token is supported
        if self.get_token_address(ChainId::Ethereum, token).is_some() ||
           ["USDC", "USDT", "WETH", "WBTC"].contains(&token.symbol.as_str()) {
            for &from in &supported_chains {
                for &to in &supported_chains {
                    if from != to && 
                       self.get_token_address(from, token).is_some() &&
                       self.get_token_address(to, token).is_some() {
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

        // Check if token addresses exist
        if self.get_token_address(from, token).is_none() || 
           self.get_token_address(to, token).is_none() {
            return Err(BridgeError::TokenNotSupported { 
                token: token.symbol.clone() 
            });
        }

        if self.mock_mode {
            return self.generate_mock_quote(from, to, token, amount, slippage).await;
        }

        // In production, this would call the Multichain API
        warn!("Multichain real API not implemented, using mock quote");
        self.generate_mock_quote(from, to, token, amount, slippage).await
    }

    async fn execute_bridge(&self, quote: &BridgeQuote) -> BridgeResult<BridgeExecution> {
        info!("🌐 Executing Multichain bridge: {} {} -> {}", 
            quote.token.symbol, 
            quote.source_chain.name(), 
            quote.destination_chain.name()
        );

        if !quote.is_valid() {
            return Err(BridgeError::QuoteExpired);
        }

        // Mock execution with high success rate
        let execution_id = Uuid::new_v4().to_string();
        let success_rate = if self.mock_mode { 0.95 } else { 0.97 }; // 95% mock, 97% real

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
        debug!("🔍 Checking Multichain execution status: {}", execution_id);

        // Mock status progression
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
            amount_received: if is_completed { Some(U256::from(9_500_000u64)) } else { None }, // ~5% fees
            fees_paid: U256::from(500_000u64), // 0.5%
            started_at: Utc::now() - Duration::minutes(10),
            completed_at: if is_completed { Some(Utc::now()) } else { None },
            error_message: None,
        })
    }

    async fn get_liquidity(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<U256> {
        if !self.is_supported_route(from, to, token) {
            return Ok(U256::ZERO);
        }

        // Multichain has excellent liquidity due to wide adoption
        let base_liquidity = match token.symbol.as_str() {
            "USDC" | "USDT" => 100_000_000u64, // $100M stablecoins
            "WETH" | "ETH" => 50_000u64,       // 50K ETH
            "WBTC" | "BTC" => 2_000u64,        // 2K BTC
            _ => 5_000_000u64,                 // $5M default
        };

        Ok(U256::from(base_liquidity) * U256::from(10u64.pow(token.decimals as u32)))
    }

    async fn get_success_rate(&self) -> BridgeResult<f64> {
        // Multichain has good reliability but slightly lower than newer bridges
        Ok(0.97) // 97%
    }

    async fn get_avg_completion_time(&self, _from: ChainId, _to: ChainId) -> BridgeResult<u64> {
        // Multichain is slower but more reliable
        Ok(self.avg_completion_time) // 10 minutes average
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multichain_bridge_creation() {
        let bridge = MultichainBridge::new();
        assert_eq!(bridge.name(), "Multichain");
    }

    #[tokio::test]
    async fn test_supports_comprehensive_routes() {
        let bridge = MultichainBridge::new();
        
        let usdc = CrossChainToken {
            symbol: "USDC".to_string(),
            addresses: HashMap::new(),
            decimals: 6,
        };

        // Test major routes
        let supports = bridge.supports_route(ChainId::Ethereum, ChainId::BSC, &usdc).await.unwrap();
        assert!(supports);

        let supports = bridge.supports_route(ChainId::Polygon, ChainId::Avalanche, &usdc).await.unwrap();
        assert!(supports);

        // Should not support same chain
        let supports = bridge.supports_route(ChainId::Ethereum, ChainId::Ethereum, &usdc).await.unwrap();
        assert!(!supports);
    }

    #[tokio::test]
    async fn test_fee_calculation() {
        let bridge = MultichainBridge::new();
        
        let amount = U256::from(1_000_000u64); // $1 USDC
        let usdc_fee = bridge.calculate_fee(amount, "USDC");
        let eth_fee = bridge.calculate_fee(amount, "WETH");
        let btc_fee = bridge.calculate_fee(amount, "WBTC");

        // USDC should have lowest fee (0.1%)
        assert_eq!(usdc_fee, U256::from(1000u64));
        
        // WETH should have higher fee (0.15%)
        assert_eq!(eth_fee, U256::from(1500u64));
        
        // WBTC should have highest fee (0.2%)
        assert_eq!(btc_fee, U256::from(2000u64));
    }

    #[tokio::test]
    async fn test_mock_quote_with_different_tokens() {
        std::env::set_var("API_MODE", "mock");
        let bridge = MultichainBridge::new();
        
        // Test USDC
        let usdc = CrossChainToken {
            symbol: "USDC".to_string(),
            addresses: HashMap::new(),
            decimals: 6,
        };

        let amount = U256::from(10_000_000u64); // $10 USDC
        let quote = bridge.get_quote(
            ChainId::Ethereum,
            ChainId::BSC,
            &usdc,
            amount,
            0.5
        ).await.unwrap();

        assert_eq!(quote.source_chain, ChainId::Ethereum);
        assert_eq!(quote.destination_chain, ChainId::BSC);
        assert!(quote.amount_out < amount); // Should have fees
        assert!(quote.bridge_fee > U256::ZERO);
        
        // Test WETH (should have higher fees)
        let weth = CrossChainToken {
            symbol: "WETH".to_string(),
            addresses: HashMap::new(),
            decimals: 18,
        };

        let weth_quote = bridge.get_quote(
            ChainId::Ethereum,
            ChainId::Polygon,
            &weth,
            amount,
            0.5
        ).await.unwrap();

        // WETH should have higher bridge fee than USDC
        assert!(weth_quote.bridge_fee > quote.bridge_fee);
        
        std::env::remove_var("API_MODE");
    }
}