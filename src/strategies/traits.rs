use async_trait::async_trait;
use anyhow::Result;
use crate::types::*;
use uuid::Uuid;

/// Strategy trait that all MEV strategies must implement
#[async_trait]
pub trait Strategy: Send + Sync {
    /// Get the strategy type
    fn strategy_type(&self) -> StrategyType;
    
    /// Check if the strategy is enabled
    fn is_enabled(&self) -> bool;
    
    /// Start the strategy
    async fn start(&self) -> Result<()>;
    
    /// Stop the strategy
    async fn stop(&self) -> Result<()>;
    
    /// Analyze a transaction for opportunities
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>>;
    
    /// Validate an opportunity
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool>;
    
    /// Create a bundle from an opportunity
    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle>;
    
    /// Get strategy ID (for predictive strategies)
    fn id(&self) -> Uuid {
        Uuid::new_v4()
    }
    
    /// Get strategy name (for predictive strategies)
    fn name(&self) -> &str {
        "Unknown"
    }
    
    /// Process external signal (for predictive strategies)
    async fn process_signal(&self, _signal: StrategySignal) -> Result<()> {
        Ok(())
    }
    
    /// Check if strategy is active (for predictive strategies)
    fn is_active(&self) -> bool {
        true
    }
} 