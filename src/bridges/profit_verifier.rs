use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use tracing::{info, error};
use serde::{Serialize, Deserialize};
use alloy::primitives::U256;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::types::{ChainId, BridgeProtocol, CrossChainToken};
use super::transaction_monitor::{MonitoredTransaction, TransactionStatus};

/// 크로스체인 수익 실현 검증 시스템
/// 
/// 크로스체인 아비트래지 거래의 실제 수익을 검증하고 분석합니다:
/// 1. 예상 수익 vs 실제 수익 비교
/// 2. 숨겨진 비용 및 슬리피지 추적
/// 3. 세금 및 규제 고려사항
/// 4. 수익성 분석 및 최적화 제안
#[derive(Debug)]
pub struct CrossChainProfitVerifier {
    /// 검증 완료된 거래들
    verified_trades: Arc<RwLock<HashMap<String, VerifiedTrade>>>,
    
    /// 진행 중인 검증 작업들
    pending_verifications: Arc<RwLock<HashMap<String, PendingVerification>>>,
    
    /// 토큰 가격 조회 서비스
    price_oracle: Arc<dyn PriceOracle>,
    
    /// 체인별 가스 가격 조회 서비스
    gas_oracle: Arc<dyn GasOracle>,
    
    /// 검증 설정
    config: VerificationConfig,
    
    /// 수익성 임계값
    profitability_thresholds: ProfitabilityThresholds,
    
    /// 세금 계산기
    tax_calculator: Arc<TaxCalculator>,
}

/// 검증된 거래
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedTrade {
    /// 실행 ID
    pub execution_id: String,
    
    /// 브리지 프로토콜
    pub bridge_protocol: BridgeProtocol,
    
    /// 거래 기본 정보
    pub trade_info: TradeInfo,
    
    /// 예상 수익 분석
    pub expected_profit: ProfitAnalysis,
    
    /// 실제 수익 분석
    pub actual_profit: ProfitAnalysis,
    
    /// 수익 차이 분석
    pub variance_analysis: VarianceAnalysis,
    
    /// 세금 계산 결과
    pub tax_analysis: TaxAnalysis,
    
    /// 최종 순수익
    pub net_profit: NetProfitSummary,
    
    /// 수익성 평가
    pub profitability_rating: ProfitabilityRating,
    
    /// 검증 시간
    pub verified_at: DateTime<Utc>,
    
    /// 검증 상태
    pub verification_status: VerificationStatus,
    
    /// 권장사항
    pub recommendations: Vec<Recommendation>,
}

/// 거래 기본 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInfo {
    pub source_chain: ChainId,
    pub dest_chain: ChainId,
    pub token: CrossChainToken,
    pub amount_in: U256,
    pub amount_out: U256,
    pub amount_in_usd: Decimal,
    pub amount_out_usd: Decimal,
    pub execution_time: u64, // 실제 실행 시간 (초)
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// 수익 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitAnalysis {
    /// 총 수익 (토큰 단위)
    pub gross_profit_tokens: Decimal,
    
    /// 총 수익 (USD)
    pub gross_profit_usd: Decimal,
    
    /// 비용 분석
    pub cost_breakdown: CostBreakdown,
    
    /// 순수익 (USD)
    pub net_profit_usd: Decimal,
    
    /// 수익률 (%)
    pub profit_margin_percent: Decimal,
    
    /// ROI (%)
    pub roi_percent: Decimal,
    
    /// 시간당 수익 (USD/hour)
    pub profit_per_hour: Decimal,
}

/// 비용 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// 브리지 수수료
    pub bridge_fees: Decimal,
    
    /// 소스 체인 가스비
    pub source_gas_fees: Decimal,
    
    /// 대상 체인 가스비
    pub dest_gas_fees: Decimal,
    
    /// DEX 트레이딩 수수료
    pub dex_fees: Decimal,
    
    /// 슬리피지 비용
    pub slippage_cost: Decimal,
    
    /// 기회비용 (가격 변동)
    pub opportunity_cost: Decimal,
    
    /// 기타 비용
    pub miscellaneous_costs: Decimal,
    
    /// 총 비용
    pub total_costs: Decimal,
}

/// 수익 차이 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarianceAnalysis {
    /// 수익 차이 (USD)
    pub profit_variance_usd: Decimal,
    
    /// 수익 차이 비율 (%)
    pub profit_variance_percent: Decimal,
    
    /// 비용 차이 분석
    pub cost_variance: CostVariance,
    
    /// 가격 변동 영향
    pub price_impact: PriceImpact,
    
    /// 시간 영향 분석
    pub time_impact: TimeImpact,
    
    /// 차이 원인 분석
    pub variance_causes: Vec<VarianceCause>,
}

/// 비용 차이
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostVariance {
    pub bridge_fee_variance: Decimal,
    pub gas_fee_variance: Decimal,
    pub slippage_variance: Decimal,
    pub total_variance: Decimal,
}

/// 가격 영향
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceImpact {
    pub source_price_change: Decimal,
    pub dest_price_change: Decimal,
    pub total_price_impact: Decimal,
    pub market_volatility: Decimal,
}

