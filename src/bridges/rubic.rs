use super::traits::{Bridge, BridgeQuote, BridgeError, BridgeResult, BridgeExecution, BridgeExecutionStatus};
use crate::types::{ChainId, CrossChainToken};
use alloy::primitives::U256;
use async_trait::async_trait;
use chrono::{Utc, Duration};
use uuid::Uuid;
use serde_json::json;

/// Rubic bridge implementation (Mock)
#[derive(Debug)]
pub struct RubicBridge {
    /// Mock success rate
    success_rate: f64,
    
    /// Mock liquidity per pool
    mock_liquidity: U256,
}

impl RubicBridge {
    pub fn new() -> Self {
        Self {
            success_rate: 0.94, // 94% success rate (aggregator has variability)
            mock_liquidity: U256::from(8_000_000u64) * U256::from(10u64.pow(18)), // 8M tokens liquidity
        }
    }
    
    /// Check if route is supported by Rubic (supports most routes via aggregation)
    fn is_supported_route(&self, from: ChainId, to: ChainId, _token: &CrossChainToken) -> bool {
        // Rubic supports most chains and tokens via aggregation
        let supported_chains = [
            ChainId::Ethereum, 
            ChainId::Polygon, 
            ChainId::BSC, 
            ChainId::Arbitrum, 
            ChainId::Optimism, 
            ChainId::Avalanche
        ];
        
        supported_chains.contains(&from) &&
        supported_chains.contains(&to) &&
        from != to
    }
}

impl Default for RubicBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Bridge for RubicBridge {
    fn name(&self) -> &'static str {
        "Rubic"
    }
    
    async fn supports_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<bool> {
        Ok(self.is_supported_route(from, to, token))
    }
    
    async fn get_routes(&self, _token: &CrossChainToken) -> BridgeResult<Vec<(ChainId, ChainId)>> {
        // Rubic supports most routes via aggregation
        let supported_chains = [
            ChainId::Ethereum, 
            ChainId::Polygon, 
            ChainId::BSC, 
            ChainId::Arbitrum, 
            ChainId::Optimism, 
            ChainId::Avalanche
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
        if !self.is_supported_route(from, to, token) {
            return Err(BridgeError::UnsupportedRoute { from, to });
        }
        
        // Check liquidity (more flexible for aggregator)
        if amount > self.mock_liquidity {
            return Err(BridgeError::InsufficientLiquidity {
                available: self.mock_liquidity,
                required: amount,
            });
        }
        
        // Rubic fees vary based on underlying bridge used
        let bridge_fee = amount * U256::from(15) / U256::from(10000); // 0.15% average fee
        let gas_fee = match from {
            ChainId::Ethereum => U256::from(120_000_000_000_000_000u64), // ~0.12 ETH (higher for aggregation)
            ChainId::BSC => U256::from(5_000_000_000_000_000u64), // ~0.005 BNB
            _ => U256::from(20_000_000_000_000_000u64), // ~0.02 ETH on other chains
        };
        
        let protocol_fee = amount * U256::from(5) / U256::from(10000); // 0.05% protocol fee
        let price_impact = 0.08; // 0.08% price impact (varies by route)
        
        // Apply slippage
        let slippage_amount = amount * U256::from((slippage * 100.0) as u64) / U256::from(10000);
        let amount_out = amount - bridge_fee - slippage_amount;
        
        // Rubic completion time varies by underlying bridge
        let estimated_time = match (from, to) {
            // Cross-chain routes through different protocols
            (ChainId::Ethereum, ChainId::BSC) | (ChainId::BSC, ChainId::Ethereum) => 1200, // 20 min
            (ChainId::Ethereum, _) | (_, ChainId::Ethereum) => 900, // 15 min
            _ => 420, // 7 minutes for other routes
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
            exchange_rate: 0.998, // Slightly worse rate due to aggregation
            price_impact,
            estimated_time,
            expires_at: Utc::now() + Duration::minutes(2), // Shorter expiry for aggregator
            route_data: json!({
                "bridge": "rubic",
                "underlying_bridges": ["stargate", "multichain", "celer"],
                "best_route": "stargate", // Mock best route selection
                "rubic_fee": protocol_fee.to_string(),
                "cross_chain_id": format!("{}-{}", from.name().to_lowercase(), to.name().to_lowercase())
            }),
            slippage_tolerance: slippage,
        };
        
        Ok(quote)
    }
    
    async fn execute_bridge(&self, quote: &BridgeQuote) -> BridgeResult<BridgeExecution> {
        if !quote.is_valid() {
            return Err(BridgeError::QuoteExpired);
        }
        
        // Mock execution with variable success rate
        let success = fastrand::f64() < self.success_rate;
        
        let status = if success {
            BridgeExecutionStatus::Completed
        } else {
            // Rubic might require action due to aggregation complexity
            if fastrand::f64() < 0.3 {
                BridgeExecutionStatus::RequiresAction
            } else {
                BridgeExecutionStatus::Failed
            }
        };
        
        let execution = BridgeExecution {
            execution_id: Uuid::new_v4().to_string(),
            source_tx_hash: format!("0x{}", Uuid::new_v4().to_string().replace('-', "")),
            destination_tx_hash: if success { 
                Some(format!("0x{}", Uuid::new_v4().to_string().replace('-', ""))) 
            } else { 
                None 
            },
            status,
            amount_received: if success { Some(quote.amount_out) } else { None },
            fees_paid: quote.total_cost(),
            started_at: Utc::now(),
            completed_at: if success { 
                Some(Utc::now() + Duration::seconds(quote.estimated_time as i64)) 
            } else { 
                None 
            },
            error_message: match success {
                true => None,
                false => Some("Mock aggregation route failure".to_string()),
            },
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
            amount_received: Some(U256::from(998_000u64)), // Slightly less due to aggregation
            fees_paid: U256::from(20_000u64), // Higher fees
            started_at: Utc::now() - Duration::minutes(20),
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
            (ChainId::Ethereum, ChainId::BSC) | (ChainId::BSC, ChainId::Ethereum) => Ok(1200), // 20 min
            (ChainId::Ethereum, _) | (_, ChainId::Ethereum) => Ok(900), // 15 min
            _ => Ok(420), // 7 minutes for other routes
        }
    }
}