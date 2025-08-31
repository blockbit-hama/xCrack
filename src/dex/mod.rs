pub mod aggregator;
pub mod ox_api;
pub mod oneinch_api;
pub mod uniswap;

pub use aggregator::*;
pub use ox_api::*;
pub use oneinch_api::*;
pub use uniswap::*;

use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum DexType {
    ZeroX,
    OneInch,
    UniswapV2,
    UniswapV3,
    SushiSwap,
    Curve,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapQuote {
    pub aggregator: DexType,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub buy_amount_min: U256,
    pub router_address: Address,
    pub calldata: Vec<u8>,
    pub allowance_target: Address,
    pub gas_estimate: u64,
    pub gas_price: U256,
    pub price_impact: f64,
    pub sources: Vec<SwapSource>,
    pub estimated_execution_time_ms: u64,
    pub quote_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapSource {
    pub name: String,
    pub proportion: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapParams {
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: U256,
    pub slippage_tolerance: f64, // e.g., 0.01 for 1%
    pub recipient: Option<Address>,
    pub deadline_seconds: Option<u64>,
    pub exclude_sources: Vec<String>,
    pub include_sources: Vec<String>,
    pub fee_recipient: Option<Address>,
    pub buy_token_percentage_fee: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SwapResult {
    pub tx_hash: String,
    pub buy_amount_actual: U256,
    pub gas_used: u64,
    pub success: bool,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuoteComparison {
    pub best_quote: SwapQuote,
    pub all_quotes: Vec<SwapQuote>,
    pub price_difference_percent: f64,
    pub gas_difference: i64,
    pub recommendation_reason: String,
}

// Object-safe trait for dynamic dispatch
#[async_trait]
pub trait DexAggregator: Send + Sync {
    async fn get_quote(&self, params: SwapParams) -> anyhow::Result<SwapQuote>;
    async fn get_price(&self, sell_token: Address, buy_token: Address) -> anyhow::Result<f64>;
    async fn get_liquidity(&self, token: Address) -> anyhow::Result<U256>;
    fn aggregator_type(&self) -> DexType;
    fn is_available(&self) -> bool;
    fn supported_networks(&self) -> Vec<u64>; // Chain IDs
}

