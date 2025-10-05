// xCrack MEV Searcher Library

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

pub mod config;
pub mod common;
pub mod core;
pub mod strategies;
pub mod protocols;
pub mod dex;
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
pub use strategies::liquidation::IntegratedLiquidationManager;
pub use protocols::MultiProtocolScanner;
pub use mev::MEVBundleExecutor;