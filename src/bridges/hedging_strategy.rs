use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, Duration};
use chrono::{DateTime, Utc};
use tracing::{info, debug, warn, error};
use serde::{Serialize, Deserialize};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

use crate::types::{ChainId, CrossChainToken};
use super::transaction_monitor::MonitoredTransaction;

/// 크로스체인 가격 변동 헤징 전략 시스템
/// 
/// 브리지 처리 시간 동안의 가격 변동 리스크를 관리합니다:
/// 1. 실시간 가격 모니터링 및 변동성 추적
/// 2. 리스크 평가 및 헤징 필요성 판단
/// 3. 다양한 헤징 전략 실행 (선물, 옵션, 스왑)
/// 4. 헤징 효과 분석 및 최적화
#[derive(Debug)]
pub struct CrossChainHedgingStrategy {
    /// 활성 헤징 포지션들
    active_hedges: Arc<RwLock<HashMap<String, HedgePosition>>>,
    
    /// 헤징 히스토리
    hedge_history: Arc<RwLock<Vec<HedgePosition>>>,
    
    /// 가격 모니터링 서비스
    price_monitor: Arc<dyn PriceMonitor>,
    
    /// 헤징 실행 서비스
    hedge_executor: Arc<dyn HedgeExecutor>,
    
    /// 리스크 계산기
    risk_calculator: Arc<RiskCalculator>,
    
    /// 헤징 설정
    config: HedgingConfig,
    
    /// 실행 상태
    is_running: Arc<RwLock<bool>>,
    
    /// 알림 채널
    notification_sender: Option<mpsc::UnboundedSender<HedgingEvent>>,
}

/// 헤징 포지션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgePosition {
    /// 포지션 ID
    pub position_id: String,
    
    /// 연결된 실행 ID
    pub execution_id: String,
    
    /// 헤징 대상 정보
    pub target_info: HedgeTarget,
    
    /// 헤징 전략 타입
    pub strategy_type: HedgeStrategyType,
    
    /// 헤징 상품 정보
    pub hedge_instrument: HedgeInstrument,
    
    /// 포지션 크기
    pub position_size: Decimal,
    
    /// 진입 가격
    pub entry_price: Decimal,
    
    /// 현재 가격
    pub current_price: Option<Decimal>,
    
    /// 타겟 가격 (보호하려는 가격)
    pub target_price: Decimal,
    
    /// 손절매 가격
    pub stop_loss: Option<Decimal>,
    
    /// 익절 가격
    pub take_profit: Option<Decimal>,
    
    /// 포지션 상태
    pub status: HedgeStatus,
    
    /// 개설 시간
    pub opened_at: DateTime<Utc>,
    
    /// 만료 시간
    pub expires_at: DateTime<Utc>,
    
    /// 종료 시간
    pub closed_at: Option<DateTime<Utc>>,
    
    /// 손익 (USD)
    pub pnl_usd: Decimal,
    
    /// 수수료
    pub fees: Decimal,
    
    /// 헤징 효과성
    pub effectiveness: Option<HedgeEffectiveness>,
    
    /// 리스크 메트릭
    pub risk_metrics: RiskMetrics,
}

/// 헤징 대상
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgeTarget {
    /// 대상 토큰
    pub token: CrossChainToken,
    
    /// 노출 금액 (USD)
    pub exposure_amount: Decimal,
    
    /// 노출 기간 (초)
    pub exposure_duration: u64,
    
    /// 소스 체인
    pub source_chain: ChainId,
    
    /// 대상 체인
    pub dest_chain: ChainId,
    
    /// 예상 완료 시간
    pub expected_completion: DateTime<Utc>,
}

/// 헤징 전략 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HedgeStrategyType {
    /// 단순 헤징 (반대 포지션)
    SimpleHedge,
    
    /// 델타 헤징 (민감도 기반)
    DeltaHedge,
    
    /// 옵션 헤징 (보호 풋/콜)
    OptionHedge,
    
    /// 스왑 헤징 (토큰 스왑)
    SwapHedge,
    
    /// 복합 헤징 (여러 전략 조합)
    CompositeHedge,
    
    /// 동적 헤징 (실시간 조정)
    DynamicHedge,
}

/// 헤징 상품
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgeInstrument {
    /// 상품 타입
    pub instrument_type: InstrumentType,
    
    /// 상품 심볼/주소
    pub symbol: String,
    
    /// 거래소/플랫폼
    pub platform: String,
    
    /// 만료일 (옵션/선물의 경우)
    pub expiry: Option<DateTime<Utc>>,
    
    /// 행사가 (옵션의 경우)
    pub strike_price: Option<Decimal>,
    
    /// 계약 크기
    pub contract_size: Decimal,
    
    /// 최소 거래 단위
    pub min_trade_size: Decimal,
    
    /// 수수료율
    pub fee_rate: Decimal,
}

