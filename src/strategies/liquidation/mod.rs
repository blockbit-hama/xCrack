//! 청산 전략 모듈
//! 
//! 이 모듈은 DeFi 프로토콜에서 청산 가능한 포지션을 감지하고 실행하는
//! 다양한 청산 전략들을 포함합니다.

pub mod types;
pub mod stats;
pub mod position_scanner;
pub mod position_analyzer;
pub mod liquidation_executor;
pub mod price_oracle;
pub mod manager;
pub mod bundle_builder;
pub mod execution_engine;
pub mod mempool_watcher;
pub mod state_indexer;
pub mod strategy_manager;

// Re-export main types and structs
pub use types::*;
pub use manager::IntegratedLiquidationManager;
pub use liquidation_executor::LiquidationExecutor;
pub use position_scanner::PositionScanner;
pub use position_analyzer::PositionAnalyzer;
pub use price_oracle::PriceOracle;