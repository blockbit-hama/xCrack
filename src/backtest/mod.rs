//! 통합 백테스팅 시스템
//! xQuant의 백테스팅 기능을 xCrack의 MEV 전략과 결합

pub mod engine;
pub mod data_provider;
pub mod performance;
pub mod scenarios;

pub use engine::BacktestEngine;
pub use data_provider::{DataProvider, HistoricalDataSource};
pub use performance::PerformanceAnalyzer;
pub use scenarios::{BacktestScenario, ScenarioBuilder};