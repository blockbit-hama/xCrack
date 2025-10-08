//! Micro Arbitrage 전략 모듈
//!
//! 이 모듈은 CEX와 DEX 간의 가격 차이를 이용하여
//! 소액 차익거래를 수행하는 micro arbitrage 전략을 포함합니다.

pub mod types;
pub mod price_monitor;
pub mod opportunity_detector;
pub mod execution_engine;
pub mod risk_manager;
pub mod performance_tracker;
pub mod manager;
pub mod aave_flashloan;

// Re-export main types and structs
pub use types::*;
pub use price_monitor::PriceMonitor;
pub use opportunity_detector::OpportunityDetector;
pub use execution_engine::ExecutionEngine;
pub use risk_manager::RiskManager;
pub use performance_tracker::PerformanceTracker;
pub use manager::MicroArbitrageManager;
pub use aave_flashloan::AaveFlashLoanExecutor;