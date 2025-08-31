/// Backtesting scenarios and test cases
use crate::types::StrategyType;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestScenario {
    pub name: String,
    pub description: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub initial_balance: u128,
    pub strategies: Vec<StrategyType>,
    pub market_conditions: MarketConditions,
    pub expected_outcomes: ExpectedOutcomes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    pub volatility: VolatilityLevel,
    pub trend: TrendDirection,
    pub liquidity: LiquidityLevel,
    pub gas_price_range: (u64, u64), // (min, max) in gwei
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolatilityLevel {
    Low,    // < 20% daily
    Medium, // 20-50% daily
    High,   // > 50% daily
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Bullish,
    Bearish,
    Sideways,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidityLevel {
    Low,    // Large spreads, low volume
    Medium, // Normal market conditions
    High,   // Tight spreads, high volume
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedOutcomes {
    pub min_profit: u128,
    pub max_drawdown: f64,
    pub min_success_rate: f64,
    pub max_gas_cost: u128,
}

pub struct ScenarioGenerator;

impl ScenarioGenerator {
    /// Generate a bull market scenario
    pub fn bull_market_scenario() -> BacktestScenario {
        BacktestScenario {
            name: "Bull Market".to_string(),
            description: "Strong uptrend with high volatility and good liquidity".to_string(),
            start_time: Utc::now() - Duration::days(30),
            end_time: Utc::now(),
            initial_balance: 1000000000000000000, // 1 ETH
            strategies: vec![StrategyType::MicroArbitrage, StrategyType::Sandwich],
            market_conditions: MarketConditions {
                volatility: VolatilityLevel::Medium,
                trend: TrendDirection::Bullish,
                liquidity: LiquidityLevel::High,
                gas_price_range: (20, 100),
            },
            expected_outcomes: ExpectedOutcomes {
                min_profit: 50000000000000000, // 0.05 ETH
                max_drawdown: 0.1,
                min_success_rate: 0.7,
                max_gas_cost: 10000000000000000, // 0.01 ETH
            },
        }
    }
    
    /// Generate a bear market scenario
    pub fn bear_market_scenario() -> BacktestScenario {
        BacktestScenario {
            name: "Bear Market".to_string(),
            description: "Downtrend with high volatility and reduced liquidity".to_string(),
            start_time: Utc::now() - Duration::days(30),
            end_time: Utc::now(),
            initial_balance: 1000000000000000000, // 1 ETH
            strategies: vec![StrategyType::Liquidation, StrategyType::MicroArbitrage],
            market_conditions: MarketConditions {
                volatility: VolatilityLevel::High,
                trend: TrendDirection::Bearish,
                liquidity: LiquidityLevel::Medium,
                gas_price_range: (15, 80),
            },
            expected_outcomes: ExpectedOutcomes {
                min_profit: 30000000000000000, // 0.03 ETH
                max_drawdown: 0.15,
                min_success_rate: 0.6,
                max_gas_cost: 15000000000000000, // 0.015 ETH
            },
        }
    }
    
    /// Generate a sideways market scenario
    pub fn sideways_market_scenario() -> BacktestScenario {
        BacktestScenario {
            name: "Sideways Market".to_string(),
            description: "Range-bound market with low volatility".to_string(),
            start_time: Utc::now() - Duration::days(30),
            end_time: Utc::now(),
            initial_balance: 1000000000000000000, // 1 ETH
            strategies: vec![StrategyType::MicroArbitrage],
            market_conditions: MarketConditions {
                volatility: VolatilityLevel::Low,
                trend: TrendDirection::Sideways,
                liquidity: LiquidityLevel::High,
                gas_price_range: (10, 30),
            },
            expected_outcomes: ExpectedOutcomes {
                min_profit: 20000000000000000, // 0.02 ETH
                max_drawdown: 0.05,
                min_success_rate: 0.8,
                max_gas_cost: 5000000000000000, // 0.005 ETH
            },
        }
    }
    
    /// Generate a high gas scenario
    pub fn high_gas_scenario() -> BacktestScenario {
        BacktestScenario {
            name: "High Gas Environment".to_string(),
            description: "Normal market with extremely high gas prices".to_string(),
            start_time: Utc::now() - Duration::days(7),
            end_time: Utc::now(),
            initial_balance: 1000000000000000000, // 1 ETH
            strategies: vec![StrategyType::Sandwich, StrategyType::Liquidation],
            market_conditions: MarketConditions {
                volatility: VolatilityLevel::Medium,
                trend: TrendDirection::Sideways,
                liquidity: LiquidityLevel::Medium,
                gas_price_range: (200, 500), // Very high gas
            },
            expected_outcomes: ExpectedOutcomes {
                min_profit: 100000000000000000, // 0.1 ETH (need high profit to cover gas)
                max_drawdown: 0.2,
                min_success_rate: 0.5,
                max_gas_cost: 50000000000000000, // 0.05 ETH
            },
        }
    }
    
    /// Generate a flash crash scenario
    pub fn flash_crash_scenario() -> BacktestScenario {
        BacktestScenario {
            name: "Flash Crash".to_string(),
            description: "Sudden market crash with liquidation opportunities".to_string(),
            start_time: Utc::now() - Duration::hours(4),
            end_time: Utc::now(),
            initial_balance: 1000000000000000000, // 1 ETH
            strategies: vec![StrategyType::Liquidation, StrategyType::Sandwich],
            market_conditions: MarketConditions {
                volatility: VolatilityLevel::High,
                trend: TrendDirection::Bearish,
                liquidity: LiquidityLevel::Low,
                gas_price_range: (50, 300),
            },
            expected_outcomes: ExpectedOutcomes {
                min_profit: 200000000000000000, // 0.2 ETH (high profit opportunity)
                max_drawdown: 0.3,
                min_success_rate: 0.4,
                max_gas_cost: 30000000000000000, // 0.03 ETH
            },
        }
    }
    
    /// Get all predefined scenarios
    pub fn get_all_scenarios() -> Vec<BacktestScenario> {
        vec![
            Self::bull_market_scenario(),
            Self::bear_market_scenario(),
            Self::sideways_market_scenario(),
            Self::high_gas_scenario(),
            Self::flash_crash_scenario(),
        ]
    }
    
    /// Generate custom scenario
    pub fn custom_scenario(
        name: String,
        description: String,
        duration_days: i64,
        market_conditions: MarketConditions,
        strategies: Vec<StrategyType>,
    ) -> BacktestScenario {
        let end_time = Utc::now();
        let start_time = end_time - Duration::days(duration_days);
        
        BacktestScenario {
            name,
            description,
            start_time,
            end_time,
            initial_balance: 1000000000000000000, // 1 ETH
            strategies,
            market_conditions,
            expected_outcomes: ExpectedOutcomes {
                min_profit: 10000000000000000, // 0.01 ETH
                max_drawdown: 0.2,
                min_success_rate: 0.5,
                max_gas_cost: 20000000000000000, // 0.02 ETH
            },
        }
    }
}

/// Scenario builder for creating custom scenarios
pub struct ScenarioBuilder;

impl ScenarioBuilder {
    /// Create a new scenario builder
    pub fn new() -> Self {
        Self
    }
    
    /// Build scenario from configuration
    pub fn build(
        name: String,
        description: String,
        duration_days: i64,
        _initial_balance: u128,
        strategies: Vec<StrategyType>,
        market_conditions: MarketConditions,
    ) -> BacktestScenario {
        ScenarioGenerator::custom_scenario(name, description, duration_days, market_conditions, strategies)
    }
}

impl Default for ScenarioBuilder {
    fn default() -> Self {
        Self::new()
    }
}