/// 상품 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstrumentType {
    /// 현물 (반대 포지션)
    Spot,
    
    /// 선물
    Futures,
    
    /// 콜 옵션
    CallOption,
    
    /// 풋 옵션
    PutOption,
    
    /// 스왑
    Swap,
    
    /// CFD (차액결제계약)
    CFD,
}

/// 헤징 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HedgeStatus {
    /// 계획됨
    Planned,
    
    /// 실행 중
    Executing,
    
    /// 활성 (헤징 중)
    Active,
    
    /// 조정 중
    Adjusting,
    
    /// 만료됨
    Expired,
    
    /// 종료됨
    Closed,
    
    /// 실패
    Failed,
}

/// 헤징 효과성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgeEffectiveness {
    /// 헤징 비율 (0-1)
    pub hedge_ratio: Decimal,
    
    /// 상관관계 (기초자산과 헤징 상품)
    pub correlation: Decimal,
    
    /// 베타 (시장 민감도)
    pub beta: Decimal,
    
    /// 추적 오차
    pub tracking_error: Decimal,
    
    /// 헤징 효율성 점수 (0-100)
    pub effectiveness_score: u8,
}

/// 리스크 메트릭
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    /// VaR (Value at Risk)
    pub var_95: Decimal,
    pub var_99: Decimal,
    
    /// 예상 손실 (Expected Shortfall)
    pub expected_shortfall: Decimal,
    
    /// 변동성 (일일)
    pub daily_volatility: Decimal,
    
    /// 최대 낙폭 (Max Drawdown)
    pub max_drawdown: Decimal,
    
    /// 샤프 비율
    pub sharpe_ratio: Decimal,
    
    /// 리스크 점수 (0-100)
    pub risk_score: u8,
}

/// 헤징 이벤트
#[derive(Debug, Clone)]
pub struct HedgingEvent {
    pub position_id: String,
    pub event_type: HedgingEventType,
    pub data: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// 헤징 이벤트 타입
#[derive(Debug, Clone)]
pub enum HedgingEventType {
    PositionOpened,
    PositionAdjusted,
    PositionClosed,
    RiskAlert,
    EffectivenessUpdate,
    MarketVolatilitySpike,
}

/// 헤징 설정
#[derive(Debug, Clone)]
pub struct HedgingConfig {
    /// 헤징 활성화 여부
    pub enabled: bool,
    
    /// 최소 헤징 금액 (USD)
    pub min_hedge_amount: Decimal,
    
    /// 리스크 임계값
    pub risk_thresholds: RiskThresholds,
    
    /// 헤징 전략 설정
    pub strategy_config: StrategyConfig,
    
    /// 모니터링 설정
    pub monitoring_config: MonitoringConfig,
    
    /// 수수료 한도
    pub max_fee_percent: Decimal,
    
    /// 자동 실행 여부
    pub auto_execution: bool,
}

/// 리스크 임계값
#[derive(Debug, Clone)]
pub struct RiskThresholds {
    /// 최대 허용 VaR
    pub max_var_percent: Decimal,
    
    /// 변동성 임계값
    pub volatility_threshold: Decimal,
    
    /// 상관관계 최소값
    pub min_correlation: Decimal,
    
    /// 최대 추적 오차
    pub max_tracking_error: Decimal,
    
    /// 헤징 효율성 최소값
    pub min_effectiveness: u8,
}

/// 전략 설정
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// 선호 헤징 전략
    pub preferred_strategies: Vec<HedgeStrategyType>,
    
    /// 헤징 비율 (기본값)
    pub default_hedge_ratio: Decimal,
    
    /// 리밸런싱 빈도 (초)
    pub rebalance_frequency: u64,
    
    /// 동적 조정 활성화
    pub dynamic_adjustment: bool,
    
    /// 부분 헤징 허용
    pub allow_partial_hedge: bool,
}

/// 모니터링 설정
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// 가격 모니터링 간격 (초)
    pub price_check_interval: u64,
    
    /// 리스크 평가 간격 (초)
    pub risk_assessment_interval: u64,
    
    /// 알림 임계값
    pub alert_thresholds: AlertThresholds,
    
    /// 로깅 레벨
    pub log_level: LogLevel,
}

/// 알림 임계값
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    /// 손실 알림 임계값 (%)
    pub loss_alert_percent: Decimal,
    
    /// 변동성 알림 임계값
    pub volatility_alert: Decimal,
    
    /// 상관관계 악화 임계값
    pub correlation_degradation: Decimal,
}

