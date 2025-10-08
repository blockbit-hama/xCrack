//! 마이크로아비트리지 전략 타입 정의
//! 
//! 이 모듈은 마이크로아비트리지 전략에서 사용되는
//! 모든 데이터 구조와 타입을 정의합니다.

use std::collections::HashMap;
use std::time::Instant;
use ethers::types::{Address, U256, H256};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

/// 거래소 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExchangeType {
    CEX,  // 중앙화 거래소
    DEX,  // 탈중앙화 거래소
}

/// 거래소 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeInfo {
    pub name: String,
    pub exchange_type: ExchangeType,
    pub api_endpoint: String,
    pub trading_pairs: Vec<String>,
    pub fee_percentage: f64,
    pub min_order_size: U256,
    pub max_order_size: U256,
    pub latency_ms: u64,
    pub is_active: bool,
    pub last_heartbeat: Option<Instant>,
}

/// 가격 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub symbol: String,
    pub exchange: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last_price: Decimal,
    pub volume_24h: U256,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
    pub spread: Decimal,
    pub price_impact: f64,
}

/// 오더북 스냅샷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub symbol: String,
    pub exchange: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}

/// 오더북 레벨
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub amount: U256,
    pub orders: u32,
}

/// 마이크로아비트리지 기회
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroArbitrageOpportunity {
    pub id: String,
    pub token_symbol: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub price_spread: Decimal,
    pub profit_percentage: f64,
    pub max_amount: U256,
    pub execution_window_ms: u64,
    pub confidence_score: f64,
    pub expected_profit: U256,
    pub buy_amount: U256,
    pub base_asset: String,
    pub quote_asset: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub gas_cost: U256,
    pub net_profit: U256,
    pub success_probability: f64,
}

/// 아비트리지 실행 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageExecutionResult {
    pub opportunity_id: String,
    pub success: bool,
    pub transaction_hashes: Vec<H256>,
    pub actual_profit: Option<U256>,
    pub gas_used: U256,
    pub execution_time_ms: u64,
    pub error_message: Option<String>,
    pub slippage: f64,
    pub fees_paid: U256,
    pub created_at: DateTime<Utc>,
}

/// 주문 상태
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// 주문 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderInfo {
    pub order_id: String,
    pub exchange: String,
    pub symbol: String,
    pub side: OrderSide,
    pub amount: U256,
    pub price: Decimal,
    pub status: OrderStatus,
    pub filled_amount: U256,
    pub remaining_amount: U256,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 주문 방향
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// 자금 조달 방식
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FundingMode {
    Wallet,     // 지갑 잔고 사용
    FlashLoan,  // 플래시론 사용
    Auto,       // 자동 선택
}

/// 자금 조달 메트릭
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingMetrics {
    pub mode: FundingMode,
    pub gross_profit: U256,
    pub total_cost: U256,
    pub net_profit: U256,
    pub gas_cost: U256,
    pub premium_cost: U256,
    pub success_probability: f64,
    pub liquidity_available: bool,
    pub estimated_execution_time_ms: u64,
}

/// 위험 관리 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskManagementConfig {
    pub max_position_size: U256,
    pub max_daily_volume: U256,
    pub max_daily_loss: U256,
    pub max_concurrent_trades: usize,
    pub min_profit_threshold: U256,
    pub max_slippage_percentage: f64,
    pub stop_loss_percentage: f64,
    pub position_timeout_seconds: u64,
}

/// 성능 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroArbitrageStats {
    pub total_opportunities: u64,
    pub executed_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_volume: U256,
    pub total_profit: U256,
    pub total_fees: U256,
    pub avg_profit_per_trade: U256,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub profit_rate: f64,
    pub uptime_percentage: f64,
    pub exchanges_monitored: u32,
    pub pairs_monitored: u32,
    pub last_updated: DateTime<Utc>,
}

