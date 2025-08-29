pub mod sandwich; 
pub mod liquidation;
pub mod liquidation_v2;
pub mod integrated_liquidation_manager;
pub mod micro_arbitrage;
pub mod multi_asset_arbitrage;
pub mod manager;
pub mod utils;
pub mod traits;
pub mod execution_engine;
pub mod cross_chain_arbitrage;
pub mod gas_optimization;
pub mod liquidation_state_indexer;
pub mod liquidation_mempool_watcher;
pub mod liquidation_bundle_builder;
pub mod liquidation_execution_engine;
pub mod liquidation_strategy_manager;

// On-chain integrated strategies
pub mod sandwich_onchain;
pub mod liquidation_onchain;


// Re-exports
pub use sandwich::RealTimeSandwichStrategy;
pub use liquidation::CompetitiveLiquidationStrategy;
pub use liquidation_v2::{LiquidationStrategyV2, LiquidationOpportunity as LiquidationOpportunityV2, LiquidationStrategyStats};
pub use integrated_liquidation_manager::{IntegratedLiquidationManager, PerformanceMetrics, LiquidationSummary};
pub use micro_arbitrage::MicroArbitrageStrategy;
pub use multi_asset_arbitrage::MultiAssetArbitrageStrategy;
pub use manager::StrategyManager;
pub use traits::Strategy;
pub use execution_engine::{QuantExecutionEngine, ExecutionStrategy, ExecutionTask};
pub use cross_chain_arbitrage::{CrossChainArbitrageStrategy, run_cross_chain_arbitrage_mock};

// On-chain strategy re-exports
pub use sandwich_onchain::OnChainSandwichStrategy;
pub use liquidation_onchain::OnChainLiquidationStrategy;
pub use gas_optimization::{GasOptimizer, GasStrategy, SandwichOpportunity};
pub use liquidation_state_indexer::{LiquidationStateIndexer, LiquidationCandidate, ProtocolConfig};
pub use liquidation_mempool_watcher::{LiquidationMempoolWatcher, LiquidationSignal, LiquidationUrgency};
pub use liquidation_bundle_builder::{LiquidationBundleBuilder, LiquidationBundle, LiquidationScenario};
pub use liquidation_execution_engine::{LiquidationExecutionEngine, SubmissionResult, ExecutionStats};
pub use liquidation_strategy_manager::{LiquidationStrategyManager, LiquidationOpportunity};
// pub use utils::*;
