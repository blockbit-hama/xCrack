//! Multi-Asset Arbitrage 전략 모듈
//!
//! 이 모듈은 Aave V3의 다중자산 flashLoan을 사용하여
//! 복잡한 차익거래를 수행하는 multi-asset arbitrage 전략을 포함합니다.
//!
//! ## 주요 기능
//!
//! - **삼각 아비트리지**: A → B → C → A 경로로 차익 획득
//! - **포지션 마이그레이션**: Aave → Compound 등 프로토콜 간 포지션 이동
//! - **복합 아비트리지**: 여러 DEX를 거친 복잡한 경로
//!
//! ## 모듈 구성
//!
//! - **types**: 타입 정의 (기회, 통계, 전략 타입 등)
//! - **opportunity_detector**: 기회 탐지 (삼각, 복합 아비트리지)
//! - **flashloan_executor**: Aave V3 다중자산 FlashLoan 실행
//! - **execution_engine**: 아비트리지 실행 엔진
//! - **manager**: 통합 관리자 (Strategy trait 구현)

pub mod types;
pub mod opportunity_detector;
pub mod flashloan_executor;
pub mod execution_engine;
pub mod manager;

// Re-exports
pub use types::*;
pub use opportunity_detector::OpportunityDetector;
pub use flashloan_executor::AaveFlashLoanExecutor;
pub use execution_engine::ExecutionEngine;
pub use manager::MultiAssetArbitrageManager;

// 레거시 호환성을 위한 재export (기존 multi_asset_arbitrage.rs를 사용하는 코드)
pub use manager::MultiAssetArbitrageManager as MultiAssetArbitrageStrategy;
