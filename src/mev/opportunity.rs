use alloy::primitives::U256;
use serde::{Deserialize, Serialize};

/// MEV 기회
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: String,
    pub strategy_type: MEVStrategy,
    pub estimated_profit: U256,
    pub gas_cost: U256,
    pub net_profit: U256,
    pub success_probability: f64,
    pub execution_time_ms: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// MEV 전략 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MEVStrategy {
    Liquidation,
    Arbitrage,
    Sandwich,
    CrossChainArbitrage,
    FlashLoan,
}

impl Opportunity {
    pub fn new(
        strategy_type: MEVStrategy,
        estimated_profit: U256,
        gas_cost: U256,
        success_probability: f64,
    ) -> Self {
        let net_profit = if estimated_profit > gas_cost {
            estimated_profit - gas_cost
        } else {
            U256::from(0)
        };
        
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            strategy_type,
            estimated_profit,
            gas_cost,
            net_profit,
            success_probability,
            execution_time_ms: 1000, // 기본 1초
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(300), // 5분 후 만료
        }
    }
    
    pub fn is_profitable(&self) -> bool {
        self.net_profit > U256::from(0)
    }
    
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }
    
    pub fn priority_score(&self) -> f64 {
        let profit_score = self.net_profit.to::<u128>() as f64 / 1e18;
        let time_score = 1.0 - (chrono::Utc::now() - self.created_at).num_seconds() as f64 / 300.0;
        let success_score = self.success_probability;
        
        profit_score * 0.5 + time_score * 0.3 + success_score * 0.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_opportunity_creation() {
        let opportunity = Opportunity::new(
            MEVStrategy::Liquidation,
            U256::from(1_000_000_000_000_000_000u64), // 1 ETH
            U256::from(100_000_000_000_000_000u64), // 0.1 ETH
            0.8,
        );
        
        assert!(opportunity.is_profitable());
        assert!(!opportunity.is_expired());
    }
    
    #[test]
    fn test_priority_score() {
        let opportunity = Opportunity::new(
            MEVStrategy::Arbitrage,
            U256::from(500_000_000_000_000_000u64), // 0.5 ETH
            U256::from(50_000_000_000_000_000u64), // 0.05 ETH
            0.9,
        );
        
        let score = opportunity.priority_score();
        assert!(score > 0.0);
    }
}