/// 시간 영향
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeImpact {
    pub expected_duration: u64,
    pub actual_duration: u64,
    pub duration_variance: i64,
    pub time_cost_impact: Decimal,
}

/// 차이 원인
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarianceCause {
    pub category: VarianceCategory,
    pub description: String,
    pub impact_usd: Decimal,
    pub impact_percent: Decimal,
    pub severity: VarianceSeverity,
    pub mitigation_suggestion: String,
}

/// 차이 카테고리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VarianceCategory {
    BridgeFees,
    GasFees,
    Slippage,
    PriceMovement,
    TimingDelay,
    MarketConditions,
    TechnicalIssues,
    ExchangeRates,
}

/// 차이 심각도
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VarianceSeverity {
    Low,    // <1% 영향
    Medium, // 1-5% 영향
    High,   // 5-10% 영향
    Critical, // >10% 영향
}

/// 세금 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxAnalysis {
    /// 관할 지역
    pub jurisdiction: String,
    
    /// 과세 대상 이벤트들
    pub taxable_events: Vec<TaxableEvent>,
    
    /// 총 과세 소득
    pub total_taxable_income: Decimal,
    
    /// 예상 세금
    pub estimated_tax: Decimal,
    
    /// 세후 순수익
    pub after_tax_profit: Decimal,
    
    /// 세금 최적화 제안
    pub tax_optimization_suggestions: Vec<String>,
}

/// 과세 이벤트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxableEvent {
    pub event_type: TaxEventType,
    pub description: String,
    pub taxable_amount: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
    pub occurred_at: DateTime<Utc>,
}

/// 세금 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum TaxEventType {
    Trading,        // 거래 소득
    ArbitrageProfit, // 차익거래 수익
    BridgeReward,   // 브리지 보상
    GasFeeDeduction, // 가스비 공제
}

/// 순수익 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetProfitSummary {
    /// 총 수익 (세전)
    pub gross_profit: Decimal,
    
    /// 총 비용
    pub total_costs: Decimal,
    
    /// 세전 순수익
    pub pre_tax_profit: Decimal,
    
    /// 세금
    pub tax_amount: Decimal,
    
    /// 최종 순수익 (세후)
    pub final_net_profit: Decimal,
    
    /// 최종 수익률 (%)
    pub final_profit_margin: Decimal,
    
    /// 연환산 수익률 (APY)
    pub annualized_return: Decimal,
}

/// 수익성 평가
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitabilityRating {
    /// 전체 점수 (0-100)
    pub overall_score: u8,
    
    /// 수익성 등급
    pub grade: ProfitabilityGrade,
    
    /// 리스크 조정 수익률
    pub risk_adjusted_return: Decimal,
    
    /// 벤치마크 대비 성과
    pub benchmark_comparison: BenchmarkComparison,
    
    /// 개선 포인트
    pub improvement_areas: Vec<ImprovementArea>,
}

/// 수익성 등급
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProfitabilityGrade {
    Excellent, // 90-100점
    Good,      // 70-89점
    Average,   // 50-69점
    Poor,      // 30-49점
    Terrible,  // 0-29점
}

/// 벤치마크 비교
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub market_average_return: Decimal,
    pub outperformance: Decimal,
    pub percentile_ranking: u8, // 상위 몇 퍼센트
}

/// 개선 영역
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementArea {
    pub category: String,
    pub current_performance: Decimal,
    pub target_performance: Decimal,
    pub potential_improvement: Decimal,
    pub action_items: Vec<String>,
}

/// 권장사항
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: RecommendationCategory,
    pub title: String,
    pub description: String,
    pub priority: RecommendationPriority,
    pub potential_impact: Decimal,
    pub implementation_effort: ImplementationEffort,
    pub specific_actions: Vec<String>,
}

/// 권장사항 카테고리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    CostOptimization,
    TimingImprovement,
    BridgeSelection,
    RiskManagement,
    TaxOptimization,
    ProcessAutomation,
}

/// 권장사항 우선순위
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// 구현 노력도
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,    // 1-2일
    Medium, // 1주일
    High,   // 1개월
    VeryHigh, // 3개월+
}

/// 진행 중인 검증
#[derive(Debug, Clone)]
pub struct PendingVerification {
    pub execution_id: String,
    pub transaction: MonitoredTransaction,
    pub started_at: DateTime<Utc>,
    pub expected_completion: DateTime<Utc>,
    pub verification_steps: Vec<VerificationStep>,
    pub current_step: usize,
}

/// 검증 단계
#[derive(Debug, Clone)]
pub struct VerificationStep {
    pub step_name: String,
    pub description: String,
    pub is_completed: bool,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// 검증 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    PartiallyCompleted,
}

/// 검증 설정
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    pub enable_real_time_verification: bool,
    pub verification_timeout: u64,
    pub price_tolerance_percent: Decimal,
    pub cost_tolerance_percent: Decimal,
    pub minimum_profit_threshold: Decimal,
    pub tax_calculation_enabled: bool,
    pub benchmark_comparison_enabled: bool,
}

