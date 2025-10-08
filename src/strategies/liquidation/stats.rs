use std::time::Instant;
use ethers::types::U256;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OnChainLiquidationStats {
    pub protocols_monitored: u64,
    pub users_monitored: u64,
    pub transactions_analyzed: u64,
    pub opportunities_found: u64,
    pub successful_liquidations: u64,
    pub total_profit: U256,
    pub avg_profit_per_liquidation: U256,
    pub avg_gas_used: U256,
    pub last_scan_time: Option<Instant>,
}

impl Default for OnChainLiquidationStats {
    fn default() -> Self {
        Self {
            protocols_monitored: 0,
            users_monitored: 0,
            transactions_analyzed: 0,
            opportunities_found: 0,
            successful_liquidations: 0,
            total_profit: U256::zero(),
            avg_profit_per_liquidation: U256::zero(),
            avg_gas_used: U256::zero(),
            last_scan_time: None,
        }
    }
}
