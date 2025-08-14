use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rust_decimal::Decimal;
use alloy::primitives::{Address, B256, U256};
use chrono::{DateTime, Utc};

/// Transaction representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub hash: B256,
    pub from: Address,
    pub to: Option<Address>,
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
    Sandwich,
    Liquidation,
    MicroArbitrage, // 초고속 거래소간 마이크로 아비트래지
    CrossChainArbitrage, // 크로스체인 아비트래지
    // TODO: 향후 구현 예정
    // Frontrun,
    // Backrun,
}

impl std::fmt::Display for StrategyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrategyType::Sandwich => write!(f, "Sandwich"),
            StrategyType::Liquidation => write!(f, "Liquidation"),
            StrategyType::MicroArbitrage => write!(f, "MicroArbitrage"),
            StrategyType::CrossChainArbitrage => write!(f, "CrossChainArbitrage"),
        }
    }
}

/// Opportunity types  
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpportunityType {
    Sandwich,
    Liquidation,
    MicroArbitrage,
    CrossChainArbitrage,
    MevBoost,
}

/// MEV Opportunity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            opportunity_type,
            strategy,
            expected_profit,
            confidence,
            gas_estimate,
            priority: if confidence > 0.8 { Priority::High } else { Priority::Medium },
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
        self.expected_profit.to::<u128>() as f64 / self.gas_estimate as f64
    }
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpportunityDetails {
    Arbitrage(ArbitrageDetails),
    Sandwich(SandwichDetails),
    Liquidation(LiquidationDetails),
    MicroArbitrage(MicroArbitrageDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArbitrageDetails {
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub dex_path: Vec<String>,
    pub price_impact: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandwichDetails {
    pub victim_transaction: Transaction,
    pub frontrun_amount: U256,
    pub backrun_amount: U256,
    pub target_slippage: f64,
    pub pool_address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiquidationDetails {
    pub protocol: String,
    pub user: Address,
    pub collateral_asset: Address,
    pub debt_asset: Address,
    pub collateral_amount: U256,
    pub debt_amount: U256,
    pub health_factor: Decimal,
}

/// 마이크로 아비트래지 세부 정보
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicroArbitrageDetails {
    pub token_symbol: String,
    pub buy_exchange: ExchangeInfo,
    pub sell_exchange: ExchangeInfo,
    pub amount: U256,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub price_diff: Decimal,
    pub profit_percentage: f64,
    pub execution_time_ms: u64,
    pub order_books: Vec<OrderBookSnapshot>,
}

/// 거래소 정보
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExchangeInfo {
    pub name: String,
    pub exchange_type: ExchangeType,
    pub api_endpoint: String,
    pub trading_pairs: Vec<String>,
    pub fee_percentage: f64,
    pub min_order_size: U256,
    pub max_order_size: U256,
    pub latency_ms: u64,
}

/// 거래소 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExchangeType {
    DEX, // 탈중앙화 거래소
    CEX, // 중앙화 거래소
}

/// 실시간 가격 데이터
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceData {
    pub symbol: String,
    pub exchange: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last_price: Decimal,
    pub volume_24h: U256,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64, // 순서 보장을 위한 시퀀스 번호
}

/// 오더북 스냅샷
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderBookSnapshot {
    pub exchange: String,
    pub symbol: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}

/// 오더북 레벨 (가격, 수량)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub quantity: U256,
}

/// 마이크로 아비트래지 기회
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicroArbitrageOpportunity {
    pub token_symbol: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub price_spread: Decimal,
    pub profit_percentage: f64,
    pub max_amount: U256,
    pub execution_window_ms: u64,
    pub confidence_score: f64,
}

/// 주문 실행 결과
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderExecutionResult {
    pub order_id: String,
    pub exchange: String,
    pub symbol: String,
    pub side: OrderSide,
    pub amount: U256,
    pub price: Decimal,
    pub filled_amount: U256,
    pub filled_price: Decimal,
    pub status: OrderStatus,
    pub execution_time: DateTime<Utc>,
    pub latency_ms: u64,
    pub fees: U256,
}

/// 주문 방향
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// 주문 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// 마이크로 아비트래지 통계
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicroArbitrageStats {
    pub total_opportunities: u64,
    pub executed_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_volume: U256,
    pub total_profit: U256,
    pub total_fees: U256,
    pub avg_profit_per_trade: U256,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub profit_rate: f64,
    pub uptime_percentage: f64,
    pub exchanges_monitored: u32,
    pub pairs_monitored: u32,
}

/// Bundle representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub hash: Option<B256>,
    pub timestamp: DateTime<Utc>,
    pub expiry_time: DateTime<Utc>,
}

impl Bundle {
    pub fn new(
        transactions: Vec<Transaction>,
        target_block: u64,
        expected_profit: U256,
        gas_estimate: u64,
        strategy: StrategyType,
    ) -> Self {
        let timestamp = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            transactions,
            target_block,
            expected_profit,
            gas_estimate,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            priority: Priority::Medium,
            strategy,
            hash: None,
            timestamp,
            expiry_time: timestamp + chrono::Duration::minutes(5),
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expiry_time
    }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleResult {
    pub bundle_id: String,
    pub bundle_hash: B256,
    pub status: BundleStatus,
    pub block_number: Option<u64>,
    pub actual_profit: Option<U256>,
    pub gas_used: Option<u64>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Order representation for predictive strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Order {
    pub id: uuid::Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
    pub time_in_force: TimeInForce,
    pub timestamp: u64,
}

/// Order type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}

/// Time in force for orders
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TimeInForce {
    GTC, // Good Till Cancelled
    IOC, // Immediate Or Cancel
    FOK, // Fill Or Kill
    GTD, // Good Till Date
}

/// Position representation for predictive strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub id: uuid::Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub timestamp: DateTime<Utc>,
}

/// Strategy signal for MEV strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrategySignal {
    pub signal_type: String,
    pub data: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// Prediction signal from AI models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictionSignal {
    pub symbol: String,
    pub direction: f64, // -1.0 to 1.0
    pub confidence: f64, // 0.0 to 1.0
    pub time_horizon: u32, // minutes
    pub expected_move: f64, // percentage
    pub timestamp: u64,
    pub strategy_type: PredictiveStrategyType,
}

/// Predictive strategy types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PredictiveStrategyType {
    VwapExecution {
        duration_minutes: u32,
        max_participation_rate: f64,
    },
    TwapExecution {
        duration_minutes: u32,
        slice_count: u32,
    },
    IcebergExecution {
        visible_size: f64,
        total_size: f64,
    },
    MevPredictive {
        mev_threshold: f64,
        fallback_strategy: Box<PredictiveStrategyType>,
    },
}

