//! 샌드위치 공격 전략 모듈
//!
//! 이 모듈은 DEX에서 대형 스왑 트랜잭션을 감지하고, 해당 트랜잭션 앞뒤로
//! 우리의 트랜잭션을 삽입하여 가격 변동으로부터 수익을 추출하는 샌드위치 공격 전략을 구현합니다.
//!
//! ## 주요 컴포넌트
//!
//! - `types`: 핵심 타입 정의 (SandwichOpportunity, SandwichBundle 등)
//! - `stats`: 통계 관리
//! - `dex_router`: DEX 라우터 관리 (Uniswap V2/V3, SushiSwap 등)
//! - `mempool_monitor`: 실시간 멤풀 모니터링
//! - `target_analyzer`: 타겟 트랜잭션 분석
//! - `profitability`: 수익성 분석 및 Kelly Criterion 구현
//! - `strategy_manager`: 전략 조정 및 의사결정
//! - `bundle_builder`: MEV 번들 생성
//! - `executor`: Flashbots 번들 실행
//! - `manager`: 최상위 통합 매니저
//!
//! ## 아키텍처
//!
//! ```text
//! IntegratedSandwichManager
//! ├── MempoolMonitor (실시간 트랜잭션 감시)
//! │   └── DexRouterManager (DEX 스왑 감지)
//! ├── SandwichStrategyManager (전략 조정)
//! │   ├── TargetAnalyzer (타겟 분석)
//! │   └── ProfitabilityAnalyzer (수익성 분석 + Kelly Criterion)
//! ├── SandwichBundleBuilder (MEV 번들 생성)
//! └── SandwichExecutor (Flashbots 실행)
//!     └── SandwichStatsManager (통계)
//! ```
//!
//! ## Kelly Criterion
//!
//! 최적 포지션 크기 계산:
//! - f* = (p * b - q) / b
//! - p: 성공 확률
//! - q: 실패 확률 (1-p)
//! - b: 가격 영향 (price impact)
//! - Half Kelly 사용으로 위험 조정
//!
//! ## 사용 예시
//!
//! ```rust,no_run
//! use xcrack::strategies::sandwich::IntegratedSandwichManager;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = Arc::new(Config::default());
//!     let provider = Arc::new(Provider::<Ws>::connect("ws://localhost:8545").await?);
//!     let wallet = LocalWallet::new(&mut rand::thread_rng());
//!     let contract_address = "0x...".parse()?;
//!
//!     let manager = IntegratedSandwichManager::new(
//!         config,
//!         provider,
//!         wallet,
//!         contract_address,
//!     ).await?;
//!
//!     manager.start().await?;
//!
//!     // 전략 실행...
//!
//!     manager.stop().await?;
//!     Ok(())
//! }
//! ```

pub mod types;
pub mod stats;
pub mod dex_router;
pub mod mempool_monitor;
pub mod target_analyzer;
pub mod profitability;
pub mod strategy_manager;
pub mod bundle_builder;
pub mod executor;
pub mod manager;

// Legacy modules (deprecated)
pub mod sandwich;
pub mod sandwich_onchain;

// Re-export main types and structs
pub use types::*;
pub use manager::IntegratedSandwichManager;
pub use stats::SandwichStatsManager;
pub use dex_router::DexRouterManager;
pub use mempool_monitor::MempoolMonitor;
pub use target_analyzer::TargetAnalyzer;
pub use profitability::ProfitabilityAnalyzer;
pub use strategy_manager::SandwichStrategyManager;
pub use bundle_builder::SandwichBundleBuilder;
pub use executor::SandwichExecutor;

// Legacy re-exports
pub use sandwich::*;
pub use sandwich_onchain::*;