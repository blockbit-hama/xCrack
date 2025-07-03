use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rust_decimal::Decimal;
use ethers::types::{H160, H256, U256};
use chrono::{DateTime, Utc};

/// Transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: H256,
    pub from: H160,
    pub to: Option<H160>,
    pub value: U256,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub data: Vec<u8>,
    pub nonce: u64,
    pub timestamp: DateTime<Utc>,
    pub block_number: Option<u64>,
}

/// Strategy types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StrategyType {
    Arbitrage,
    Sandwich,
    Liquidation,
    // TODO: 향후 구현 예정
    // Frontrun,
    // Backrun,
}

/// Opportunity types  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityType {
    Arbitrage,
    Sandwich,
    Liquidation,
    MevBoost,
}

/// MEV Opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: String,
    pub opportunity_type: OpportunityType,
    pub strategy: StrategyType,
    pub expected_profit: U256,
    pub confidence: f64, // 0.0 to 1.0
    pub gas_estimate: u64,
    pub priority: Priority,
    pub timestamp: DateTime<Utc>,
    pub expiry_block: u64,
    pub details: OpportunityDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityDetails {
    Arbitrage(ArbitrageDetails),
    Sandwich(SandwichDetails),
    Liquidation(LiquidationDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageDetails {
    pub token_in: H160,
    pub token_out: H160,
    pub amount_in: U256,
    pub amount_out: U256,
    pub dex_path: Vec<String>,
    pub price_impact: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandwichDetails {
    pub victim_transaction: Transaction,
    pub frontrun_amount: U256,
    pub backrun_amount: U256,
    pub target_slippage: f64,
    pub pool_address: H160,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationDetails {
    pub protocol: String,
    pub user: H160,
    pub collateral_asset: H160,
    pub debt_asset: H160,
    pub collateral_amount: U256,
    pub debt_amount: U256,
    pub health_factor: Decimal,
}

/// Bundle representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: String,
    pub transactions: Vec<Transaction>,
    pub target_block: u64,
    pub expected_profit: U256,
    pub gas_estimate: u64,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub priority: Priority,
    pub strategy: StrategyType,
    pub hash: Option<H256>,
    pub timestamp: DateTime<Utc>,
    pub expiry_time: DateTime<Utc>,
}

/// Bundle status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BundleStatus {
    Created,
    Queued,
    Submitted,
    Pending,
    Included,
    Failed,
    Expired,
}

/// Bundle result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResult {
    pub bundle_id: String,
    pub bundle_hash: H256,
    pub status: BundleStatus,
    pub block_number: Option<u64>,
    pub actual_profit: Option<U256>,
    pub gas_used: Option<u64>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Pool state for AMM calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub pair_address: H160,
    pub token0: H160,
    pub token1: H160,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee: u32, // Fee in basis points (300 = 0.3%)
    pub dex: String,
    pub block_number: u64,
    pub timestamp: DateTime<Utc>,
}

/// Token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: H160,
    pub symbol: String,
    pub decimals: u8,
    pub name: String,
}

/// Price information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceInfo {
    pub token: H160,
    pub price: Decimal,
    pub dex: String,
    pub timestamp: DateTime<Utc>,
    pub liquidity: U256,
}

/// Simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub success: bool,
    pub profit: U256,
    pub gas_used: u64,
    pub gas_cost: U256,
    pub net_profit: U256,
    pub price_impact: f64,
    pub error_message: Option<String>,
    pub traces: Option<Vec<String>>,
}

/// Gas estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEstimate {
    pub gas_limit: u64,
    pub gas_price: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub total_cost: U256,
}

/// Fee data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeData {
    pub gas_price: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub last_base_fee_per_gas: U256,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub transactions_processed: u64,
    pub opportunities_found: u64,
    pub bundles_submitted: u64,
    pub bundles_included: u64,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub avg_analysis_time: f64,
    pub avg_submission_time: f64,
    pub success_rate: f64,
    pub uptime: u64,
}

/// Alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub alert_type: AlertType,
    pub title: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    Profit,
    Error,
    Warning,
    Emergency,
}

/// Profit report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitReport {
    pub date: String,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub net_profit: U256,
    pub bundles_submitted: u64,
    pub bundles_included: u64,
    pub success_rate: f64,
    pub strategies: HashMap<StrategyType, StrategyStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub profit: U256,
    pub count: u64,
    pub success_rate: f64,
}

/// Error types
#[derive(thiserror::Error, Debug)]
pub enum MevError {
    #[error("Strategy error: {message}")]
    Strategy { message: String, strategy: StrategyType },

    #[error("Bundle error: {message}")]
    Bundle { message: String, bundle_id: String },

    #[error("Simulation error: {0}")]
    Simulation(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),
}

/// Result type alias
pub type MevResult<T> = Result<T, MevError>;