/// 로깅 레벨
#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// 가격 모니터링 트레이트
#[async_trait::async_trait]
pub trait PriceMonitor: Send + Sync + std::fmt::Debug {
    async fn get_current_price(&self, token: &CrossChainToken, chain: ChainId) -> Result<Decimal>;
    async fn get_price_history(&self, token: &CrossChainToken, chain: ChainId, hours: u64) -> Result<Vec<PricePoint>>;
    async fn calculate_volatility(&self, token: &CrossChainToken, chain: ChainId, period_hours: u64) -> Result<Decimal>;
    async fn get_correlation(&self, token1: &CrossChainToken, token2: &CrossChainToken, period_hours: u64) -> Result<Decimal>;
}

/// 가격 포인트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub price: Decimal,
    pub volume: Decimal,
}

/// 헤징 실행 트레이트
#[async_trait::async_trait]
pub trait HedgeExecutor: Send + Sync + std::fmt::Debug {
    async fn execute_hedge(&self, hedge_plan: &HedgePlan) -> Result<HedgeExecution>;
    async fn close_position(&self, position_id: &str) -> Result<HedgeExecution>;
    async fn adjust_position(&self, position_id: &str, adjustment: &PositionAdjustment) -> Result<HedgeExecution>;
    async fn get_available_instruments(&self, token: &CrossChainToken) -> Result<Vec<HedgeInstrument>>;
}

/// 헤징 계획
#[derive(Debug, Clone)]
pub struct HedgePlan {
    pub target: HedgeTarget,
    pub strategy_type: HedgeStrategyType,
    pub instrument: HedgeInstrument,
    pub position_size: Decimal,
    pub hedge_ratio: Decimal,
    pub duration: u64,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
}

/// 헤징 실행 결과
#[derive(Debug, Clone)]
pub struct HedgeExecution {
    pub success: bool,
    pub position_id: Option<String>,
    pub execution_price: Option<Decimal>,
    pub fees: Decimal,
    pub error_message: Option<String>,
    pub execution_time: DateTime<Utc>,
}

/// 포지션 조정
#[derive(Debug, Clone)]
pub struct PositionAdjustment {
    pub adjustment_type: AdjustmentType,
    pub amount: Decimal,
    pub reason: String,
}

/// 조정 타입
#[derive(Debug, Clone)]
pub enum AdjustmentType {
    IncreaseSize,
    DecreaseSize,
    AdjustHedgeRatio,
    UpdateStopLoss,
    UpdateTakeProfit,
}

/// 리스크 계산기
#[derive(Debug)]
pub struct RiskCalculator {
    config: RiskCalculationConfig,
}

/// 리스크 계산 설정
#[derive(Debug, Clone)]
pub struct RiskCalculationConfig {
    pub confidence_level_var: Decimal, // VaR 신뢰도 (0.95, 0.99 등)
    pub holding_period_days: u32,      // 보유 기간 (일)
    pub lookback_period_days: u32,     // 과거 데이터 기간 (일)
    pub monte_carlo_simulations: u32,  // 몬테카를로 시뮬레이션 횟수
}

impl Default for HedgingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_hedge_amount: Decimal::from_str_exact("100.0").unwrap(), // $100
            risk_thresholds: RiskThresholds {
                max_var_percent: Decimal::from_str_exact("5.0").unwrap(), // 5%
                volatility_threshold: Decimal::from_str_exact("20.0").unwrap(), // 20%
                min_correlation: Decimal::from_str_exact("0.7").unwrap(), // 70%
                max_tracking_error: Decimal::from_str_exact("2.0").unwrap(), // 2%
                min_effectiveness: 80, // 80점
            },
            strategy_config: StrategyConfig {
                preferred_strategies: vec![
                    HedgeStrategyType::SimpleHedge,
                    HedgeStrategyType::DeltaHedge,
                ],
                default_hedge_ratio: Decimal::from_str_exact("0.8").unwrap(), // 80%
                rebalance_frequency: 300, // 5분
                dynamic_adjustment: true,
                allow_partial_hedge: true,
            },
            monitoring_config: MonitoringConfig {
                price_check_interval: 30, // 30초
                risk_assessment_interval: 60, // 1분
                alert_thresholds: AlertThresholds {
                    loss_alert_percent: Decimal::from_str_exact("2.0").unwrap(), // 2%
                    volatility_alert: Decimal::from_str_exact("30.0").unwrap(), // 30%
                    correlation_degradation: Decimal::from_str_exact("0.5").unwrap(), // 50%
                },
                log_level: LogLevel::Info,
            },
            max_fee_percent: Decimal::from_str_exact("0.5").unwrap(), // 0.5%
            auto_execution: true,
        }
    }
}