/// Pool state for AMM calculations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolState {
    pub pair_address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee: u32, // Fee in basis points (300 = 0.3%)
    pub dex: String,
    pub block_number: u64,
    pub timestamp: DateTime<Utc>,
}

/// Token information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenInfo {
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub name: String,
}

/// Price information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceInfo {
    pub token: Address,
    pub price: Decimal,
    pub dex: String,
    pub timestamp: DateTime<Utc>,
    pub liquidity: U256,
}

/// Simulation result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasEstimate {
    pub gas_limit: u64,
    pub gas_price: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub total_cost: U256,
}

/// Fee data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeData {
    pub gas_price: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub last_base_fee_per_gas: U256,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Alert {
    pub id: String,
    pub alert_type: AlertType,
    pub title: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertType {
    Profit,
    Error,
    Warning,
    Emergency,
}

/// Profit report
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// Signal type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalType {
    Entry,
    Exit,
    Rebalance,
    StopLoss,
    TakeProfit,
}


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



#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use alloy::primitives::{Address, B256, U256};
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
            token_in: Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            token_out: Address::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
            amount_in: U256::from(1000000000000000000u64), // 1 ETH
            amount_out: U256::from(2000000000u64), // 2000 USDC
            dex_path: vec!["uniswap_v2".to_string(), "sushiswap".to_string()],
            price_impact: 0.01,
        });
        
        let opportunity = Opportunity::new(
            OpportunityType::MicroArbitrage,
            StrategyType::MicroArbitrage,
            U256::from(60000000000000000u64), // 0.06 ETH profit
            0.8,
            200_000,
            1000,
            details,
        );
        
        // Test basic fields
        assert!(!opportunity.id.is_empty());
        assert!(matches!(opportunity.opportunity_type, OpportunityType::MicroArbitrage));
        assert!(matches!(opportunity.strategy, StrategyType::MicroArbitrage));
        assert_eq!(opportunity.expected_profit, U256::from(60000000000000000u64));
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
        assert_eq!(profit_per_gas, 60000000000000000.0 / 200_000.0);
    }

    #[test]
    fn test_opportunity_priority_calculation() {
        // High priority opportunity (> 0.1 ETH profit)
        let high_profit_opp = Opportunity::new(
            OpportunityType::MicroArbitrage,
            StrategyType::MicroArbitrage,
            U256::from(200000000000000000u64), // 0.2 ETH profit
            0.9,
            300_000,
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: Address::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(2000000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        assert!(matches!(high_profit_opp.priority, Priority::High));
        
        // Medium priority opportunity (0.05-0.1 ETH profit)
        let medium_profit_opp = Opportunity::new(
            OpportunityType::MicroArbitrage,
            StrategyType::MicroArbitrage,
            U256::from(80000000000000000u64), // 0.08 ETH profit
            0.8,
            250_000,
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: Address::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(1800000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        assert!(matches!(medium_profit_opp.priority, Priority::Medium));
        
        // Low priority opportunity (< 0.05 ETH profit)
        let low_profit_opp = Opportunity::new(
            OpportunityType::MicroArbitrage,
            StrategyType::MicroArbitrage,
            U256::from(10000000000000000u64), // 0.01 ETH profit
            0.7,
            200_000,
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: Address::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
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
                hash: B256::from_str("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").unwrap(),
                from: Address::from_str("0x742d35Cc65700000000000000000000000000004").unwrap(),
                to: Some(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()),
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
            U256::from(60000000000000000u64), // 0.06 ETH
            200_000,
            StrategyType::MicroArbitrage,
        );
        
        // Test basic fields
        assert!(!bundle.id.is_empty());
        assert_eq!(bundle.transactions.len(), 1);
        assert_eq!(bundle.target_block, 1000);
        assert_eq!(bundle.expected_profit, U256::from(60000000000000000u64));
        assert_eq!(bundle.gas_estimate, 200_000);
        assert!(matches!(bundle.strategy, StrategyType::MicroArbitrage));
        assert!(matches!(bundle.priority, Priority::Medium)); // 0.06 ETH 
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
        assert_eq!(crate::constants::get_token_address("WETH").unwrap(), Address::from_str(crate::constants::WETH).unwrap());
        assert_eq!(crate::constants::get_token_address("USDC").unwrap(), Address::from_str(crate::constants::USDC).unwrap());
        assert_eq!(crate::constants::get_token_address("USDT").unwrap(), Address::from_str(crate::constants::USDT).unwrap());
        assert_eq!(crate::constants::get_token_address("DAI").unwrap(), Address::from_str(crate::constants::DAI).unwrap());
        assert_eq!(crate::constants::get_token_address("WBTC").unwrap(), Address::from_str(crate::constants::WBTC).unwrap());
        
        // Test case insensitivity
        assert_eq!(crate::constants::get_token_address("weth").unwrap(), Address::from_str(crate::constants::WETH).unwrap());
        assert_eq!(crate::constants::get_token_address("usdc").unwrap(), Address::from_str(crate::constants::USDC).unwrap());
        
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
            OpportunityType::MicroArbitrage,
            StrategyType::MicroArbitrage,
            U256::from(1000000000000000000u64),
            0.8,
            0, // Zero gas
            1000,
            OpportunityDetails::Arbitrage(ArbitrageDetails {
                token_in: Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                token_out: Address::from_str("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46").unwrap(),
                amount_in: U256::from(1000000000000000000u64),
                amount_out: U256::from(2000000000000000000u64),
                dex_path: vec!["uniswap_v2".to_string()],
                price_impact: 0.01,
            }),
        );
        
        assert_eq!(opportunity.profit_per_gas(), 0.0);
    }
}

// ================================
// Cross-Chain Arbitrage Types
// ================================

/// Supported blockchain networks
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChainId {
    Ethereum = 1,
    Polygon = 137,
    BSC = 56,
    Arbitrum = 42161,
    Optimism = 10,
    Avalanche = 43114,
}

impl ChainId {
    pub fn name(&self) -> &'static str {
        match self {
            ChainId::Ethereum => "ethereum",
            ChainId::Polygon => "polygon",
            ChainId::BSC => "bsc",
            ChainId::Arbitrum => "arbitrum",
            ChainId::Optimism => "optimism",
            ChainId::Avalanche => "avalanche",
        }
    }

    pub fn native_token(&self) -> &'static str {
        match self {
            ChainId::Ethereum => "ETH",
            ChainId::Polygon => "MATIC",
            ChainId::BSC => "BNB",
            ChainId::Arbitrum => "ETH",
            ChainId::Optimism => "ETH",
            ChainId::Avalanche => "AVAX",
        }
    }
}

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Bridge protocol types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BridgeProtocol {
    Stargate,
    Hop,
    Synapse,
    Rubic,
    Across,
    Multichain,
}

