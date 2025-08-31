// xCrack MEV Searcher Library

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

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
pub use strategies::{integrated_liquidation_manager::IntegratedLiquidationManager, LiquidationStrategyV2};
pub use protocols::MultiProtocolScanner;
pub use mev::MEVBundleExecutor;