impl CrossChainHedgingStrategy {
    /// 새로운 헤징 전략 생성
    pub fn new(
        price_monitor: Arc<dyn PriceMonitor>,
        hedge_executor: Arc<dyn HedgeExecutor>,
    ) -> Self {
        let risk_calculator = Arc::new(RiskCalculator::new());
        
        Self {
            active_hedges: Arc::new(RwLock::new(HashMap::new())),
            hedge_history: Arc::new(RwLock::new(Vec::new())),
            price_monitor,
            hedge_executor,
            risk_calculator,
            config: HedgingConfig::default(),
            is_running: Arc::new(RwLock::new(false)),
            notification_sender: None,
        }
    }
    
    /// 커스텀 설정으로 생성
    pub fn with_config(
        price_monitor: Arc<dyn PriceMonitor>,
        hedge_executor: Arc<dyn HedgeExecutor>,
        config: HedgingConfig,
    ) -> Self {
        let risk_calculator = Arc::new(RiskCalculator::new());
        
        Self {
            active_hedges: Arc::new(RwLock::new(HashMap::new())),
            hedge_history: Arc::new(RwLock::new(Vec::new())),
            price_monitor,
            hedge_executor,
            risk_calculator,
            config,
            is_running: Arc::new(RwLock::new(false)),
            notification_sender: None,
        }
    }
    
    /// 알림 채널 설정
    pub fn with_notifications(mut self, sender: mpsc::UnboundedSender<HedgingEvent>) -> Self {
        self.notification_sender = Some(sender);
        self
    }
    
    /// 헤징 시스템 시작
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("🛡️ 크로스체인 헤징 전략 시작");
        
        // 가격 모니터링 루프 시작
        self.start_price_monitoring_loop().await;
        
        // 리스크 평가 루프 시작
        self.start_risk_assessment_loop().await;
        
        // 포지션 관리 루프 시작
        self.start_position_management_loop().await;
        