/// 실시간 모니터링 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStatus {
    pub is_running: bool,
    pub active_exchanges: u32,
    pub failed_exchanges: u32,
    pub avg_latency_ms: f64,
    pub data_quality_score: f64,
    pub last_heartbeat: Option<Instant>,
    pub error_count: u64,
    pub last_error: Option<String>,
}

/// 가격 피드 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFeedStatus {
    pub is_active: bool,
    pub feeds_count: u32,
    pub last_update: Option<DateTime<Utc>>,
    pub update_frequency_ms: u64,
    pub missed_updates: u64,
    pub data_freshness_ms: u64,
}

/// 실행 엔진 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEngineStatus {
    pub is_running: bool,
    pub active_orders: u32,
    pub pending_orders: u32,
    pub completed_orders: u64,
    pub failed_orders: u64,
    pub avg_execution_time_ms: f64,
    pub last_execution: Option<DateTime<Utc>>,
}

/// 시스템 상태 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroArbitrageSystemStatus {
    pub is_running: bool,
    pub strategy_enabled: bool,
    pub monitoring_status: MonitoringStatus,
    pub price_feed_status: PriceFeedStatus,
    pub execution_engine_status: ExecutionEngineStatus,
    pub performance_stats: MicroArbitrageStats,
    pub risk_metrics: RiskMetrics,
    pub last_health_check: DateTime<Utc>,
}

/// 위험 메트릭
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub current_exposure: U256,
    pub daily_pnl: U256,
    pub max_drawdown: U256,
    pub var_95: U256,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub avg_win: U256,
    pub avg_loss: U256,
    pub profit_factor: f64,
}

/// 설정 옵션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroArbitrageConfig {
    pub enabled: bool,
    pub min_profit_percentage: f64,
    pub min_profit_usd: f64,
    pub execution_timeout_ms: u64,
    pub max_concurrent_trades: usize,
    pub latency_threshold_ms: u64,
    pub daily_volume_limit: U256,
    pub risk_limit_per_trade: U256,
    pub funding_mode: FundingMode,
    pub price_update_interval_ms: u64,
    pub max_flashloan_fee_bps: u32,
    pub gas_buffer_pct: f64,
    pub exchanges: Vec<ExchangeConfig>,
    pub trading_pairs: Vec<String>,
}

/// 거래소 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub name: String,
    pub enabled: bool,
    pub exchange_type: ExchangeType,
    pub api_endpoint: String,
    pub trading_pairs: Vec<String>,
    pub fee_percentage: f64,
    pub min_order_size: String,
    pub max_order_size: String,
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub passphrase: Option<String>,
}

/// 가격 임팩트 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceImpactAnalysis {
    pub symbol: String,
    pub exchange: String,
    pub current_price: Decimal,
    pub impact_percentage: f64,
    pub liquidity_depth: U256,
    pub optimal_trade_size: U256,
    pub max_safe_trade_size: U256,
    pub estimated_slippage: f64,
}

/// 경쟁 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionAnalysis {
    pub symbol: String,
    pub competitor_count: u32,
    pub avg_competitor_size: U256,
    pub competition_intensity: f64,
    pub market_share: f64,
    pub recommended_strategy: CompetitionStrategy,
}

/// 경쟁 전략
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompetitionStrategy {
    Aggressive,  // 공격적
    Conservative, // 보수적
    Adaptive,    // 적응적
    Avoid,       // 회피
}

/// 실행 우선순위
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionPriority {
    Critical,  // 최고 우선순위
    High,      // 높은 우선순위
    Medium,    // 중간 우선순위
    Low,       // 낮은 우선순위
}

