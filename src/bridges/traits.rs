use anyhow::Result;
use async_trait::async_trait;
use alloy::primitives::U256;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{ChainId, CrossChainToken};

/// Bridge operation result type
pub type BridgeResult<T> = Result<T, BridgeError>;

/// Bridge-specific errors
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Insufficient liquidity: {available} < {required}")]
    InsufficientLiquidity { available: U256, required: U256 },
    
    #[error("Unsupported route: {from} -> {to}")]
    UnsupportedRoute { from: ChainId, to: ChainId },
    
    #[error("Token not supported: {token}")]
    TokenNotSupported { token: String },
    
    #[error("Bridge temporarily unavailable")]
    BridgeUnavailable,
    
    #[error("Transaction failed: {reason}")]
    TransactionFailed { reason: String },
    
    #[error("Quote expired")]
    QuoteExpired,
    
    #[error("API error: {message}")]
    ApiError { message: String },
}

/// Bridge quote for cross-chain transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeQuote {
    /// Quote ID for tracking
    pub quote_id: String,
    
    /// Source chain
    pub source_chain: ChainId,
    
    /// Destination chain  
    pub destination_chain: ChainId,
    
    /// Token to bridge
    pub token: CrossChainToken,
    
    /// Amount to bridge (in source token units)
    pub amount_in: U256,
    
    /// Amount received on destination (in destination token units)
    pub amount_out: U256,
    
    /// Bridge fee (in source token units)
    pub bridge_fee: U256,
    
    /// Gas fee estimate (in source chain native token)
    pub gas_fee: U256,
    
    /// Protocol fee (if any)
    pub protocol_fee: U256,
    
    /// Exchange rate (destination/source)
    pub exchange_rate: f64,
    
    /// Price impact percentage
    pub price_impact: f64,
    
    /// Estimated completion time in seconds
    pub estimated_time: u64,
    
    /// Quote expiration time
    pub expires_at: DateTime<Utc>,
    
    /// Bridge-specific route data
    pub route_data: serde_json::Value,
    
    /// Slippage tolerance
    pub slippage_tolerance: f64,
}

impl BridgeQuote {
    /// Check if quote is still valid
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }
    
    /// Calculate total cost including all fees
    pub fn total_cost(&self) -> U256 {
        self.bridge_fee + self.gas_fee + self.protocol_fee
    }
    
    /// Calculate net profit (amount_out - amount_in - fees)
    pub fn net_profit(&self) -> i128 {
        let amount_out = self.amount_out.to::<u128>() as i128;
        let amount_in = self.amount_in.to::<u128>() as i128;
        let total_cost = self.total_cost().to::<u128>() as i128;
        
        amount_out - amount_in - total_cost
    }
    
    /// Check if quote is profitable
    pub fn is_profitable(&self) -> bool {
        self.net_profit() > 0
    }
}

/// Bridge execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeExecution {
    /// Execution ID
    pub execution_id: String,
    
    /// Source transaction hash
    pub source_tx_hash: String,
    
    /// Destination transaction hash (if completed)
    pub destination_tx_hash: Option<String>,
    
    /// Execution status
    pub status: BridgeExecutionStatus,
    
    /// Actual amount received
    pub amount_received: Option<U256>,
    
    /// Actual fees paid
    pub fees_paid: U256,
    
    /// Execution start time
    pub started_at: DateTime<Utc>,
    
    /// Execution completion time
    pub completed_at: Option<DateTime<Utc>>,
    
    /// Error message (if failed)
    pub error_message: Option<String>,
}

/// Bridge execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeExecutionStatus {
    /// Transaction pending on source chain
    Pending,
    
    /// Transaction confirmed on source chain, bridging in progress
    Bridging,
    
    /// Bridge transfer completed successfully
    Completed,
    
    /// Bridge transfer failed
    Failed,
    
    /// Bridge transfer requires manual intervention
    RequiresAction,
}

/// Cross-chain bridge trait
#[async_trait]
pub trait Bridge: Send + Sync + std::fmt::Debug {
    /// Get bridge protocol name
    fn name(&self) -> &'static str;
    
    /// Check if bridge supports a specific route
    async fn supports_route(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<bool>;
    
    /// Get available routes for a token
    async fn get_routes(&self, token: &CrossChainToken) -> BridgeResult<Vec<(ChainId, ChainId)>>;
    
    /// Get quote for bridging tokens
    async fn get_quote(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
        amount: U256,
        slippage: f64,
    ) -> BridgeResult<BridgeQuote>;
    
    /// Execute bridge transaction
    async fn execute_bridge(&self, quote: &BridgeQuote) -> BridgeResult<BridgeExecution>;
    
    /// Check execution status
    async fn get_execution_status(&self, execution_id: &str) -> BridgeResult<BridgeExecution>;
    
    /// Get current liquidity for a route
    async fn get_liquidity(&self, from: ChainId, to: ChainId, token: &CrossChainToken) -> BridgeResult<U256>;
    
    /// Get historical success rate
    async fn get_success_rate(&self) -> BridgeResult<f64>;
    
    /// Get average completion time for a route
    async fn get_avg_completion_time(&self, from: ChainId, to: ChainId) -> BridgeResult<u64>;
}