/// Utility functions
impl Priority {
    pub fn to_u8(&self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Medium => 1,
            Priority::High => 2,
            Priority::Urgent => 3,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Priority::Low,
            1 => Priority::Medium,
            2 => Priority::High,
            3 => Priority::Urgent,
            _ => Priority::Medium,
        }
    }
}

impl Opportunity {
    pub fn new(
        opportunity_type: OpportunityType,
        strategy: StrategyType,
        expected_profit: U256,
        confidence: f64,
        gas_estimate: u64,
        expiry_block: u64,
        details: OpportunityDetails,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let priority = if expected_profit > U256::from(100_000_000_000_000_000u64) {
            Priority::High
        } else if expected_profit > U256::from(50_000_000_000_000_000u64) {
            Priority::Medium
        } else {
            Priority::Low
        };

        Self {
            id,
            opportunity_type,
            strategy,
            expected_profit,
            confidence,
            gas_estimate,
            priority,
            timestamp: Utc::now(),
            expiry_block,
            details,
        }
    }

    pub fn is_expired(&self, current_block: u64) -> bool {
        current_block >= self.expiry_block
    }

    pub fn profit_per_gas(&self) -> f64 {
        if self.gas_estimate == 0 {
            return 0.0;
        }
        self.expected_profit.as_u128() as f64 / self.gas_estimate as f64
    }
}