/// 수익성 임계값
#[derive(Debug, Clone)]
pub struct ProfitabilityThresholds {
    pub minimum_profit_usd: Decimal,
    pub minimum_profit_margin: Decimal,
    pub minimum_roi: Decimal,
    pub maximum_risk_score: Decimal,
    pub minimum_profitability_score: u8,
}

/// 가격 오라클 트레이트
#[async_trait::async_trait]
pub trait PriceOracle: Send + Sync + std::fmt::Debug {
    async fn get_token_price_usd(&self, token: &CrossChainToken, chain: ChainId, timestamp: DateTime<Utc>) -> Result<Decimal>;
    async fn get_historical_volatility(&self, token: &CrossChainToken, period_hours: u64) -> Result<Decimal>;
}

/// 가스 오라클 트레이트
#[async_trait::async_trait]
pub trait GasOracle: Send + Sync + std::fmt::Debug {
    async fn get_gas_price(&self, chain: ChainId, timestamp: DateTime<Utc>) -> Result<Decimal>;
    async fn estimate_transaction_cost(&self, chain: ChainId, tx_type: &str) -> Result<Decimal>;
}

/// 세금 계산기
#[derive(Debug)]
pub struct TaxCalculator {
    jurisdiction: String,
    tax_rates: HashMap<TaxEventType, Decimal>,
    deduction_rules: Vec<DeductionRule>,
}

/// 공제 규칙
#[derive(Debug, Clone)]
pub struct DeductionRule {
    pub name: String,
    pub applies_to: Vec<TaxEventType>,
    pub deduction_rate: Decimal,
    pub max_deduction: Option<Decimal>,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            enable_real_time_verification: true,
            verification_timeout: 300, // 5분
            price_tolerance_percent: Decimal::from_str_exact("2.0").unwrap(), // 2%
            cost_tolerance_percent: Decimal::from_str_exact("5.0").unwrap(),  // 5%
            minimum_profit_threshold: Decimal::from_str_exact("10.0").unwrap(), // $10
            tax_calculation_enabled: true,
            benchmark_comparison_enabled: true,
        }
    }
}

impl Default for ProfitabilityThresholds {
    fn default() -> Self {
        Self {
            minimum_profit_usd: Decimal::from_str_exact("5.0").unwrap(),   // $5
            minimum_profit_margin: Decimal::from_str_exact("0.5").unwrap(), // 0.5%
            minimum_roi: Decimal::from_str_exact("1.0").unwrap(),          // 1%
            maximum_risk_score: Decimal::from_str_exact("0.7").unwrap(),   // 70%
            minimum_profitability_score: 50,                               // 50점
        }
    }
}

impl CrossChainProfitVerifier {
    /// 새로운 수익 검증기 생성
    pub fn new(
        price_oracle: Arc<dyn PriceOracle>,
        gas_oracle: Arc<dyn GasOracle>,
    ) -> Self {
        let tax_calculator = Arc::new(TaxCalculator::new("US".to_string()));
        
        Self {
            verified_trades: Arc::new(RwLock::new(HashMap::new())),
            pending_verifications: Arc::new(RwLock::new(HashMap::new())),
            price_oracle,
            gas_oracle,
            config: VerificationConfig::default(),
            profitability_thresholds: ProfitabilityThresholds::default(),
            tax_calculator,
        }
    }
    
    /// 커스텀 설정으로 생성
    pub fn with_config(
        price_oracle: Arc<dyn PriceOracle>,
        gas_oracle: Arc<dyn GasOracle>,
        config: VerificationConfig,
        thresholds: ProfitabilityThresholds,
        jurisdiction: String,
    ) -> Self {
        let tax_calculator = Arc::new(TaxCalculator::new(jurisdiction));
        
        Self {
            verified_trades: Arc::new(RwLock::new(HashMap::new())),
            pending_verifications: Arc::new(RwLock::new(HashMap::new())),
            price_oracle,
            gas_oracle,
            config,
            profitability_thresholds: thresholds,
            tax_calculator,
        }
    }
    
