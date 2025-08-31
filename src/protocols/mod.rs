pub mod aave;
pub mod compound;
pub mod maker;
pub mod scanner;

pub use scanner::*;

use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ProtocolType {
    Aave,
    CompoundV2,
    CompoundV3,
    MakerDAO,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccountData {
    pub user: Address,
    pub protocol: ProtocolType,
    pub total_collateral_usd: f64,
    pub total_debt_usd: f64,
    pub available_borrows_usd: f64,
    pub current_liquidation_threshold: f64,
    pub ltv: f64,
    pub health_factor: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralPosition {
    pub asset: Address,
    pub amount: U256,
    pub usd_value: f64,
    pub liquidation_threshold: f64,
    pub price_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtPosition {
    pub asset: Address,
    pub amount: U256,
    pub usd_value: f64,
    pub borrow_rate: f64,
    pub price_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidatableUser {
    pub address: Address,
    pub protocol: ProtocolType,
    pub account_data: UserAccountData,
    pub collateral_positions: Vec<CollateralPosition>,
    pub debt_positions: Vec<DebtPosition>,
    pub max_liquidatable_debt: HashMap<Address, U256>,
    pub liquidation_bonus: HashMap<Address, f64>,
    pub priority_score: f64, // Based on debt size, health factor, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub protocol: ProtocolType,
    pub total_users: u64,
    pub liquidatable_users: u64,
    pub total_tvl_usd: f64,
    pub total_borrows_usd: f64,
    pub avg_health_factor: f64,
    pub last_scan_duration_ms: u64,
}

// Object-safe trait for dynamic dispatch
#[async_trait]
pub trait ProtocolScanner: Send + Sync {
    async fn scan_all_users(&self) -> anyhow::Result<Vec<LiquidatableUser>>;
    async fn get_user_data(&self, user: Address) -> anyhow::Result<Option<LiquidatableUser>>;
    async fn get_protocol_stats(&self) -> anyhow::Result<ProtocolStats>;
    fn protocol_type(&self) -> ProtocolType;
    fn is_healthy(&self) -> bool;
}