impl Bundle {
    pub fn new(
        transactions: Vec<Transaction>,
        target_block: u64,
        expected_profit: U256,
        gas_estimate: u64,
        strategy: StrategyType,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let priority = if expected_profit > U256::from(100_000_000_000_000_000u64) {
            Priority::High
        } else if expected_profit > U256::from(50_000_000_000_000_000u64) {
            Priority::Medium
        } else {
            Priority::Low
        };

        let now = Utc::now();
        let expiry_time = now + chrono::Duration::seconds(crate::constants::MAX_BUNDLE_LIFETIME as i64);

        Self {
            id,
            transactions,
            target_block,
            expected_profit,
            gas_estimate,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            priority,
            strategy,
            hash: None,
            timestamp: now,
            expiry_time,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expiry_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ethers::types::{H160, H256, U256};
    use std::str::FromStr;

    #[test]
    fn test_priority_conversion() {
        // Test to_u8
        assert_eq!(Priority::Low.to_u8(), 0);
        assert_eq!(Priority::Medium.to_u8(), 1);
        assert_eq!(Priority::High.to_u8(), 2);
        assert_eq!(Priority::Urgent.to_u8(), 3);
        
        // Test from_u8
        assert!(matches!(Priority::from_u8(0), Priority::Low));
        assert!(matches!(Priority::from_u8(1), Priority::Medium));
        assert!(matches!(Priority::from_u8(2), Priority::High));
        assert!(matches!(Priority::from_u8(3), Priority::Urgent));
        
        // Test invalid value (should default to Medium)
        assert!(matches!(Priority::from_u8(99), Priority::Medium));
    }

    #[test]
    fn test_opportunity_creation() {
        let details = OpportunityDetails::Arbitrage(ArbitrageDetails {
            token_in: H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            token_out: H160::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
            amount_in: U256::from(1000000000000000000u64), // 1 ETH
            amount_out: U256::from(2000000000u64), // 2000 USDC
            dex_path: vec!["uniswap_v2".to_string(), "sushiswap".to_string()],
            price_impact: 0.01,
        });
        
        let opportunity = Opportunity::new(
            OpportunityType::Arbitrage,
            StrategyType::Arbitrage,
            U256::from(50000000000000000u64), // 0.05 ETH profit
            0.8,
            200_000,
            1000,
            details,
        );
        
        // Test basic fields
        assert!(!opportunity.id.is_empty());
        assert!(matches!(opportunity.opportunity_type, OpportunityType::Arbitrage));
        assert!(matches!(opportunity.strategy, StrategyType::Arbitrage));
        assert_eq!(opportunity.expected_profit, U256::from(50000000000000000u64));
        assert_eq!(opportunity.confidence, 0.8);
        assert_eq!(opportunity.gas_estimate, 200_000);
        assert_eq!(opportunity.expiry_block, 1000);
        assert!(matches!(opportunity.priority, Priority::Medium));
        
        // Test expiry
        assert!(!opportunity.is_expired(999));
        assert!(opportunity.is_expired(1000));
        assert!(opportunity.is_expired(1001));
        
        // Test profit per gas calculation
        let profit_per_gas = opportunity.profit_per_gas();
        assert!(profit_per_gas > 0.0);
        assert_eq!(profit_per_gas, 50000000000000000.0 / 200_000.0);
    }

    #[test]
    fn test_opportunity_priority_calculation() {
        // High priority opportunity (> 0.1 ETH profit)
        let high_profit_opp = Opportunity::new(
            OpportunityType::Arbitrage,
            StrategyType::Arbitrage,
            U256::from(200000000000000000u64), // 0.2 ETH profit
            0.9,
            300_000,
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: H160::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(2000000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        assert!(matches!(high_profit_opp.priority, Priority::High));
        
        // Medium priority opportunity (0.05-0.1 ETH profit)
        let medium_profit_opp = Opportunity::new(
            OpportunityType::Arbitrage,
            StrategyType::Arbitrage,
            U256::from(80000000000000000u64), // 0.08 ETH profit
            0.8,
            250_000,
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: H160::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(1800000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        assert!(matches!(medium_profit_opp.priority, Priority::Medium));
        
        // Low priority opportunity (< 0.05 ETH profit)
        let low_profit_opp = Opportunity::new(
            OpportunityType::Arbitrage,
            StrategyType::Arbitrage,
            U256::from(10000000000000000u64), // 0.01 ETH profit
            0.7,
            200_000,
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: H160::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(1100000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        assert!(matches!(low_profit_opp.priority, Priority::Low));
    }

    #[test]
    fn test_bundle_creation() {
        let transactions = vec![
            Transaction {
                hash: H256::from_str("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").unwrap(),
                from: H160::from_str("0x742d35Cc6570000000000000000000000000004").unwrap(),
                to: Some(H160::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()),
                value: U256::from(1000000000000000000u64), // 1 ETH
                gas_price: U256::from(20000000000u64), // 20 gwei
                gas_limit: U256::from(200000u64),
                data: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
                nonce: 1,
                timestamp: Utc::now(),
                block_number: None,
            }
        ];
        
        let bundle = Bundle::new(
            transactions.clone(),
            1000,
            U256::from(50000000000000000u64), // 0.05 ETH
            200_000,
            StrategyType::Arbitrage,
        );
        
        // Test basic fields
        assert!(!bundle.id.is_empty());
        assert_eq!(bundle.transactions.len(), 1);
        assert_eq!(bundle.target_block, 1000);
        assert_eq!(bundle.expected_profit, U256::from(50000000000000000u64));
        assert_eq!(bundle.gas_estimate, 200_000);
        assert!(matches!(bundle.strategy, StrategyType::Arbitrage));
        assert!(matches!(bundle.priority, Priority::Medium)); // 0.05 ETH 
        assert!(bundle.hash.is_none());
        assert!(bundle.max_fee_per_gas.is_none());
        assert!(bundle.max_priority_fee_per_gas.is_none());
        
        // Test expiry
        assert!(!bundle.is_expired());
        
        // Test with high priority
        let high_priority_bundle = Bundle::new(
            transactions,
            1000,
            U256::from(200000000000000000u64), // 0.2 ETH
            300_000,
            StrategyType::Sandwich,
        );
        assert!(matches!(high_priority_bundle.priority, Priority::High));
    }

    #[test]
    fn test_constants() {
        // Test token addresses
        assert_eq!(crate::constants::get_token_address("WETH").unwrap(), H160::from_str(crate::constants::WETH).unwrap());
        assert_eq!(crate::constants::get_token_address("USDC").unwrap(), H160::from_str(crate::constants::USDC).unwrap());
        assert_eq!(crate::constants::get_token_address("USDT").unwrap(), H160::from_str(crate::constants::USDT).unwrap());
        assert_eq!(crate::constants::get_token_address("DAI").unwrap(), H160::from_str(crate::constants::DAI).unwrap());
        assert_eq!(crate::constants::get_token_address("WBTC").unwrap(), H160::from_str(crate::constants::WBTC).unwrap());
        
        // Test case insensitivity
        assert_eq!(crate::constants::get_token_address("weth").unwrap(), H160::from_str(crate::constants::WETH).unwrap());
        assert_eq!(crate::constants::get_token_address("usdc").unwrap(), H160::from_str(crate::constants::USDC).unwrap());
        
        // Test non-existent token
        assert!(crate::constants::get_token_address("NONEXISTENT").is_none());
        
        // Test constants values
        assert_eq!(crate::constants::DEFAULT_GAS_LIMIT, 300_000);
        assert_eq!(crate::constants::MAX_GAS_LIMIT, 30_000_000);
        assert_eq!(crate::constants::BLOCK_TIME, 12);
        assert_eq!(crate::constants::MAX_BUNDLE_LIFETIME, 300);
        assert_eq!(crate::constants::MIN_PROFIT_WEI, 10_000_000_000_000_000);
        assert_eq!(crate::constants::MIN_PROFIT_RATIO, 0.01);
        assert_eq!(crate::constants::MAX_GAS_PRICE_GWEI, 500);
        assert_eq!(crate::constants::MAX_PRIORITY_FEE_GWEI, 50);
    }

    #[test]
    fn test_profit_per_gas_edge_cases() {
        // Test zero gas estimate
        let opportunity = Opportunity::new(
            OpportunityType::Arbitrage,
            StrategyType::Arbitrage,
            U256::from(1000000000000000000u64),
            0.8,
            0, // Zero gas
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: H160::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(2000000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        
        assert_eq!(opportunity.profit_per_gas(), 0.0);
    }
}