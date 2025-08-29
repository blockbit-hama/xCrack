// xCrack MEV Searcher Library

pub mod config;
pub mod strategies;
pub mod protocols;
pub mod dex;
pub mod utils;
pub mod execution;
pub mod mev;
pub mod exchange;
pub mod adapters;
pub mod mocks;
pub mod bridges;
pub mod blockchain;
pub mod oracle;
pub mod opportunity;
pub mod storage;
pub mod flashbots;

// Core types
pub mod types;
pub mod constants;

// Re-exports for convenience
pub use config::Config;
pub use strategies::{IntegratedLiquidationManager, LiquidationStrategyV2};
pub use protocols::MultiProtocolScanner;
pub use mev::MEVBundleExecutor;