    /// 거래 수익 검증 시작
    pub async fn start_verification(&self, transaction: MonitoredTransaction) -> Result<()> {
        if transaction.status != TransactionStatus::DestConfirmed {
            return Err(anyhow!("거래가 완료되지 않았습니다: {:?}", transaction.status));
        }
        
        let execution_id = transaction.execution_id.clone();
        info!("💰 수익 검증 시작: {}", execution_id);
        
        let verification_steps = vec![
            VerificationStep {
                step_name: "price_collection".to_string(),
                description: "토큰 가격 정보 수집".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
            VerificationStep {
                step_name: "cost_analysis".to_string(),
                description: "비용 분석 및 계산".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
            VerificationStep {
                step_name: "profit_calculation".to_string(),
                description: "수익 계산 및 분석".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
            VerificationStep {
                step_name: "variance_analysis".to_string(),
                description: "예상 대비 실제 차이 분석".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
            VerificationStep {
                step_name: "tax_calculation".to_string(),
                description: "세금 계산".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
            VerificationStep {
                step_name: "profitability_rating".to_string(),
                description: "수익성 평가".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
            VerificationStep {
                step_name: "recommendations".to_string(),
                description: "권장사항 생성".to_string(),
                is_completed: false,
                result: None,
                error: None,
            },
        ];
        
        let pending_verification = PendingVerification {
            execution_id: execution_id.clone(),
            transaction,
            started_at: Utc::now(),
            expected_completion: Utc::now() + ChronoDuration::seconds(self.config.verification_timeout as i64),
            verification_steps,
            current_step: 0,
        };
        
        {
            let mut pending = self.pending_verifications.write().await;
            pending.insert(execution_id.clone(), pending_verification);
        }
        
        // 비동기로 검증 수행
        let verifier = self.clone();
        tokio::spawn(async move {
            if let Err(e) = verifier.perform_verification(execution_id).await {
                error!("수익 검증 실패: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// 실제 검증 수행
    async fn perform_verification(&self, execution_id: String) -> Result<()> {
        let verified_trade = {
            let mut pending = self.pending_verifications.write().await;
            let verification = pending.get_mut(&execution_id)
                .ok_or_else(|| anyhow!("검증 작업을 찾을 수 없음: {}", execution_id))?;
            
            // 1단계: 가격 정보 수집
            let trade_info = self.extract_trade_info(&verification.transaction).await?;
            verification.verification_steps[0].is_completed = true;
            verification.current_step = 1;
            
            // 2단계: 비용 분석
            let actual_costs = self.analyze_actual_costs(&verification.transaction, &trade_info).await?;
            verification.verification_steps[1].is_completed = true;
            verification.current_step = 2;
            
            // 3단계: 수익 계산
            let expected_profit = self.calculate_expected_profit(&trade_info).await?;
            let actual_profit = self.calculate_actual_profit(&trade_info, &actual_costs).await?;
            verification.verification_steps[2].is_completed = true;
            verification.current_step = 3;
            
            // 4단계: 차이 분석
            let variance_analysis = self.analyze_variance(&expected_profit, &actual_profit, &trade_info).await?;
            verification.verification_steps[3].is_completed = true;
            verification.current_step = 4;
            
            // 5단계: 세금 계산
            let tax_analysis = if self.config.tax_calculation_enabled {
                self.calculate_tax(&actual_profit, &trade_info).await?
            } else {
                TaxAnalysis::default()
            };
            verification.verification_steps[4].is_completed = true;
            verification.current_step = 5;
            
            // 6단계: 수익성 평가
            let net_profit = self.calculate_net_profit(&actual_profit, &tax_analysis).await?;
            let profitability_rating = self.rate_profitability(&net_profit, &variance_analysis, &trade_info).await?;
            verification.verification_steps[5].is_completed = true;
            verification.current_step = 6;
            
            // 7단계: 권장사항 생성
            let recommendations = self.generate_recommendations(&variance_analysis, &profitability_rating, &trade_info).await?;
            verification.verification_steps[6].is_completed = true;
            verification.current_step = 7;
            
            VerifiedTrade {
                execution_id: execution_id.clone(),
                bridge_protocol: verification.transaction.bridge_protocol.clone(),
                trade_info,
                expected_profit,
                actual_profit,
                variance_analysis,
                tax_analysis,
                net_profit,
                profitability_rating,
                verified_at: Utc::now(),
                verification_status: VerificationStatus::Completed,
                recommendations,
            }
        };
        
        // 검증 완료된 거래 저장
        {
            let mut verified = self.verified_trades.write().await;
            verified.insert(execution_id.clone(), verified_trade.clone());
        }
        
        // 진행 중 목록에서 제거
        {
            let mut pending = self.pending_verifications.write().await;
            pending.remove(&execution_id);
        }
        
        info!("✅ 수익 검증 완료: {} - 최종 수익: ${:.2}", 
              execution_id, verified_trade.net_profit.final_net_profit);
        
        Ok(())
    }
    
    /// 거래 정보 추출
    async fn extract_trade_info(&self, transaction: &MonitoredTransaction) -> Result<TradeInfo> {
        // Mock 구현 - 실제로는 트랜잭션 데이터에서 추출
        let token = CrossChainToken {
            symbol: transaction.token_symbol.clone(),
            addresses: HashMap::new(), // 실제로는 체인별 주소 매핑
            decimals: 18,
        };
        
        let amount_in = transaction.amount;
        let amount_out = transaction.amount * U256::from(1005) / U256::from(1000); // 0.5% 이익 가정
        
        // 가격 조회 (시작 시점과 완료 시점)
        let start_price = self.price_oracle.get_token_price_usd(&token, transaction.source_chain.chain_id, transaction.monitoring_started).await?;
        let end_price = self.price_oracle.get_token_price_usd(&token, transaction.dest_chain.chain_id, transaction.actual_completion.unwrap_or(Utc::now())).await?;
        
        let amount_in_usd = Decimal::from(amount_in.to::<u128>()) * start_price / Decimal::from(10u128.pow(token.decimals as u32));
        let amount_out_usd = Decimal::from(amount_out.to::<u128>()) * end_price / Decimal::from(10u128.pow(token.decimals as u32));
        
        Ok(TradeInfo {
            source_chain: transaction.source_chain.chain_id,
            dest_chain: transaction.dest_chain.chain_id,
            token,
            amount_in,
            amount_out,
            amount_in_usd,
            amount_out_usd,
            execution_time: (transaction.actual_completion.unwrap_or(Utc::now()) - transaction.monitoring_started).num_seconds() as u64,
            started_at: transaction.monitoring_started,
            completed_at: transaction.actual_completion.unwrap_or(Utc::now()),
        })
    }
    
    /// 실제 비용 분석
    async fn analyze_actual_costs(&self, _transaction: &MonitoredTransaction, trade_info: &TradeInfo) -> Result<CostBreakdown> {
        // Mock 구현 - 실제로는 트랜잭션 영수증에서 추출
        let bridge_fees = Decimal::from_str_exact("5.0").unwrap(); // $5
        
        let source_gas_fees = self.gas_oracle.estimate_transaction_cost(
            trade_info.source_chain, 
            "bridge_deposit"
        ).await.unwrap_or_else(|_| Decimal::from_str_exact("10.0").unwrap());
        
        let dest_gas_fees = self.gas_oracle.estimate_transaction_cost(
            trade_info.dest_chain, 
            "bridge_withdraw"
        ).await.unwrap_or_else(|_| Decimal::from_str_exact("8.0").unwrap());
        
        let dex_fees = Decimal::from_str_exact("2.0").unwrap(); // $2
        let slippage_cost = trade_info.amount_in_usd * Decimal::from_str_exact("0.001").unwrap(); // 0.1%
        let opportunity_cost = Decimal::ZERO; // 계산 복잡성으로 인해 Mock에서는 0
        let miscellaneous_costs = Decimal::from_str_exact("1.0").unwrap(); // $1
        
        let total_costs = bridge_fees + source_gas_fees + dest_gas_fees + dex_fees + slippage_cost + opportunity_cost + miscellaneous_costs;
        
        Ok(CostBreakdown {
            bridge_fees,
            source_gas_fees,
            dest_gas_fees,
            dex_fees,
            slippage_cost,
            opportunity_cost,
            miscellaneous_costs,
            total_costs,
        })
    }
    
    /// 예상 수익 계산
    async fn calculate_expected_profit(&self, trade_info: &TradeInfo) -> Result<ProfitAnalysis> {
        // Mock 구현 - 실제로는 초기 견적에서 가져옴
        let gross_profit_usd = trade_info.amount_out_usd - trade_info.amount_in_usd;
        let gross_profit_tokens = Decimal::from(trade_info.amount_out.to::<u128>()) - Decimal::from(trade_info.amount_in.to::<u128>());
        
        // 예상 비용 (실제 비용과 유사하게 계산)
        let expected_costs = CostBreakdown {
            bridge_fees: Decimal::from_str_exact("5.0").unwrap(),
            source_gas_fees: Decimal::from_str_exact("12.0").unwrap(),
            dest_gas_fees: Decimal::from_str_exact("8.0").unwrap(),
            dex_fees: Decimal::from_str_exact("2.0").unwrap(),
            slippage_cost: trade_info.amount_in_usd * Decimal::from_str_exact("0.001").unwrap(),
            opportunity_cost: Decimal::ZERO,
            miscellaneous_costs: Decimal::from_str_exact("1.0").unwrap(),
            total_costs: Decimal::from_str_exact("28.0").unwrap() + trade_info.amount_in_usd * Decimal::from_str_exact("0.001").unwrap(),
        };
        
        let net_profit_usd = gross_profit_usd - expected_costs.total_costs;
        let profit_margin_percent = if trade_info.amount_in_usd > Decimal::ZERO {
            (net_profit_usd / trade_info.amount_in_usd) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        let roi_percent = if expected_costs.total_costs > Decimal::ZERO {
            (net_profit_usd / expected_costs.total_costs) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        let profit_per_hour = if trade_info.execution_time > 0 {
            net_profit_usd * Decimal::from(3600) / Decimal::from(trade_info.execution_time)
        } else {
            Decimal::ZERO
        };
        
        Ok(ProfitAnalysis {
            gross_profit_tokens,
            gross_profit_usd,
            cost_breakdown: expected_costs,
            net_profit_usd,
            profit_margin_percent,
            roi_percent,
            profit_per_hour,
        })
    }
    
    /// 실제 수익 계산
    async fn calculate_actual_profit(&self, trade_info: &TradeInfo, actual_costs: &CostBreakdown) -> Result<ProfitAnalysis> {
        let gross_profit_usd = trade_info.amount_out_usd - trade_info.amount_in_usd;
        let gross_profit_tokens = Decimal::from(trade_info.amount_out.to::<u128>()) - Decimal::from(trade_info.amount_in.to::<u128>());
        
        let net_profit_usd = gross_profit_usd - actual_costs.total_costs;
        let profit_margin_percent = if trade_info.amount_in_usd > Decimal::ZERO {
            (net_profit_usd / trade_info.amount_in_usd) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        let roi_percent = if actual_costs.total_costs > Decimal::ZERO {
            (net_profit_usd / actual_costs.total_costs) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        let profit_per_hour = if trade_info.execution_time > 0 {
            net_profit_usd * Decimal::from(3600) / Decimal::from(trade_info.execution_time)
        } else {
            Decimal::ZERO
        };
        
        Ok(ProfitAnalysis {
            gross_profit_tokens,
            gross_profit_usd,
            cost_breakdown: actual_costs.clone(),
            net_profit_usd,
            profit_margin_percent,
            roi_percent,
            profit_per_hour,
        })
    }
    
    /// 차이 분석
    async fn analyze_variance(&self, expected: &ProfitAnalysis, actual: &ProfitAnalysis, trade_info: &TradeInfo) -> Result<VarianceAnalysis> {
        let profit_variance_usd = actual.net_profit_usd - expected.net_profit_usd;
        let profit_variance_percent = if expected.net_profit_usd != Decimal::ZERO {
            (profit_variance_usd / expected.net_profit_usd) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        let cost_variance = CostVariance {
            bridge_fee_variance: actual.cost_breakdown.bridge_fees - expected.cost_breakdown.bridge_fees,
            gas_fee_variance: (actual.cost_breakdown.source_gas_fees + actual.cost_breakdown.dest_gas_fees) - 
                             (expected.cost_breakdown.source_gas_fees + expected.cost_breakdown.dest_gas_fees),
            slippage_variance: actual.cost_breakdown.slippage_cost - expected.cost_breakdown.slippage_cost,
            total_variance: actual.cost_breakdown.total_costs - expected.cost_breakdown.total_costs,
        };
        
        // Mock 가격 영향 계산
        let price_impact = PriceImpact {
            source_price_change: Decimal::from_str_exact("0.5").unwrap(), // 0.5% 변동
            dest_price_change: Decimal::from_str_exact("-0.3").unwrap(),  // -0.3% 변동
            total_price_impact: Decimal::from_str_exact("0.2").unwrap(),  // 0.2% 순 영향
            market_volatility: Decimal::from_str_exact("1.2").unwrap(),   // 1.2% 변동성
        };
        
        let time_impact = TimeImpact {
            expected_duration: 300, // 5분 예상
            actual_duration: trade_info.execution_time,
            duration_variance: trade_info.execution_time as i64 - 300,
            time_cost_impact: Decimal::from_str_exact("0.5").unwrap(), // $0.5 지연 비용
        };
        
        // 차이 원인 분석
        let mut variance_causes = Vec::new();
        
        if cost_variance.gas_fee_variance.abs() > Decimal::from_str_exact("1.0").unwrap() {
            variance_causes.push(VarianceCause {
                category: VarianceCategory::GasFees,
                description: "가스비 변동".to_string(),
                impact_usd: cost_variance.gas_fee_variance,
                impact_percent: (cost_variance.gas_fee_variance / expected.cost_breakdown.total_costs) * Decimal::from(100),
                severity: if cost_variance.gas_fee_variance.abs() > Decimal::from_str_exact("5.0").unwrap() {
                    VarianceSeverity::High
                } else {
                    VarianceSeverity::Medium
                },
                mitigation_suggestion: "가스 가격 모니터링 개선".to_string(),
            });
        }
        
        Ok(VarianceAnalysis {
            profit_variance_usd,
            profit_variance_percent,
            cost_variance,
            price_impact,
            time_impact,
            variance_causes,
        })
    }
    
    /// 세금 계산
    async fn calculate_tax(&self, profit: &ProfitAnalysis, trade_info: &TradeInfo) -> Result<TaxAnalysis> {
        self.tax_calculator.calculate_tax(profit, trade_info).await
    }
    
    /// 순수익 계산
    async fn calculate_net_profit(&self, profit: &ProfitAnalysis, tax: &TaxAnalysis) -> Result<NetProfitSummary> {
        let gross_profit = profit.gross_profit_usd;
        let total_costs = profit.cost_breakdown.total_costs;
        let pre_tax_profit = profit.net_profit_usd;
        let tax_amount = tax.estimated_tax;
        let final_net_profit = pre_tax_profit - tax_amount;
        
        let final_profit_margin = if gross_profit > Decimal::ZERO {
            (final_net_profit / gross_profit) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        // 연환산 수익률 계산 (가정: 이 수익률이 지속된다면)
        let annualized_return = if final_net_profit > Decimal::ZERO && total_costs > Decimal::ZERO {
            let daily_return = (final_net_profit / total_costs) * Decimal::from(24) * Decimal::from(3600); // 시간당 → 일일
            daily_return * Decimal::from(365) * Decimal::from(100) // 연환산 %
        } else {
            Decimal::ZERO
        };
        
        Ok(NetProfitSummary {
            gross_profit,
            total_costs,
            pre_tax_profit,
            tax_amount,
            final_net_profit,
            final_profit_margin,
            annualized_return,
        })
    }
    
    /// 수익성 평가
    async fn rate_profitability(&self, net_profit: &NetProfitSummary, variance: &VarianceAnalysis, trade_info: &TradeInfo) -> Result<ProfitabilityRating> {
        // 점수 계산 (0-100)
        let mut score = 0u8;
        
        // 수익성 (40점)
        if net_profit.final_net_profit >= self.profitability_thresholds.minimum_profit_usd {
            score += 20;
            if net_profit.final_profit_margin >= self.profitability_thresholds.minimum_profit_margin {
                score += 20;
            }
        }
        
        // 효율성 (30점)
        if net_profit.annualized_return > Decimal::from(50) { // 50% APY 이상
            score += 30;
        } else if net_profit.annualized_return > Decimal::from(20) { // 20% APY 이상
            score += 20;
        } else if net_profit.annualized_return > Decimal::from(10) { // 10% APY 이상
            score += 10;
        }
        
        // 일관성 (20점)
        if variance.profit_variance_percent.abs() < Decimal::from(5) { // 5% 이내 차이
            score += 20;
        } else if variance.profit_variance_percent.abs() < Decimal::from(10) { // 10% 이내 차이
            score += 10;
        }
        
        // 실행 품질 (10점)
        if trade_info.execution_time < 300 { // 5분 이내
            score += 10;
        } else if trade_info.execution_time < 600 { // 10분 이내
            score += 5;
        }
        
        let grade = match score {
            90..=100 => ProfitabilityGrade::Excellent,
            70..=89 => ProfitabilityGrade::Good,
            50..=69 => ProfitabilityGrade::Average,
            30..=49 => ProfitabilityGrade::Poor,
            _ => ProfitabilityGrade::Terrible,
        };
        
        // 리스크 조정 수익률
        let risk_factor = Decimal::from(100 - variance.price_impact.market_volatility.to_u8().unwrap_or(10)) / Decimal::from(100);
        let risk_adjusted_return = net_profit.annualized_return * risk_factor;
        
        // 벤치마크 비교 (Mock)
        let benchmark_comparison = BenchmarkComparison {
            market_average_return: Decimal::from_str_exact("8.0").unwrap(), // 8% 시장 평균
            outperformance: net_profit.annualized_return - Decimal::from_str_exact("8.0").unwrap(),
            percentile_ranking: if score >= 80 { 90 } else if score >= 60 { 70 } else { 50 },
        };
        
        // 개선 영역
        let mut improvement_areas = Vec::new();
        if trade_info.execution_time > 300 {
            improvement_areas.push(ImprovementArea {
                category: "실행 속도".to_string(),
                current_performance: Decimal::from(trade_info.execution_time),
                target_performance: Decimal::from(300),
                potential_improvement: Decimal::from_str_exact("2.0").unwrap(), // $2 개선 가능
                action_items: vec!["더 빠른 브리지 사용".to_string(), "가스 최적화".to_string()],
            });
        }
        
        Ok(ProfitabilityRating {
            overall_score: score,
            grade,
            risk_adjusted_return,
            benchmark_comparison,
            improvement_areas,
        })
    }
    
    /// 권장사항 생성
    async fn generate_recommendations(&self, variance: &VarianceAnalysis, rating: &ProfitabilityRating, trade_info: &TradeInfo) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        // 가스비 최적화
        if variance.cost_variance.gas_fee_variance > Decimal::from_str_exact("2.0").unwrap() {
            recommendations.push(Recommendation {
                category: RecommendationCategory::CostOptimization,
                title: "가스비 최적화".to_string(),
                description: "가스 가격이 예상보다 높았습니다".to_string(),
                priority: RecommendationPriority::High,
                potential_impact: variance.cost_variance.gas_fee_variance,
                implementation_effort: ImplementationEffort::Low,
                specific_actions: vec![
                    "가스 가격 모니터링 강화".to_string(),
                    "더 저렴한 시간대 선택".to_string(),
                    "가스 최적화된 스마트 컨트랙트 사용".to_string(),
                ],
            });
        }
        
        // 브리지 선택 최적화
        if rating.overall_score < 70 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::BridgeSelection,
                title: "브리지 프로토콜 최적화".to_string(),
                description: "더 효율적인 브리지 프로토콜 사용을 고려하세요".to_string(),
                priority: RecommendationPriority::Medium,
                potential_impact: Decimal::from_str_exact("5.0").unwrap(),
                implementation_effort: ImplementationEffort::Medium,
                specific_actions: vec![
                    "브리지 성능 벤치마킹".to_string(),
                    "다중 브리지 비교 시스템 구축".to_string(),
                ],
            });
        }
        
        // 타이밍 개선
        if trade_info.execution_time > 600 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::TimingImprovement,
                title: "실행 속도 개선".to_string(),
                description: "거래 실행 시간이 너무 깁니다".to_string(),
                priority: RecommendationPriority::High,
                potential_impact: Decimal::from_str_exact("3.0").unwrap(),
                implementation_effort: ImplementationEffort::Medium,
                specific_actions: vec![
                    "더 빠른 브리지 프로토콜 사용".to_string(),
                    "실행 자동화 개선".to_string(),
                    "네트워크 연결 최적화".to_string(),
                ],
            });
        }
        
        Ok(recommendations)
    }
    
    /// 검증된 거래 조회
    pub async fn get_verified_trade(&self, execution_id: &str) -> Option<VerifiedTrade> {
        let verified = self.verified_trades.read().await;
        verified.get(execution_id).cloned()
    }
    
    /// 모든 검증된 거래 조회
    pub async fn get_all_verified_trades(&self) -> Vec<VerifiedTrade> {
        let verified = self.verified_trades.read().await;
        verified.values().cloned().collect()
    }
    
    /// 수익성 요약 통계
    pub async fn get_profitability_summary(&self) -> ProfitabilitySummary {
        let verified = self.verified_trades.read().await;
        
        if verified.is_empty() {
            return ProfitabilitySummary::default();
        }
        
        let total_trades = verified.len();
        let profitable_trades = verified.values()
            .filter(|t| t.net_profit.final_net_profit > Decimal::ZERO)
            .count();
        
        let total_profit: Decimal = verified.values()
            .map(|t| t.net_profit.final_net_profit)
            .sum();
        
        let avg_profit = total_profit / Decimal::from(total_trades);
        
        let avg_score: f64 = verified.values()
            .map(|t| t.profitability_rating.overall_score as f64)
            .sum::<f64>() / total_trades as f64;
        
        ProfitabilitySummary {
            total_trades,
            profitable_trades,
            success_rate: profitable_trades as f64 / total_trades as f64,
            total_profit,
            avg_profit,
            avg_score: avg_score as u8,
        }
    }
}

/// 수익성 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitabilitySummary {
    pub total_trades: usize,
    pub profitable_trades: usize,
    pub success_rate: f64,
    pub total_profit: Decimal,
    pub avg_profit: Decimal,
    pub avg_score: u8,
}

impl Default for ProfitabilitySummary {
    fn default() -> Self {
        Self {
            total_trades: 0,
            profitable_trades: 0,
            success_rate: 0.0,
            total_profit: Decimal::ZERO,
            avg_profit: Decimal::ZERO,
            avg_score: 0,
        }
    }
}

impl Default for TaxAnalysis {
    fn default() -> Self {
        Self {
            jurisdiction: "US".to_string(),
            taxable_events: Vec::new(),
            total_taxable_income: Decimal::ZERO,
            estimated_tax: Decimal::ZERO,
            after_tax_profit: Decimal::ZERO,
            tax_optimization_suggestions: Vec::new(),
        }
    }
}

impl TaxCalculator {
    fn new(jurisdiction: String) -> Self {
        let mut tax_rates = HashMap::new();
        tax_rates.insert(TaxEventType::Trading, Decimal::from_str_exact("0.25").unwrap()); // 25%
        tax_rates.insert(TaxEventType::ArbitrageProfit, Decimal::from_str_exact("0.20").unwrap()); // 20%
        tax_rates.insert(TaxEventType::BridgeReward, Decimal::from_str_exact("0.15").unwrap()); // 15%
        tax_rates.insert(TaxEventType::GasFeeDeduction, Decimal::from_str_exact("-1.0").unwrap()); // 공제
        
        Self {
            jurisdiction,
            tax_rates,
            deduction_rules: Vec::new(),
        }
    }
    
    async fn calculate_tax(&self, profit: &ProfitAnalysis, trade_info: &TradeInfo) -> Result<TaxAnalysis> {
        let mut taxable_events = Vec::new();
        
        // 아비트래지 수익 과세
        if profit.net_profit_usd > Decimal::ZERO {
            let tax_rate = self.tax_rates.get(&TaxEventType::ArbitrageProfit)
                .copied()
                .unwrap_or(Decimal::from_str_exact("0.20").unwrap());
            
            let tax_amount = profit.net_profit_usd * tax_rate;
            
            taxable_events.push(TaxableEvent {
                event_type: TaxEventType::ArbitrageProfit,
                description: "크로스체인 아비트래지 수익".to_string(),
                taxable_amount: profit.net_profit_usd,
                tax_rate,
                tax_amount,
                occurred_at: trade_info.completed_at,
            });
        }
        
        let total_taxable_income = taxable_events.iter()
            .map(|e| e.taxable_amount)
            .sum();
        
        let estimated_tax = taxable_events.iter()
            .map(|e| e.tax_amount)
            .sum();
        
        let after_tax_profit = profit.net_profit_usd - estimated_tax;
        
        Ok(TaxAnalysis {
            jurisdiction: self.jurisdiction.clone(),
            taxable_events,
            total_taxable_income,
            estimated_tax,
            after_tax_profit,
            tax_optimization_suggestions: vec![
                "거래 손실을 이용한 세금 최적화 고려".to_string(),
                "장기 보유 자산과의 밸런싱".to_string(),
            ],
        })
    }
}

// Clone 구현을 위한 별도 구조체들
impl Clone for CrossChainProfitVerifier {
    fn clone(&self) -> Self {
        Self {
            verified_trades: Arc::clone(&self.verified_trades),
            pending_verifications: Arc::clone(&self.pending_verifications),
            price_oracle: Arc::clone(&self.price_oracle),
            gas_oracle: Arc::clone(&self.gas_oracle),
            config: self.config.clone(),
            profitability_thresholds: self.profitability_thresholds.clone(),
            tax_calculator: Arc::clone(&self.tax_calculator),
        }
    }
}