impl BridgeProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            BridgeProtocol::Stargate => "stargate",
            BridgeProtocol::Hop => "hop",
            BridgeProtocol::Synapse => "synapse", 
            BridgeProtocol::Rubic => "rubic",
            BridgeProtocol::Across => "across",
            BridgeProtocol::Multichain => "multichain",
        }
    }
}

/// Cross-chain token information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrossChainToken {
    /// Token symbol (e.g., "USDC")
    pub symbol: String,
    /// Token addresses on different chains
    pub addresses: std::collections::HashMap<ChainId, Address>,
    /// Token decimals (usually same across chains)
    pub decimals: u8,
}

/// Cross-chain arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrossChainArbitrageOpportunity {
    /// Unique opportunity ID
    pub id: String,
    /// Token to arbitrage
    pub token: CrossChainToken,
    /// Source chain (buy from)
    pub source_chain: ChainId,
    /// Destination chain (sell to)
    pub dest_chain: ChainId,
    /// Price on source chain
    pub source_price: f64,
    /// Price on destination chain
    pub dest_price: f64,
    /// Price difference percentage
    pub price_diff_percent: f64,
    /// Trade amount
    pub amount: U256,
    /// Bridge protocol to use
    pub bridge_protocol: BridgeProtocol,
    /// Estimated bridge cost
    pub bridge_cost: U256,
    /// Estimated total gas costs
    pub total_gas_cost: U256,
    /// Expected profit (after costs)
    pub expected_profit: U256,
    /// Profit percentage
    pub profit_percent: f64,
    /// Estimated execution time (seconds)
    pub estimated_time: u64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Timestamp when opportunity was discovered
    pub discovered_at: DateTime<Utc>,
    /// Expiry time for this opportunity
    pub expires_at: DateTime<Utc>,
}

