use async_trait::async_trait;
use anyhow::Result;
use crate::types::*;

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
} 