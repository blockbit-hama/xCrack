use super::traits::{Bridge, BridgeQuote, BridgeError, BridgeResult, BridgeExecution, BridgeExecutionStatus};
use crate::types::{ChainId, CrossChainToken};
use alloy::primitives::U256;
use async_trait::async_trait;
use chrono::{Utc, Duration};
use uuid::Uuid;
use serde_json::json;

/// Hop Protocol bridge implementation (Mock)
#[derive(Debug)]
pub struct HopBridge {
    /// Mock success rate
    success_rate: f64,
    
    /// Mock liquidity per pool
    mock_liquidity: U256,
}

impl HopBridge {
    pub fn new() -> Self {
        Self {
            success_rate: 0.96, // 96% success rate
            mock_liquidity: U256::from(5_000_000u64) * U256::from(10u64.pow(18)), // 5M tokens liquidity
        }
    }
    
    /// Check if route is supported by Hop
    fn is_supported_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> bool {
        // Hop supports ETH, USDC, USDT, DAI
        let supported_tokens = ["ETH", "WETH", "USDC", "USDT", "DAI"];
        let supported_chains = [ChainId::Ethereum, ChainId::Polygon, ChainId::Arbitrum, ChainId::Optimism];
        
        supported_tokens.contains(&token.symbol.as_str()) &&
        supported_chains.contains(&from) &&
        supported_chains.contains(&to) &&
        from != to
    }
}

impl Default for HopBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Bridge for HopBridge {
    fn name(&self) -> &'static str {
        "Hop"
    }
    
    async fn supports_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<bool> {
        Ok(self.is_supported_route(from, to, token))
    }
    
    async fn get_routes(&self, token: &CrossChainToken) -> BridgeResult<Vec<(ChainId, ChainId)>> {
        if !["ETH", "WETH", "USDC", "USDT", "DAI"].contains(&token.symbol.as_str()) {
            return Ok(vec![]);
        }
        
        let supported_chains = [ChainId::Ethereum, ChainId::Polygon, ChainId::Arbitrum, ChainId::Optimism];
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
        if !self.is_supported_route(from, to, token) {
            return Err(BridgeError::UnsupportedRoute { from, to });
        }
        
        // Check liquidity
        if amount > self.mock_liquidity {
            return Err(BridgeError::InsufficientLiquidity {
                available: self.mock_liquidity,
                required: amount,
            });
        }
        
        // Hop has slightly higher fees but faster for ETH
        let bridge_fee = if token.symbol == "ETH" || token.symbol == "WETH" {
            amount * U256::from(4) / U256::from(10000) // 0.04% for ETH
        } else {
            amount * U256::from(8) / U256::from(10000) // 0.08% for stablecoins
        };
        
        let gas_fee = match from {
            ChainId::Ethereum => U256::from(80_000_000_000_000_000u64), // ~0.08 ETH
            _ => U256::from(15_000_000_000_000_000u64), // ~0.015 ETH on L2s
        };
        
        let protocol_fee = U256::ZERO;
        let price_impact = 0.05; // 0.05% price impact
        
        // Apply slippage
        let slippage_amount = amount * U256::from((slippage * 100.0) as u64) / U256::from(10000);
        let amount_out = amount - bridge_fee - slippage_amount;
        
        let estimated_time = match (from, to) {
            (ChainId::Ethereum, _) | (_, ChainId::Ethereum) => 1800, // 30 minutes for mainnet
            _ => 600, // 10 minutes for L2 to L2
        };
        
        let quote = BridgeQuote {
            quote_id: Uuid::new_v4().to_string(),
            source_chain: from,
            destination_chain: to,
            token: token.clone(),
            amount_in: amount,
            amount_out,
            bridge_fee,
            gas_fee,
            protocol_fee,
            exchange_rate: 1.0,
            price_impact,
            estimated_time,
            expires_at: Utc::now() + Duration::minutes(3),
            route_data: json!({
                "bridge": "hop",
                "route_type": if from == ChainId::Ethereum || to == ChainId::Ethereum {
                    "canonical"
                } else {
                    "fast"
                },
                "bonder_fee": bridge_fee.to_string(),
                "amm": format!("Hop_{}_{}", from.name(), token.symbol)
            }),
            slippage_tolerance: slippage,
        };
        
        Ok(quote)
    }
    
    async fn execute_bridge(&self, quote: &BridgeQuote) -> BridgeResult<BridgeExecution> {
        if !quote.is_valid() {
            return Err(BridgeError::QuoteExpired);
        }
        
        // Mock execution with slightly lower success rate
        let success = fastrand::f64() < self.success_rate;
        
        let execution = BridgeExecution {
            execution_id: Uuid::new_v4().to_string(),
            source_tx_hash: format!("0x{}", Uuid::new_v4().to_string().replace('-', "")),
            destination_tx_hash: if success { 
                Some(format!("0x{}", Uuid::new_v4().to_string().replace('-', ""))) 
            } else { 
                None 
            },
            status: if success { BridgeExecutionStatus::Completed } else { BridgeExecutionStatus::Failed },
            amount_received: if success { Some(quote.amount_out) } else { None },
            fees_paid: quote.total_cost(),
            started_at: Utc::now(),
            completed_at: if success { 
                Some(Utc::now() + Duration::seconds(quote.estimated_time as i64)) 
            } else { 
                None 
            },
            error_message: if !success { Some("Mock bonder liquidity shortage".to_string()) } else { None },
        };
        
        Ok(execution)
    }
    
    async fn get_execution_status(&self, execution_id: &str) -> BridgeResult<BridgeExecution> {
        // Mock status lookup
        Ok(BridgeExecution {
            execution_id: execution_id.to_string(),
            source_tx_hash: format!("0x{}", Uuid::new_v4().to_string().replace('-', "")),
            destination_tx_hash: Some(format!("0x{}", Uuid::new_v4().to_string().replace('-', ""))),
            status: BridgeExecutionStatus::Completed,
            amount_received: Some(U256::from(5_000_000_000_000_000_000u64)), // 5 ETH
            fees_paid: U256::from(80_000_000_000_000_000u64), // 0.08 ETH
            started_at: Utc::now() - Duration::minutes(15),
            completed_at: Some(Utc::now() - Duration::minutes(5)),
            error_message: None,
        })
    }
    
    async fn get_liquidity(&self, _from: ChainId, _to: ChainId, _token: &CrossChainToken) -> BridgeResult<U256> {
        Ok(self.mock_liquidity)
    }
    
    async fn get_success_rate(&self) -> BridgeResult<f64> {
        Ok(self.success_rate)
    }
    
    async fn get_avg_completion_time(&self, from: ChainId, to: ChainId) -> BridgeResult<u64> {
        match (from, to) {
            (ChainId::Ethereum, _) | (_, ChainId::Ethereum) => Ok(1800), // 30 minutes for mainnet
            _ => Ok(600), // 10 minutes for L2 to L2
        }
    }
}