impl CrossChainArbitrageOpportunity {
    /// Check if opportunity is still valid
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at && self.expected_profit > U256::ZERO
    }

    /// Calculate profitability score
    pub fn profitability_score(&self) -> f64 {
        (self.profit_percent * self.confidence) / (self.estimated_time as f64 / 60.0) // per minute
    }
}

/// Bridge route information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgeRoute {
    /// Bridge protocol
    pub protocol: BridgeProtocol,
    /// Source chain
    pub from_chain: ChainId,
    /// Destination chain  
    pub to_chain: ChainId,
    /// Token to bridge
    pub token: CrossChainToken,
    /// Bridge cost
    pub cost: U256,
    /// Estimated time (seconds)
    pub estimated_time: u64,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Whether this route requires destination gas
    pub requires_dest_gas: bool,
}

/// Cross-chain trade execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrossChainTradeStatus {
    /// Trade initiated
    Initiated,
    /// Source chain transaction confirmed
    SourceTxConfirmed { tx_hash: B256 },
    /// Bridge transaction in progress
    BridgeInProgress { bridge_tx_hash: Option<B256> },
    /// Bridge completed, destination chain transaction pending
    BridgeCompleted,
    /// Destination chain transaction confirmed
    DestTxConfirmed { tx_hash: B256 },
    /// Trade completed successfully
    Completed {
        source_tx_hash: B256,
        dest_tx_hash: B256,
        actual_profit: U256,
    },
    /// Trade failed
    Failed {
        reason: String,
        stage: CrossChainTradeStage,
        recovery_possible: bool,
    },
}

/// Cross-chain trade execution stages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrossChainTradeStage {
    SourceChainBuy,
    BridgeTransfer,
    DestChainSell,
}

/// Cross-chain trade execution record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrossChainTrade {
    /// Trade ID
    pub id: String,
    /// Opportunity that triggered this trade
    pub opportunity: CrossChainArbitrageOpportunity,
    /// Current status
    pub status: CrossChainTradeStatus,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// Completion time (if completed)
    pub completed_at: Option<DateTime<Utc>>,
    /// Actual execution time
    pub actual_execution_time: Option<u64>,
    /// Source chain transaction hash
    pub source_tx_hash: Option<B256>,
    /// Destination chain transaction hash  
    pub dest_tx_hash: Option<B256>,
    /// Actual profit realized
    pub actual_profit: Option<U256>,
    /// Error message (if failed)
    pub error_message: Option<String>,
}

impl CrossChainTrade {
    pub fn new(opportunity: CrossChainArbitrageOpportunity) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            opportunity,
            status: CrossChainTradeStatus::Initiated,
            started_at: Utc::now(),
            completed_at: None,
            actual_execution_time: None,
            source_tx_hash: None,
            dest_tx_hash: None,
            actual_profit: None,
            error_message: None,
        }
    }

    pub fn is_completed(&self) -> bool {
        matches!(
            self.status,
            CrossChainTradeStatus::Completed { .. } | CrossChainTradeStatus::Failed { .. }
        )
    }
}