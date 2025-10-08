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
// pub mod bridges;  // 크로스체인 아비트리지 제거됨
pub mod blockchain;
pub mod oracle;
pub mod opportunity;
pub mod storage;
pub mod flashbots;
pub mod mempool;

// Core types
pub mod types;
pub mod constants;

// Re-exports for convenience
pub use config::Config;
pub use strategies::liquidation::IntegratedLiquidationManager;
pub use protocols::MultiProtocolScanner;
pub use mev::MEVBundleExecutor;

// Re-export common types
pub use common::Strategy;
pub use common::abi::*;
pub use common::profitability::*;
pub use common::crypto::*;
pub use common::formatting::*;
pub use common::math::*;
pub use common::network::*;
pub use common::time::*;
pub use common::validation::*;
pub use common::traits::*;
pub use common::gas_optimization::*;