        info!("✅ 크로스체인 헤징 전략 시작 완료");
        Ok(())
    }
    
    /// 헤징 시스템 중지
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        
        info!("🛑 크로스체인 헤징 전략 중지");
        Ok(())
    }
    
    /// 헤징 필요성 평가 및 실행
    pub async fn evaluate_and_hedge(&self, transaction: &MonitoredTransaction) -> Result<Option<HedgePosition>> {
        if !self.config.enabled {
            return Ok(None);
        }
        
        let execution_id = transaction.execution_id.clone();
        info!("🔍 헤징 필요성 평가: {}", execution_id);
        
        // 헤징 대상 분석
        let hedge_target = self.analyze_hedge_target(transaction).await?;
        
        // 리스크 평가
        let risk_assessment = self.assess_risk(&hedge_target).await?;
        
        // 헤징 필요성 판단
        if !self.should_hedge(&hedge_target, &risk_assessment).await? {
            info!("헤징 불필요: {} - 리스크 수준 낮음", execution_id);
            return Ok(None);
        }
        
        // 헤징 전략 선택
        let strategy_type = self.select_hedge_strategy(&hedge_target, &risk_assessment).await?;
        
        // 헤징 계획 수립
        let hedge_plan = self.create_hedge_plan(&hedge_target, strategy_type).await?;
        
        // 헤징 실행
        match self.execute_hedge_plan(&hedge_plan, &execution_id).await {
            Ok(hedge_position) => {
                info!("✅ 헤징 실행 완료: {} - 포지션 {}", 
                      execution_id, hedge_position.position_id);
                
                // 활성 헤지에 추가
                {
                    let mut active = self.active_hedges.write().await;
                    active.insert(hedge_position.position_id.clone(), hedge_position.clone());
                }
                
                // 알림 전송
                self.send_notification(HedgingEvent {
                    position_id: hedge_position.position_id.clone(),
                    event_type: HedgingEventType::PositionOpened,
                    data: [
                        ("execution_id".to_string(), execution_id),
                        ("strategy".to_string(), format!("{:?}", hedge_position.strategy_type)),
                        ("size".to_string(), hedge_position.position_size.to_string()),
                    ].into(),
                    timestamp: Utc::now(),
                }).await;
                
                Ok(Some(hedge_position))
            }
            Err(e) => {
                error!("❌ 헤징 실행 실패: {} - {}", execution_id, e);
                Err(e)
            }
        }
    }
    
    /// 헤징 대상 분석
    async fn analyze_hedge_target(&self, transaction: &MonitoredTransaction) -> Result<HedgeTarget> {
        let token = CrossChainToken {
            symbol: transaction.token_symbol.clone(),
            addresses: HashMap::new(), // 실제로는 체인별 주소 매핑
            decimals: 18,
        };
        
        Ok(HedgeTarget {
            token,
            exposure_amount: Decimal::from_f64(transaction.amount_usd).unwrap_or_else(|| Decimal::ZERO),
            exposure_duration: transaction.estimated_completion.signed_duration_since(Utc::now()).num_seconds() as u64,
            source_chain: transaction.source_chain.chain_id,
            dest_chain: transaction.dest_chain.chain_id,
            expected_completion: transaction.estimated_completion,
        })
    }
    
    /// 리스크 평가
    async fn assess_risk(&self, target: &HedgeTarget) -> Result<RiskAssessment> {
        // 변동성 계산
        let volatility = self.price_monitor.calculate_volatility(
            &target.token,
            target.source_chain,
            24 // 24시간 기준
        ).await?;
        
        // VaR 계산
        let var_95 = self.risk_calculator.calculate_var(
            target.exposure_amount,
            volatility,
            Decimal::from_str_exact("0.95").unwrap()
        ).await?;
        
        let var_99 = self.risk_calculator.calculate_var(
            target.exposure_amount,
            volatility,
            Decimal::from_str_exact("0.99").unwrap()
        ).await?;
        
        // 리스크 점수 계산
        let risk_score = self.calculate_risk_score(volatility, var_95, target.exposure_duration).await?;
        
        Ok(RiskAssessment {
            volatility,
            var_95,
            var_99,
            risk_score,
            recommended_hedge_ratio: self.calculate_recommended_hedge_ratio(volatility).await?,
        })
    }
    
    /// 헤징 필요성 판단
    async fn should_hedge(&self, target: &HedgeTarget, risk: &RiskAssessment) -> Result<bool> {
        // 최소 헤징 금액 체크
        if target.exposure_amount < self.config.min_hedge_amount {
            return Ok(false);
        }
        
        // 리스크 임계값 체크
        if risk.volatility > self.config.risk_thresholds.volatility_threshold {
            return Ok(true);
        }
        
        if risk.var_95 > target.exposure_amount * self.config.risk_thresholds.max_var_percent / Decimal::from(100) {
            return Ok(true);
        }
        
        // 노출 기간 체크 (긴 기간일수록 헤징 필요)
        if target.exposure_duration > 1800 { // 30분 이상
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// 헤징 전략 선택
    async fn select_hedge_strategy(&self, _target: &HedgeTarget, risk: &RiskAssessment) -> Result<HedgeStrategyType> {
        // 리스크 수준에 따른 전략 선택
        if risk.risk_score >= 80 {
            // 고위험: 복합 헤징
            Ok(HedgeStrategyType::CompositeHedge)
        } else if risk.risk_score >= 60 {
            // 중위험: 델타 헤징
            Ok(HedgeStrategyType::DeltaHedge)
        } else {
            // 저위험: 단순 헤징
            Ok(HedgeStrategyType::SimpleHedge)
        }
    }
    
    /// 헤징 계획 수립
    async fn create_hedge_plan(&self, target: &HedgeTarget, strategy_type: HedgeStrategyType) -> Result<HedgePlan> {
        // 사용 가능한 헤징 상품 조회
        let available_instruments = self.hedge_executor.get_available_instruments(&target.token).await?;
        
        // 최적 상품 선택 (수수료, 유동성 고려)
        let instrument = available_instruments.into_iter()
            .min_by_key(|i| (i.fee_rate * Decimal::from(1000)).to_u32().unwrap_or(u32::MAX))
            .ok_or_else(|| anyhow!("사용 가능한 헤징 상품이 없습니다"))?;
        
        // 헤징 비율 계산
        let hedge_ratio = match strategy_type {
            HedgeStrategyType::SimpleHedge => self.config.strategy_config.default_hedge_ratio,
            HedgeStrategyType::DeltaHedge => self.calculate_delta_hedge_ratio(target).await?,
            _ => self.config.strategy_config.default_hedge_ratio,
        };
        
        // 포지션 크기 계산
        let position_size = target.exposure_amount * hedge_ratio / instrument.contract_size;
        
        Ok(HedgePlan {
            target: target.clone(),
            strategy_type,
            instrument,
            position_size,
            hedge_ratio,
            duration: target.exposure_duration,
            stop_loss: None, // 기본적으로 손절매 없음 (완전 헤징)
            take_profit: None,
        })
    }
    
    /// 헤징 계획 실행
    async fn execute_hedge_plan(&self, plan: &HedgePlan, execution_id: &str) -> Result<HedgePosition> {
        info!("🚀 헤징 실행: {} via {:?}", execution_id, plan.strategy_type);
        
        // 헤징 실행
        let execution_result = self.hedge_executor.execute_hedge(plan).await?;
        
        if !execution_result.success {
            return Err(anyhow!("헤징 실행 실패: {}", 
                execution_result.error_message.unwrap_or_default()));
        }
        
        let position_id = execution_result.position_id
            .ok_or_else(|| anyhow!("포지션 ID가 반환되지 않았습니다"))?;
        
        let entry_price = execution_result.execution_price
            .ok_or_else(|| anyhow!("실행 가격이 반환되지 않았습니다"))?;
        
        // 현재 가격 조회
        let current_price = self.price_monitor.get_current_price(
            &plan.target.token,
            plan.target.source_chain
        ).await?;
        
        // 리스크 메트릭 계산
        let risk_metrics = self.risk_calculator.calculate_position_risk(
            &plan.target,
            plan.position_size,
            entry_price
        ).await?;
        
        Ok(HedgePosition {
            position_id,
            execution_id: execution_id.to_string(),
            target_info: plan.target.clone(),
            strategy_type: plan.strategy_type.clone(),
            hedge_instrument: plan.instrument.clone(),
            position_size: plan.position_size,
            entry_price,
            current_price: Some(current_price),
            target_price: current_price, // 현재 가격을 보호 목표로 설정
            stop_loss: plan.stop_loss,
            take_profit: plan.take_profit,
            status: HedgeStatus::Active,
            opened_at: Utc::now(),
            expires_at: plan.target.expected_completion,
            closed_at: None,
            pnl_usd: Decimal::ZERO,
            fees: execution_result.fees,
            effectiveness: None, // 시간이 지나면 계산
            risk_metrics,
        })
    }
    
    /// 포지션 종료
    pub async fn close_position(&self, position_id: &str, reason: &str) -> Result<()> {
        info!("🔚 포지션 종료: {} - {}", position_id, reason);
        
        let mut active = self.active_hedges.write().await;
        if let Some(mut position) = active.remove(position_id) {
            // 포지션 종료 실행
            let close_result = self.hedge_executor.close_position(position_id).await?;
            
            if close_result.success {
                // 최종 손익 계산
                if let Some(close_price) = close_result.execution_price {
                    position.current_price = Some(close_price);
                    position.pnl_usd = self.calculate_pnl(&position, close_price).await?;
                }
                
                position.status = HedgeStatus::Closed;
                position.closed_at = Some(Utc::now());
                position.fees += close_result.fees;
                
                // 헤징 효과성 계산
                position.effectiveness = Some(self.calculate_hedge_effectiveness(&position).await?);
                
                // 히스토리에 추가
                {
                    let mut history = self.hedge_history.write().await;
                    history.push(position.clone());
                    
                    // 최대 1000개 히스토리 유지
                    if history.len() > 1000 {
                        history.remove(0);
                    }
                }
                
                info!("✅ 포지션 종료 완료: {} - 손익: ${:.2}", 
                      position_id, position.pnl_usd);
                
                // 알림 전송
                self.send_notification(HedgingEvent {
                    position_id: position_id.to_string(),
                    event_type: HedgingEventType::PositionClosed,
                    data: [
                        ("reason".to_string(), reason.to_string()),
                        ("pnl".to_string(), position.pnl_usd.to_string()),
                    ].into(),
                    timestamp: Utc::now(),
                }).await;
            } else {
                error!("❌ 포지션 종료 실패: {} - {}", 
                       position_id, close_result.error_message.unwrap_or_default());
                
                // 실패한 포지션은 다시 활성 목록에 추가
                active.insert(position_id.to_string(), position);
            }
        }
        
        Ok(())
    }
    
    /// 가격 모니터링 루프
    async fn start_price_monitoring_loop(&self) {
        let active_hedges = Arc::clone(&self.active_hedges);
        let price_monitor = Arc::clone(&self.price_monitor);
        let is_running = Arc::clone(&self.is_running);
        let check_interval = self.config.monitoring_config.price_check_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(check_interval));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let active = active_hedges.read().await;
                for position in active.values() {
                    // 현재 가격 조회
                    if let Ok(current_price) = price_monitor.get_current_price(
                        &position.target_info.token,
                        position.target_info.source_chain
                    ).await {
                        // 가격 변동 체크 및 알림
                        let price_change = if let Some(prev_price) = position.current_price {
                            ((current_price - prev_price) / prev_price * Decimal::from(100)).abs()
                        } else {
                            Decimal::ZERO
                        };
                        
                        if price_change > Decimal::from_str_exact("5.0").unwrap() { // 5% 이상 변동
                            debug!("💥 가격 급변동 감지: {} - {:.2}% 변동", 
                                   position.position_id, price_change);
                        }
                    }
                }
            }
        });
    }
    
    /// 리스크 평가 루프
    async fn start_risk_assessment_loop(&self) {
        let active_hedges = Arc::clone(&self.active_hedges);
        let risk_calculator = Arc::clone(&self.risk_calculator);
        let is_running = Arc::clone(&self.is_running);
        let assessment_interval = self.config.monitoring_config.risk_assessment_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(assessment_interval));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let mut active = active_hedges.write().await;
                for position in active.values_mut() {
                    // 리스크 메트릭 업데이트
                    if let Ok(updated_metrics) = risk_calculator.calculate_position_risk(
                        &position.target_info,
                        position.position_size,
                        position.entry_price
                    ).await {
                        position.risk_metrics = updated_metrics;
                        
                        // 리스크 알림 체크
                        if position.risk_metrics.risk_score >= 80 {
                            warn!("⚠️ 고위험 포지션 감지: {} - 리스크 점수: {}", 
                                  position.position_id, position.risk_metrics.risk_score);
                        }
                    }
                }
            }
        });
    }
    
    /// 포지션 관리 루프
    async fn start_position_management_loop(&self) {
        let active_hedges = Arc::clone(&self.active_hedges);
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 5분마다
            
            while *is_running.read().await {
                interval.tick().await;
                
                let active = active_hedges.read().await;
                let now = Utc::now();
                
                for position in active.values() {
                    // 만료된 포지션 체크
                    if now > position.expires_at && position.status == HedgeStatus::Active {
                        info!("⏰ 포지션 만료: {}", position.position_id);
                        // 실제로는 close_position 호출해야 하지만 여기서는 로그만
                    }
                    
                    // 손절매/익절 체크
                    if let Some(current_price) = position.current_price {
                        if let Some(stop_loss) = position.stop_loss {
                            if current_price <= stop_loss {
                                warn!("🛑 손절매 트리거: {} - 현재가: {}, 손절가: {}", 
                                      position.position_id, current_price, stop_loss);
                            }
                        }
                        
                        if let Some(take_profit) = position.take_profit {
                            if current_price >= take_profit {
                                info!("🎯 익절 트리거: {} - 현재가: {}, 익절가: {}", 
                                      position.position_id, current_price, take_profit);
                            }
                        }
                    }
                }
            }
        });
    }
    
    /// 알림 전송
    async fn send_notification(&self, event: HedgingEvent) {
        if let Some(ref sender) = self.notification_sender {
            if let Err(e) = sender.send(event) {
                error!("헤징 알림 전송 실패: {}", e);
            }
        }
    }
    
    /// 활성 포지션 조회
    pub async fn get_active_positions(&self) -> Vec<HedgePosition> {
        let active = self.active_hedges.read().await;
        active.values().cloned().collect()
    }
    
    /// 헤징 히스토리 조회
    pub async fn get_hedge_history(&self, limit: usize) -> Vec<HedgePosition> {
        let history = self.hedge_history.read().await;
        let start = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };
        history[start..].to_vec()
    }
    
    /// 헤징 성과 요약
    pub async fn get_performance_summary(&self) -> HedgingPerformanceSummary {
        let history = self.hedge_history.read().await;
        
        if history.is_empty() {
            return HedgingPerformanceSummary::default();
        }
        
        let total_positions = history.len();
        let profitable_positions = history.iter()
            .filter(|p| p.pnl_usd > Decimal::ZERO)
            .count();
        
        let total_pnl: Decimal = history.iter()
            .map(|p| p.pnl_usd)
            .sum();
        
        let total_fees: Decimal = history.iter()
            .map(|p| p.fees)
            .sum();
        
        let avg_effectiveness: f64 = history.iter()
            .filter_map(|p| p.effectiveness.as_ref())
            .map(|e| e.effectiveness_score as f64)
            .sum::<f64>() / history.len() as f64;
        
        HedgingPerformanceSummary {
            total_positions,
            profitable_positions,
            success_rate: profitable_positions as f64 / total_positions as f64,
            total_pnl,
            total_fees,
            net_pnl: total_pnl - total_fees,
            avg_effectiveness_score: avg_effectiveness as u8,
        }
    }
    
    // Helper methods (Mock implementations)
    
    async fn calculate_risk_score(&self, volatility: Decimal, var: Decimal, duration: u64) -> Result<u8> {
        let mut score = 0u8;
        
        // 변동성 기준 점수 (0-40점)
        if volatility > Decimal::from_str_exact("30.0").unwrap() {
            score += 40;
        } else if volatility > Decimal::from_str_exact("20.0").unwrap() {
            score += 30;
        } else if volatility > Decimal::from_str_exact("10.0").unwrap() {
            score += 20;
        } else {
            score += 10;
        }
        
        // VaR 기준 점수 (0-30점)
        if var > Decimal::from_str_exact("100.0").unwrap() {
            score += 30;
        } else if var > Decimal::from_str_exact("50.0").unwrap() {
            score += 20;
        } else {
            score += 10;
        }
        
        // 기간 기준 점수 (0-30점)
        if duration > 3600 { // 1시간 이상
            score += 30;
        } else if duration > 1800 { // 30분 이상
            score += 20;
        } else {
            score += 10;
        }
        
        Ok(score.min(100))
    }
    
    async fn calculate_recommended_hedge_ratio(&self, volatility: Decimal) -> Result<Decimal> {
        // 변동성에 따른 헤징 비율 권장
        if volatility > Decimal::from_str_exact("30.0").unwrap() {
            Ok(Decimal::from_str_exact("0.95").unwrap()) // 95%
        } else if volatility > Decimal::from_str_exact("20.0").unwrap() {
            Ok(Decimal::from_str_exact("0.80").unwrap()) // 80%
        } else if volatility > Decimal::from_str_exact("10.0").unwrap() {
            Ok(Decimal::from_str_exact("0.60").unwrap()) // 60%
        } else {
            Ok(Decimal::from_str_exact("0.40").unwrap()) // 40%
        }
    }
    
    async fn calculate_delta_hedge_ratio(&self, _target: &HedgeTarget) -> Result<Decimal> {
        // Mock 델타 헤징 비율 계산
        // 실제로는 옵션 그릭스를 사용하여 계산
        Ok(Decimal::from_str_exact("0.75").unwrap())
    }
    
    async fn calculate_pnl(&self, position: &HedgePosition, close_price: Decimal) -> Result<Decimal> {
        let price_diff = close_price - position.entry_price;
        let pnl = price_diff * position.position_size;
        Ok(pnl)
    }
    
    async fn calculate_hedge_effectiveness(&self, _position: &HedgePosition) -> Result<HedgeEffectiveness> {
        // Mock 헤징 효과성 계산
        Ok(HedgeEffectiveness {
            hedge_ratio: Decimal::from_str_exact("0.85").unwrap(),
            correlation: Decimal::from_str_exact("0.92").unwrap(),
            beta: Decimal::from_str_exact("1.05").unwrap(),
            tracking_error: Decimal::from_str_exact("1.2").unwrap(),
            effectiveness_score: 88,
        })
    }
}