impl MicroArbitrageOpportunity {
    /// 새로운 아비트리지 기회 생성
    pub fn new(
        token_symbol: String,
        buy_exchange: String,
        sell_exchange: String,
        buy_price: Decimal,
        sell_price: Decimal,
        max_amount: U256,
        confidence_score: f64,
    ) -> Self {
        let price_spread = sell_price - buy_price;
        let profit_percentage = (price_spread / buy_price).to_f64().unwrap_or(0.0);
        let expected_profit = U256::from((max_amount.as_u128() as f64 * profit_percentage) as u64);
        
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(30); // 30초 후 만료
        
        let parts: Vec<&str> = token_symbol.split('/').collect();
        let base_asset = parts.get(0).unwrap_or(&"ETH").to_string();
        let quote_asset = parts.get(1).unwrap_or(&"USDT").to_string();
        
        Self {
            id: format!("arb_{}_{}", now.timestamp_millis(), fastrand::u32(..)),
            token_symbol,
            buy_exchange,
            sell_exchange,
            buy_price,
            sell_price,
            price_spread,
            profit_percentage,
            max_amount,
            execution_window_ms: 5000, // 5초
            confidence_score,
            expected_profit,
            buy_amount: max_amount,
            base_asset,
            quote_asset,
            created_at: now,
            expires_at,
            gas_cost: U256::zero(), // 나중에 계산
            net_profit: expected_profit, // 나중에 계산
            success_probability: confidence_score,
        }
    }
    
    /// 기회가 유효한지 확인
    pub fn is_valid(&self) -> bool {
        self.expires_at > Utc::now() && 
        self.confidence_score > 0.5 && 
        self.profit_percentage > 0.0
    }
    
    /// 기회 만료까지 남은 시간 (밀리초)
    pub fn time_to_expiry_ms(&self) -> u64 {
        let now = Utc::now();
        if self.expires_at > now {
            (self.expires_at - now).num_milliseconds() as u64
        } else {
            0
        }
    }
}

impl ArbitrageExecutionResult {
    /// 새로운 실행 결과 생성
    pub fn new(opportunity_id: String) -> Self {
        Self {
            opportunity_id,
            success: false,
            transaction_hashes: Vec::new(),
            actual_profit: None,
            gas_used: U256::zero(),
            execution_time_ms: 0,
            error_message: None,
            slippage: 0.0,
            fees_paid: U256::zero(),
            created_at: Utc::now(),
        }
    }
    
    /// 성공한 실행 결과 생성
    pub fn success(
        opportunity_id: String,
        transaction_hashes: Vec<H256>,
        actual_profit: U256,
        gas_used: U256,
        execution_time_ms: u64,
        slippage: f64,
        fees_paid: U256,
    ) -> Self {
        Self {
            opportunity_id,
            success: true,
            transaction_hashes,
            actual_profit: Some(actual_profit),
            gas_used,
            execution_time_ms,
            error_message: None,
            slippage,
            fees_paid,
            created_at: Utc::now(),
        }
    }
    
    /// 실패한 실행 결과 생성
    pub fn failure(
        opportunity_id: String,
        error_message: String,
        execution_time_ms: u64,
    ) -> Self {
        Self {
            opportunity_id,
            success: false,
            transaction_hashes: Vec::new(),
            actual_profit: None,
            gas_used: U256::zero(),
            execution_time_ms,
            error_message: Some(error_message),
            slippage: 0.0,
            fees_paid: U256::zero(),
            created_at: Utc::now(),
        }
    }
}

impl Default for MicroArbitrageStats {
    fn default() -> Self {
        Self {
            total_opportunities: 0,
            executed_trades: 0,
            successful_trades: 0,
            failed_trades: 0,
            total_volume: U256::zero(),
            total_profit: U256::zero(),
            total_fees: U256::zero(),
            avg_profit_per_trade: U256::zero(),
            avg_execution_time_ms: 0.0,
            success_rate: 0.0,
            profit_rate: 0.0,
            uptime_percentage: 100.0,
            exchanges_monitored: 0,
            pairs_monitored: 0,
            last_updated: Utc::now(),
        }
    }
}

impl Default for RiskMetrics {
    fn default() -> Self {
        Self {
            current_exposure: U256::zero(),
            daily_pnl: U256::zero(),
            max_drawdown: U256::zero(),
            var_95: U256::zero(),
            sharpe_ratio: 0.0,
            win_rate: 0.0,
            avg_win: U256::zero(),
            avg_loss: U256::zero(),
            profit_factor: 0.0,
        }
    }
}