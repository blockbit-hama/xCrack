use std::time::Instant;
use alloy::primitives::{Address, U256};

#[derive(Debug, Clone)]
pub struct LendingProtocolInfo {
    pub name: String,
    pub protocol_type: ProtocolType,
    pub lending_pool_address: Address,
    pub price_oracle_address: Option<Address>,
    pub liquidation_fee: u32, // basis points
    pub min_health_factor: f64,
    pub supported_assets: Vec<Address>,
}

#[derive(Debug, Clone)]
pub enum ProtocolType {
    Aave,
    Compound,
    MakerDAO,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UserPosition {
    pub user: Address,
    pub protocol: Address,
    pub collateral_assets: Vec<CollateralPosition>,
    pub debt_assets: Vec<DebtPosition>,
    pub health_factor: f64,
    pub liquidation_threshold: f64,
    pub total_collateral_usd: f64,
    pub total_debt_usd: f64,
    pub last_updated: Instant,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CollateralPosition {
    pub asset: Address,
    pub amount: U256,
    pub usd_value: f64,
    pub liquidation_threshold: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DebtPosition {
    pub asset: Address,
    pub amount: U256,
    pub usd_value: f64,
    pub borrow_rate: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AssetPrice {
    pub asset: Address,
    pub price_usd: f64,
    pub price_eth: f64,
    pub last_updated: Instant,
    pub source: PriceSource,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PriceSource {
    Chainlink,
    Uniswap,
    Compound,
    Manual,
}

#[derive(Debug, Clone)]
pub struct OnChainLiquidationOpportunity {
    /// 대상 사용자
    pub target_user: Address,
    /// 프로토콜
    pub protocol: LendingProtocolInfo,
    /// 사용자 포지션
    pub position: UserPosition,
    /// 청산할 담보 자산
    pub collateral_asset: Address,
    /// 상환할 부채 자산
    pub debt_asset: Address,
    /// 청산 가능 금액
    pub liquidation_amount: U256,
    /// 받을 담보 금액
    pub collateral_amount: U256,
    /// 청산 보상 (할인)
    pub liquidation_bonus: U256,
    /// 예상 수익
    pub expected_profit: U256,
    /// 가스 비용
    pub gas_cost: U256,
    /// 순수익
    pub net_profit: U256,
    /// 성공 확률
    pub success_probability: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ZeroExQuoteWire {
    #[serde(rename = "sellAmount")]
    pub sell_amount: String,
    #[serde(rename = "buyAmount")]
    pub buy_amount: String,
    pub price: String,
    #[serde(rename = "guaranteedPrice")]
    pub guaranteed_price: String,
}

#[derive(Debug, Clone)]
pub struct ZeroExQuote {
    pub to: Address,
    pub data: alloy::primitives::Bytes,
    pub value: Option<U256>,
    pub allowance_target: Option<Address>,
}

#[derive(Debug, Clone)]
pub(crate) struct PrivateSubmissionResult {
    pub success: bool,
    pub bundle_hash: Option<String>,
    pub error: Option<String>,
    pub relay_name: Option<String>,
}

/// 청산 실행 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionMode {
    /// Flashbot을 통한 프라이빗 트랜잭션 (MEV 보호)
    Flashbot,
    /// 퍼블릭 멤풀로 직접 브로드캐스트
    Public,
    /// Flashbot 먼저 시도, 실패 시 Public으로 폴백
    Hybrid,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Hybrid
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Flashbot => write!(f, "Flashbot"),
            ExecutionMode::Public => write!(f, "Public"),
            ExecutionMode::Hybrid => write!(f, "Hybrid"),
        }
    }
}

/// 청산 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiquidationConfig {
    /// 실행 모드
    pub execution_mode: ExecutionMode,
    /// 최소 수익 임계값 (USD)
    pub min_profit_threshold_usd: f64,
    /// 스캔 간격 (초)
    pub scan_interval_seconds: u64,
    /// 최대 동시 청산 수
    pub max_concurrent_liquidations: usize,
    /// Flashloan 사용 여부
    pub use_flashloan: bool,
    /// 선호하는 Flashloan 프로바이더
    pub preferred_flashloan_provider: String,
    /// 가스 가격 (Gwei)
    pub gas_price_gwei: f64,
    /// 가스 승수 (경쟁력 향상)
    pub gas_multiplier: f64,
    /// 자동 실행 여부
    pub auto_execute: bool,
    /// Flashbot 우선 팁 (ETH)
    pub flashbot_priority_tip_eth: f64,
    /// Public 모드 슬리피지 허용치 (%)
    pub public_slippage_tolerance_percent: f64,
}

impl Default for LiquidationConfig {
    fn default() -> Self {
        Self {
            execution_mode: ExecutionMode::Hybrid,
            min_profit_threshold_usd: 100.0,
            scan_interval_seconds: 10,
            max_concurrent_liquidations: 3,
            use_flashloan: true,
            preferred_flashloan_provider: "aave_v3".to_string(),
            gas_price_gwei: 30.0,
            gas_multiplier: 1.2,
            auto_execute: false,
            flashbot_priority_tip_eth: 0.01,
            public_slippage_tolerance_percent: 1.0,
        }
    }
}