/// 리스크 평가 결과
#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub volatility: Decimal,
    pub var_95: Decimal,
    pub var_99: Decimal,
    pub risk_score: u8,
    pub recommended_hedge_ratio: Decimal,
}

/// 헤징 성과 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgingPerformanceSummary {
    pub total_positions: usize,
    pub profitable_positions: usize,
    pub success_rate: f64,
    pub total_pnl: Decimal,
    pub total_fees: Decimal,
    pub net_pnl: Decimal,
    pub avg_effectiveness_score: u8,
}

impl Default for HedgingPerformanceSummary {
    fn default() -> Self {
        Self {
            total_positions: 0,
            profitable_positions: 0,
            success_rate: 0.0,
            total_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            net_pnl: Decimal::ZERO,
            avg_effectiveness_score: 0,
        }
    }
}

impl RiskCalculator {
    fn new() -> Self {
        Self {
            config: RiskCalculationConfig {
                confidence_level_var: Decimal::from_str_exact("0.95").unwrap(),
                holding_period_days: 1,
                lookback_period_days: 30,
                monte_carlo_simulations: 10000,
            },
        }
    }
    
    async fn calculate_var(&self, exposure: Decimal, volatility: Decimal, confidence: Decimal) -> Result<Decimal> {
        // Mock VaR 계산 (정규분포 가정)
        let z_score = if confidence == Decimal::from_str_exact("0.95").unwrap() {
            Decimal::from_str_exact("1.645").unwrap() // 95% 신뢰도
        } else {
            Decimal::from_str_exact("2.326").unwrap() // 99% 신뢰도
        };
        
        let var = exposure * volatility * z_score / Decimal::from(100);
        Ok(var)
    }
    
    async fn calculate_position_risk(&self, _target: &HedgeTarget, position_size: Decimal, entry_price: Decimal) -> Result<RiskMetrics> {
        // Mock 리스크 메트릭 계산
        let exposure = position_size * entry_price;
        let daily_volatility = Decimal::from_str_exact("2.5").unwrap(); // 2.5%
        
        let var_95 = self.calculate_var(exposure, daily_volatility * Decimal::from_str_exact("1.645").unwrap(), Decimal::from_str_exact("0.95").unwrap()).await?;
        let var_99 = self.calculate_var(exposure, daily_volatility * Decimal::from_str_exact("2.326").unwrap(), Decimal::from_str_exact("0.99").unwrap()).await?;
        
        Ok(RiskMetrics {
            var_95,
            var_99,
            expected_shortfall: var_99 * Decimal::from_str_exact("1.2").unwrap(),
            daily_volatility,
            max_drawdown: exposure * Decimal::from_str_exact("0.05").unwrap(), // 5%
            sharpe_ratio: Decimal::from_str_exact("1.2").unwrap(),
            risk_score: 65,
        })
    }
}