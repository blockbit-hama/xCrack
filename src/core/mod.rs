//! 핵심 시스템 모듈
//! 
//! 이 모듈은 MEV 시스템의 핵심 인프라스트럭처를 제공합니다.
//! 전략별 특화 기능이 아닌, 모든 전략이 공통으로 사용하는
//! 시스템 레벨의 핵심 기능들을 포함합니다.
//! 
//! ## 모듈 구성
//! 
//! ### 🔍 검색 엔진
//! - **SearcherCore**: 메인 검색 및 실행 엔진
//!   - 기회 탐지 및 분석
//!   - 전략 실행 조율
//!   - 시스템 상태 관리
//! 
//! ### 📦 번들 관리
//! - **BundleManager**: MEV 번들 생성 및 관리
//!   - 번들 생성 및 최적화
//!   - Flashbots 제출 관리
//!   - 번들 상태 추적
//! 
//! ### 📡 데이터 수집
//! - **MempoolMonitor**: 트랜잭션 멤풀 감시
//!   - 실시간 멤풀 모니터링
//!   - 트랜잭션 필터링
//!   - 기회 탐지 트리거
//! 
//! ### 📊 성능 관리
//! - **PerformanceTracker**: 시스템 성능 모니터링
//!   - 수익성 추적
//!   - 가스 효율성 분석
//!   - 시스템 메트릭 수집
//! 
//! ### 🔧 트랜잭션 처리
//! - **TransactionBuilder**: 트랜잭션 생성 및 최적화
//!   - 청산 트랜잭션 구축
//!   - 가스 최적화
//!   - ABI 인코딩/디코딩
//! 
//! ### 📈 모니터링
//! - **MonitoringManager**: 모니터링 API 및 대시보드
//!   - HTTP API 엔드포인트
//!   - 실시간 메트릭 제공
//!   - 시스템 상태 대시보드

pub mod searcher_core;
pub mod bundle_manager;
pub mod mempool_monitor;
pub mod performance_tracker;
pub mod transaction_builder;
pub mod monitoring_manager;

pub use searcher_core::SearcherCore;
pub use bundle_manager::BundleManager;
pub use mempool_monitor::CoreMempoolMonitor;
pub use performance_tracker::PerformanceTracker;
pub use transaction_builder::TransactionBuilder;
pub use monitoring_manager::MonitoringManager;
