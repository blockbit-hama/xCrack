use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Timelike};
use tracing::info;
use serde::{Serialize, Deserialize};
use alloy::primitives::U256;

use crate::types::{ChainId, BridgeProtocol};
use super::performance_tracker::{BridgePerformanceTracker, BridgePerformanceData};

/// 동적 브리지 점수 시스템
/// 
/// 실시간 성능 데이터를 기반으로 브리지 신뢰도를 동적으로 계산하고 조정합니다.
/// - 실시간 성능 지표 기반 점수 계산
/// - 시장 조건에 따른 가중치 조정
/// - 라우트별 맞춤형 점수 산정
/// - 예측 모델을 통한 미래 성능 예측
#[derive(Debug)]
pub struct DynamicBridgeScorer {
    /// 성능 추적기 참조
    performance_tracker: Arc<BridgePerformanceTracker>,
    
    /// 점수 계산 설정
    scoring_config: ScoringConfig,
    
    /// 시장 조건 모니터
    market_conditions: Arc<RwLock<MarketConditions>>,
    
    /// 예측 모델 데이터
    prediction_models: Arc<RwLock<HashMap<BridgeProtocol, PredictionModel>>>,
    
    /// 점수 히스토리 (최근 100개)
    score_history: Arc<RwLock<Vec<ScoreSnapshot>>>,
    
    /// 라우트별 맞춤 설정
    route_configs: Arc<RwLock<HashMap<RouteKey, RouteSpecificConfig>>>,
}

/// 라우트 키
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RouteKey {
    pub source_chain: ChainId,
    pub dest_chain: ChainId,
    pub token_symbol: String,
}

/// 점수 계산 설정
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// 기본 가중치
    pub base_weights: ScoreWeights,
    
    /// 시장 조건별 가중치 조정
    pub market_adjustments: MarketAdjustments,
    
    /// 예측 모델 설정
    pub prediction_settings: PredictionSettings,
    
    /// 점수 업데이트 간격 (초)
    pub update_interval_seconds: u64,
    
    /// 최소 데이터 요구사항
    pub min_data_requirements: MinDataRequirements,
}

/// 점수 가중치
#[derive(Debug, Clone)]
pub struct ScoreWeights {
    /// 성공률 가중치 (0.0-1.0)
    pub success_rate: f64,
    
    /// 완료 시간 가중치
    pub completion_time: f64,
    
    /// 비용 효율성 가중치
    pub cost_efficiency: f64,
    
    /// 가용성 가중치
    pub availability: f64,
    
    /// 일관성 가중치
    pub consistency: f64,
    
    /// 유동성 가중치
    pub liquidity: f64,
    
    /// 최근 성과 가중치
    pub recent_performance: f64,
    
    /// 예측 성과 가중치
    pub predicted_performance: f64,
}

/// 시장 조건별 가중치 조정
#[derive(Debug, Clone)]
pub struct MarketAdjustments {
    /// 고변동성 시장에서의 조정
    pub high_volatility: ScoreWeights,
    
    /// 저유동성 시장에서의 조정
    pub low_liquidity: ScoreWeights,
    
    /// 네트워크 혼잡 시 조정
    pub network_congestion: ScoreWeights,
    
    /// 가스 가격 급등 시 조정
    pub high_gas_price: ScoreWeights,
}

/// 예측 모델 설정
#[derive(Debug, Clone)]
pub struct PredictionSettings {
    /// 예측 시간 윈도우 (시간)
    pub prediction_window_hours: i64,
    
    /// 학습 데이터 윈도우 (일)
    pub training_window_days: i64,
    
    /// 최소 학습 데이터 포인트
    pub min_training_points: usize,
    
    /// 예측 신뢰도 임계값
    pub confidence_threshold: f64,
}

/// 최소 데이터 요구사항
#[derive(Debug, Clone)]
pub struct MinDataRequirements {
    /// 최소 실행 횟수
    pub min_executions: u64,
    
    /// 최소 데이터 수집 기간 (시간)
    pub min_data_age_hours: i64,
    
    /// 최소 성공적 거래 수
    pub min_successful_transactions: u64,
}

/// 시장 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    /// 시장 변동성 (0.0-1.0)
    pub volatility_index: f64,
    
    /// 전체 시장 유동성
    pub market_liquidity: U256,
    
    /// 평균 가스 가격 (gwei)
    pub avg_gas_price: f64,
    
    /// 네트워크 혼잡도 (0.0-1.0)
    pub network_congestion: f64,
    
    /// DEX 스프레드 (%)
    pub dex_spread_percent: f64,
    
    /// 브리지 전체 사용률
    pub bridge_utilization: f64,
    
    /// 마지막 업데이트 시간
    pub last_updated: DateTime<Utc>,
}

/// 예측 모델
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionModel {
    /// 브리지 프로토콜
    pub bridge: BridgeProtocol,
    
    /// 성공률 예측
    pub predicted_success_rate: f64,
    
    /// 완료 시간 예측 (초)
    pub predicted_completion_time: f64,
    
    /// 비용 예측 (USD)
    pub predicted_cost: f64,
    
    /// 예측 신뢰도 (0.0-1.0)
    pub prediction_confidence: f64,
    
    /// 예측 생성 시간
    pub prediction_time: DateTime<Utc>,
    
    /// 예측 유효 기간
    pub valid_until: DateTime<Utc>,
    
    /// 모델 정확도 (최근 예측들의 정확도)
    pub model_accuracy: f64,
}

/// 점수 스냅샷
#[derive(Debug, Clone)]
pub struct ScoreSnapshot {
    /// 브리지별 점수
    pub bridge_scores: HashMap<BridgeProtocol, BridgeScore>,
    
    /// 라우트별 점수
    pub route_scores: HashMap<RouteKey, RouteScore>,
    
    /// 시장 조건
    pub market_conditions: MarketConditions,
    
    /// 점수 계산 시간
    pub timestamp: DateTime<Utc>,
}

/// 브리지 점수
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeScore {
    /// 전체 점수 (0.0-100.0)
    pub overall_score: f64,
    
    /// 세부 점수들
    pub component_scores: ComponentScores,
    
    /// 시장 조건 조정 후 점수
    pub market_adjusted_score: f64,
    
    /// 예측 기반 점수
    pub predicted_score: f64,
    
    /// 최종 권장 점수
    pub final_recommendation_score: f64,
    
    /// 점수 변화 추세
    pub trend: ScoreTrend,
    
    /// 신뢰도 등급
    pub confidence_grade: ConfidenceGrade,
    
    /// 점수 계산 시간
    pub calculated_at: DateTime<Utc>,
}

/// 세부 점수 구성요소
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentScores {
    /// 성공률 점수
    pub success_rate_score: f64,
    
    /// 시간 효율성 점수
    pub time_efficiency_score: f64,
    
    /// 비용 효율성 점수
    pub cost_efficiency_score: f64,
    
    /// 가용성 점수
    pub availability_score: f64,
    
    /// 일관성 점수
    pub consistency_score: f64,
    
    /// 유동성 점수
    pub liquidity_score: f64,
    
    /// 최근 성과 점수
    pub recent_performance_score: f64,
    
    /// 예측 성과 점수
    pub predicted_performance_score: f64,
}

/// 라우트별 점수
#[derive(Debug, Clone)]
pub struct RouteScore {
    /// 라우트 키
    pub route: RouteKey,
    
    /// 브리지별 점수
    pub bridge_scores: HashMap<BridgeProtocol, f64>,
    
    /// 최적 브리지
    pub best_bridge: Option<BridgeProtocol>,
    
    /// 최적 점수
    pub best_score: f64,
    
    /// 대안 브리지들 (점수 순)
    pub alternatives: Vec<(BridgeProtocol, f64)>,
    
    /// 라우트 위험도
    pub route_risk_level: RiskLevel,
    
    /// 라우트별 특수 고려사항
    pub special_considerations: Vec<String>,
}

/// 라우트별 특수 설정
#[derive(Debug, Clone)]
pub struct RouteSpecificConfig {
    /// 라우트별 가중치 조정
    pub weight_adjustments: ScoreWeights,
    
    /// 최소 요구 점수
    pub min_required_score: f64,
    
    /// 특수 요구사항
    pub special_requirements: Vec<SpecialRequirement>,
    
    /// 블랙리스트 브리지들
    pub blacklisted_bridges: Vec<BridgeProtocol>,
    
    /// 선호 브리지들
    pub preferred_bridges: Vec<BridgeProtocol>,
}

/// 특수 요구사항
#[derive(Debug, Clone)]
pub enum SpecialRequirement {
    /// 최대 허용 시간 (초)
    MaxTime(u64),
    
    /// 최대 허용 비용 (USD)
    MaxCost(f64),
    
    /// 최소 요구 성공률
    MinSuccessRate(f64),
    
    /// 최소 요구 유동성
    MinLiquidity(U256),
    
    /// 규제 준수 요구
    ComplianceRequired,
    
    /// 빠른 최종성 요구
    FastFinality,
}

/// 점수 변화 추세
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScoreTrend {
    /// 상승 추세
    Improving { rate: f64 },
    
    /// 하락 추세
    Declining { rate: f64 },
    
    /// 안정 상태
    Stable,
    
    /// 변동성 큼
    Volatile { variance: f64 },
}

/// 신뢰도 등급
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfidenceGrade {
    /// 매우 높음 (95%+)
    VeryHigh,
    
    /// 높음 (85-95%)
    High,
    
    /// 보통 (70-85%)
    Medium,
    
    /// 낮음 (50-70%)
    Low,
    
    /// 매우 낮음 (<50%)
    VeryLow,
}

/// 위험도 수준
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    /// 낮음
    Low,
    
    /// 보통
    Medium,
    
    /// 높음
    High,
    
    /// 매우 높음
    Critical,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            base_weights: ScoreWeights {
                success_rate: 0.25,
                completion_time: 0.20,
                cost_efficiency: 0.15,
                availability: 0.15,
                consistency: 0.10,
                liquidity: 0.05,
                recent_performance: 0.05,
                predicted_performance: 0.05,
            },
            market_adjustments: MarketAdjustments::default(),
            prediction_settings: PredictionSettings {
                prediction_window_hours: 4,
                training_window_days: 7,
                min_training_points: 50,
                confidence_threshold: 0.7,
            },
            update_interval_seconds: 300, // 5분
            min_data_requirements: MinDataRequirements {
                min_executions: 10,
                min_data_age_hours: 24,
                min_successful_transactions: 5,
            },
        }
    }
}

impl Default for MarketAdjustments {
    fn default() -> Self {
        Self {
            high_volatility: ScoreWeights {
                success_rate: 0.35,      // 변동성 높을 때 안정성 중시
                completion_time: 0.25,   // 빠른 실행 중요
                cost_efficiency: 0.10,   // 비용보다 안정성
                availability: 0.20,
                consistency: 0.05,
                liquidity: 0.03,
                recent_performance: 0.02,
                predicted_performance: 0.00,
            },
            low_liquidity: ScoreWeights {
                success_rate: 0.20,
                completion_time: 0.15,
                cost_efficiency: 0.20,
                availability: 0.15,
                consistency: 0.05,
                liquidity: 0.20,        // 유동성 매우 중요
                recent_performance: 0.03,
                predicted_performance: 0.02,
            },
            network_congestion: ScoreWeights {
                success_rate: 0.30,
                completion_time: 0.35,   // 빠른 처리 매우 중요
                cost_efficiency: 0.05,   // 비용 덜 중요
                availability: 0.20,
                consistency: 0.05,
                liquidity: 0.03,
                recent_performance: 0.02,
                predicted_performance: 0.00,
            },
            high_gas_price: ScoreWeights {
                success_rate: 0.25,
                completion_time: 0.15,
                cost_efficiency: 0.30,   // 비용 효율성 매우 중요
                availability: 0.15,
                consistency: 0.05,
                liquidity: 0.05,
                recent_performance: 0.03,
                predicted_performance: 0.02,
            },
        }
    }
}

impl DynamicBridgeScorer {
    /// 새로운 동적 점수 시스템 생성
    pub fn new(performance_tracker: Arc<BridgePerformanceTracker>) -> Self {
        Self {
            performance_tracker,
            scoring_config: ScoringConfig::default(),
            market_conditions: Arc::new(RwLock::new(MarketConditions::default())),
            prediction_models: Arc::new(RwLock::new(HashMap::new())),
            score_history: Arc::new(RwLock::new(Vec::new())),
            route_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 커스텀 설정으로 생성
    pub fn with_config(
        performance_tracker: Arc<BridgePerformanceTracker>,
        config: ScoringConfig,
    ) -> Self {
        Self {
            performance_tracker,
            scoring_config: config,
            market_conditions: Arc::new(RwLock::new(MarketConditions::default())),
            prediction_models: Arc::new(RwLock::new(HashMap::new())),
            score_history: Arc::new(RwLock::new(Vec::new())),
            route_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 브리지 점수 계산
    pub async fn calculate_bridge_score(&self, bridge: BridgeProtocol) -> Result<BridgeScore> {
        // 성능 데이터 가져오기
        let performance_data = self.performance_tracker
            .get_bridge_performance(bridge.clone())
            .await
            .ok_or_else(|| anyhow::anyhow!("No performance data for bridge {}", bridge.name()))?;
        
        // 최소 데이터 요구사항 확인
        if !self.meets_min_requirements(&performance_data).await {
            return Ok(BridgeScore {
                overall_score: 0.0,
                component_scores: ComponentScores::default(),
                market_adjusted_score: 0.0,
                predicted_score: 0.0,
                final_recommendation_score: 0.0,
                trend: ScoreTrend::Stable,
                confidence_grade: ConfidenceGrade::VeryLow,
                calculated_at: Utc::now(),
            });
        }
        
        // 기본 점수 계산
        let component_scores = self.calculate_component_scores(&performance_data).await?;
        let base_score = self.calculate_weighted_score(&component_scores, &self.scoring_config.base_weights).await;
        
        // 시장 조건 기반 조정
        let market_conditions = self.market_conditions.read().await;
        let market_weights = self.get_market_adjusted_weights(&market_conditions).await;
        let market_adjusted_score = self.calculate_weighted_score(&component_scores, &market_weights).await;
        
        // 예측 기반 점수
        let predicted_score = self.calculate_predicted_score(bridge.clone()).await?;
        
        // 최종 권장 점수 (기본 60% + 시장조정 25% + 예측 15%)
        let final_score = base_score * 0.6 + market_adjusted_score * 0.25 + predicted_score * 0.15;
        
        // 점수 추세 계산
        let trend = self.calculate_score_trend(bridge.clone()).await;
        
        // 신뢰도 등급 계산
        let confidence_grade = self.calculate_confidence_grade(&performance_data, &component_scores).await;
        
        let bridge_score = BridgeScore {
            overall_score: base_score,
            component_scores,
            market_adjusted_score,
            predicted_score,
            final_recommendation_score: final_score,
            trend,
            confidence_grade,
            calculated_at: Utc::now(),
        };
        
        info!("📊 브리지 {} 점수 계산 완료: {:.1}/100 (신뢰도: {:?})", 
              bridge.name(), final_score, bridge_score.confidence_grade);
        
        Ok(bridge_score)
    }
    
    /// 라우트별 최적 브리지 추천
    pub async fn recommend_best_bridge_for_route(
        &self,
        source_chain: ChainId,
        dest_chain: ChainId,
        token_symbol: String,
        amount_usd: f64,
    ) -> Result<RouteScore> {
        let route_key = RouteKey {
            source_chain,
            dest_chain,
            token_symbol: token_symbol.clone(),
        };
        
        // 모든 브리지 성능 데이터 가져오기
        let _all_performance = self.performance_tracker.get_all_bridge_performance().await;
        
        let mut bridge_scores = HashMap::new();
        let mut valid_bridges = Vec::new();
        
        // 각 브리지별 점수 계산
        for bridge in &[
            BridgeProtocol::Stargate,
            BridgeProtocol::Hop,
            BridgeProtocol::Rubic,
            BridgeProtocol::Synapse,
            BridgeProtocol::LiFi,
            BridgeProtocol::Across,
            BridgeProtocol::Multichain,
        ] {
            if let Ok(score) = self.calculate_bridge_score(bridge.clone()).await {
                // 라우트별 특수 요구사항 적용
                let route_adjusted_score = self.apply_route_specific_adjustments(
                    &route_key,
                    bridge.clone(),
                    score.final_recommendation_score,
                    amount_usd,
                ).await;
                
                bridge_scores.insert(bridge.clone(), route_adjusted_score);
                valid_bridges.push((bridge.clone(), route_adjusted_score));
            }
        }
        
        // 점수순 정렬
        valid_bridges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        let best_bridge = valid_bridges.first().map(|(bridge, _)| bridge.clone());
        let best_score = valid_bridges.first().map(|(_, score)| *score).unwrap_or(0.0);
        let alternatives = valid_bridges.into_iter().skip(1).collect();
        
        // 라우트 위험도 평가
        let route_risk_level = self.assess_route_risk(&route_key, amount_usd).await;
        
        // 특수 고려사항
        let special_considerations = self.get_route_considerations(&route_key).await;
        
        let route_score = RouteScore {
            route: route_key,
            bridge_scores,
            best_bridge,
            best_score,
            alternatives,
            route_risk_level,
            special_considerations,
        };
        
        info!("🎯 라우트 {} -> {} 최적 브리지: {:?} (점수: {:.1})", 
              source_chain.name(), dest_chain.name(), route_score.best_bridge, best_score);
        
        Ok(route_score)
    }
    
    /// 세부 점수 구성요소 계산
    async fn calculate_component_scores(&self, data: &BridgePerformanceData) -> Result<ComponentScores> {
        let success_rate_score = (data.success_rate * 100.0).min(100.0);
        
        let time_efficiency_score = if data.avg_completion_time > 0.0 {
            // 5분을 기준으로 정규화 (빠를수록 높은 점수)
            (300.0 / data.avg_completion_time * 100.0).min(100.0)
        } else {
            0.0
        };
        
        let cost_efficiency_score = if data.avg_cost_usd > 0.0 {
            // $10을 기준으로 정규화 (저렴할수록 높은 점수)
            (10.0 / data.avg_cost_usd * 100.0).min(100.0)
        } else {
            100.0
        };
        
        let availability_score = data.availability_24h * 100.0;
        
        let consistency_score = if data.max_completion_time > data.min_completion_time {
            let variance = (data.max_completion_time - data.min_completion_time) / data.avg_completion_time;
            ((1.0 - variance.min(1.0)) * 100.0).max(0.0)
        } else {
            100.0
        };
        
        // 유동성 점수 (임시로 고정값, 실제로는 유동성 데이터 필요)
        let liquidity_score = 75.0;
        
        // 최근 성과 점수 (최근 24시간 기준)
        let recent_performance_score = self.calculate_recent_performance_score(data).await;
        
        // 예측 성과 점수
        let predicted_performance_score = 70.0; // 예측 모델 미구현시 기본값
        
        Ok(ComponentScores {
            success_rate_score,
            time_efficiency_score,
            cost_efficiency_score,
            availability_score,
            consistency_score,
            liquidity_score,
            recent_performance_score,
            predicted_performance_score,
        })
    }
    
    /// 가중치 기반 점수 계산
    async fn calculate_weighted_score(&self, scores: &ComponentScores, weights: &ScoreWeights) -> f64 {
        scores.success_rate_score * weights.success_rate +
        scores.time_efficiency_score * weights.completion_time +
        scores.cost_efficiency_score * weights.cost_efficiency +
        scores.availability_score * weights.availability +
        scores.consistency_score * weights.consistency +
        scores.liquidity_score * weights.liquidity +
        scores.recent_performance_score * weights.recent_performance +
        scores.predicted_performance_score * weights.predicted_performance
    }
    
    /// 시장 조건 기반 가중치 조정
    async fn get_market_adjusted_weights(&self, conditions: &MarketConditions) -> ScoreWeights {
        let mut adjusted_weights = self.scoring_config.base_weights.clone();
        
        // 변동성이 높으면 안정성 중시
        if conditions.volatility_index > 0.7 {
            adjusted_weights = self.blend_weights(&adjusted_weights, &self.scoring_config.market_adjustments.high_volatility, 0.5);
        }
        
        // 유동성이 낮으면 유동성 중시
        if conditions.bridge_utilization > 0.8 {
            adjusted_weights = self.blend_weights(&adjusted_weights, &self.scoring_config.market_adjustments.low_liquidity, 0.3);
        }
        
        // 네트워크 혼잡시 속도 중시
        if conditions.network_congestion > 0.7 {
            adjusted_weights = self.blend_weights(&adjusted_weights, &self.scoring_config.market_adjustments.network_congestion, 0.4);
        }
        
        // 가스비 높으면 비용 효율성 중시
        if conditions.avg_gas_price > 50.0 {
            adjusted_weights = self.blend_weights(&adjusted_weights, &self.scoring_config.market_adjustments.high_gas_price, 0.3);
        }
        
        adjusted_weights
    }
    
    /// 가중치 블렌딩
    fn blend_weights(&self, base: &ScoreWeights, adjustment: &ScoreWeights, blend_ratio: f64) -> ScoreWeights {
        ScoreWeights {
            success_rate: base.success_rate * (1.0 - blend_ratio) + adjustment.success_rate * blend_ratio,
            completion_time: base.completion_time * (1.0 - blend_ratio) + adjustment.completion_time * blend_ratio,
            cost_efficiency: base.cost_efficiency * (1.0 - blend_ratio) + adjustment.cost_efficiency * blend_ratio,
            availability: base.availability * (1.0 - blend_ratio) + adjustment.availability * blend_ratio,
            consistency: base.consistency * (1.0 - blend_ratio) + adjustment.consistency * blend_ratio,
            liquidity: base.liquidity * (1.0 - blend_ratio) + adjustment.liquidity * blend_ratio,
            recent_performance: base.recent_performance * (1.0 - blend_ratio) + adjustment.recent_performance * blend_ratio,
            predicted_performance: base.predicted_performance * (1.0 - blend_ratio) + adjustment.predicted_performance * blend_ratio,
        }
    }
    
    /// 최근 성과 점수 계산
    async fn calculate_recent_performance_score(&self, data: &BridgePerformanceData) -> f64 {
        // 최근 24시간 시간대별 성과를 기반으로 점수 계산
        if data.hourly_stats.is_empty() {
            return 50.0; // 기본값
        }
        
        let recent_hours = 6; // 최근 6시간
        let current_hour = Utc::now().hour() as usize;
        
        let mut total_score = 0.0;
        let mut count = 0;
        
        for i in 0..recent_hours {
            let hour_index = (current_hour + 24 - i) % 24;
            if let Some(hourly_stat) = data.hourly_stats.get(hour_index) {
                if hourly_stat.executions > 0 {
                    let hour_score = hourly_stat.success_rate * 100.0;
                    total_score += hour_score;
                    count += 1;
                }
            }
        }
        
        if count > 0 {
            total_score / count as f64
        } else {
            data.success_rate * 100.0
        }
    }
    
    /// 예측 기반 점수 계산
    async fn calculate_predicted_score(&self, bridge: BridgeProtocol) -> Result<f64> {
        let models = self.prediction_models.read().await;
        
        if let Some(model) = models.get(&bridge) {
            if model.valid_until > Utc::now() && model.prediction_confidence > self.scoring_config.prediction_settings.confidence_threshold {
                // 예측된 성공률과 성능을 기반으로 점수 계산
                let predicted_score = model.predicted_success_rate * 100.0 * model.prediction_confidence;
                return Ok(predicted_score.min(100.0));
            }
        }
        
        // 예측 데이터가 없거나 신뢰할 수 없으면 현재 성과 기반
        Ok(70.0) // 기본값
    }
    
    /// 점수 추세 계산
    async fn calculate_score_trend(&self, bridge: BridgeProtocol) -> ScoreTrend {
        let history = self.score_history.read().await;
        
        if history.len() < 3 {
            return ScoreTrend::Stable;
        }
        
        // 최근 3개 점수 비교
        let recent_scores: Vec<f64> = history.iter()
            .rev()
            .take(3)
            .filter_map(|snapshot| snapshot.bridge_scores.get(&bridge))
            .map(|score| score.final_recommendation_score)
            .collect();
        
        if recent_scores.len() < 3 {
            return ScoreTrend::Stable;
        }
        
        let latest = recent_scores[0];
        let previous = recent_scores[1];
        let older = recent_scores[2];
        
        let recent_change = latest - previous;
        let previous_change = previous - older;
        
        let trend_rate = (recent_change + previous_change) / 2.0;
        
        if trend_rate.abs() < 2.0 {
            ScoreTrend::Stable
        } else if trend_rate > 0.0 {
            ScoreTrend::Improving { rate: trend_rate }
        } else {
            ScoreTrend::Declining { rate: trend_rate.abs() }
        }
    }
    
    /// 신뢰도 등급 계산
    async fn calculate_confidence_grade(&self, data: &BridgePerformanceData, _scores: &ComponentScores) -> ConfidenceGrade {
        let confidence_score = if data.total_executions >= 100 {
            95.0
        } else if data.total_executions >= 50 {
            85.0
        } else if data.total_executions >= 20 {
            75.0
        } else if data.total_executions >= 10 {
            60.0
        } else {
            40.0
        };
        
        match confidence_score {
            95.0.. => ConfidenceGrade::VeryHigh,
            85.0..95.0 => ConfidenceGrade::High,
            70.0..85.0 => ConfidenceGrade::Medium,
            50.0..70.0 => ConfidenceGrade::Low,
            _ => ConfidenceGrade::VeryLow,
        }
    }
    
    /// 최소 데이터 요구사항 확인
    async fn meets_min_requirements(&self, data: &BridgePerformanceData) -> bool {
        let min_req = &self.scoring_config.min_data_requirements;
        
        data.total_executions >= min_req.min_executions &&
        data.successful_executions >= min_req.min_successful_transactions &&
        (Utc::now() - data.last_updated).num_hours() <= min_req.min_data_age_hours
    }
    
    /// 라우트별 특수 조정 적용
    async fn apply_route_specific_adjustments(
        &self,
        route_key: &RouteKey,
        bridge: BridgeProtocol,
        base_score: f64,
        amount_usd: f64,
    ) -> f64 {
        let route_configs = self.route_configs.read().await;
        
        if let Some(config) = route_configs.get(route_key) {
            // 블랙리스트 체크
            if config.blacklisted_bridges.contains(&bridge) {
                return 0.0;
            }
            
            // 선호 브리지 보너스
            let preference_bonus = if config.preferred_bridges.contains(&bridge) {
                10.0
            } else {
                0.0
            };
            
            // 특수 요구사항 체크
            let requirement_penalty = self.calculate_requirement_penalty(&config.special_requirements, amount_usd).await;
            
            let adjusted_score = base_score + preference_bonus - requirement_penalty;
            return adjusted_score.max(0.0).min(100.0);
        }
        
        base_score
    }
    
    /// 특수 요구사항 위반에 대한 페널티 계산
    async fn calculate_requirement_penalty(&self, requirements: &[SpecialRequirement], _amount_usd: f64) -> f64 {
        let mut penalty = 0.0;
        
        for requirement in requirements {
            match requirement {
                SpecialRequirement::MaxTime(max_time) => {
                    // 실제 시간과 비교 (구현 필요)
                    if *max_time < 300 { // 5분 미만 요구시
                        penalty += 5.0;
                    }
                }
                SpecialRequirement::MaxCost(max_cost) => {
                    // 실제 비용과 비교 (구현 필요)
                    if *max_cost < 10.0 { // $10 미만 요구시
                        penalty += 3.0;
                    }
                }
                SpecialRequirement::MinSuccessRate(min_rate) => {
                    if *min_rate > 0.95 { // 95% 이상 요구시
                        penalty += 2.0;
                    }
                }
                _ => {}
            }
        }
        
        penalty
    }
    
    /// 라우트 위험도 평가
    async fn assess_route_risk(&self, route_key: &RouteKey, amount_usd: f64) -> RiskLevel {
        // 체인별 기본 위험도
        let source_risk = self.get_chain_risk_level(route_key.source_chain);
        let dest_risk = self.get_chain_risk_level(route_key.dest_chain);
        
        // 금액별 위험도
        let amount_risk = if amount_usd > 100_000.0 {
            RiskLevel::High
        } else if amount_usd > 10_000.0 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };
        
        // 전체 위험도는 가장 높은 수준
        [source_risk, dest_risk, amount_risk].into_iter().max_by_key(|r| match r {
            RiskLevel::Low => 1,
            RiskLevel::Medium => 2,
            RiskLevel::High => 3,
            RiskLevel::Critical => 4,
        }).unwrap_or(RiskLevel::Medium)
    }
    
    /// 체인별 기본 위험도
    fn get_chain_risk_level(&self, chain: ChainId) -> RiskLevel {
        match chain {
            ChainId::Ethereum => RiskLevel::Low,
            ChainId::Polygon => RiskLevel::Low,
            ChainId::Arbitrum => RiskLevel::Low,
            ChainId::Optimism => RiskLevel::Low,
            ChainId::BSC => RiskLevel::Medium,
            ChainId::Avalanche => RiskLevel::Medium,
        }
    }
    
    /// 라우트별 특수 고려사항
    async fn get_route_considerations(&self, route_key: &RouteKey) -> Vec<String> {
        let mut considerations = Vec::new();
        
        // 체인별 특수 고려사항
        if route_key.source_chain == ChainId::Ethereum {
            considerations.push("높은 가스비 고려 필요".to_string());
        }
        
        if route_key.dest_chain == ChainId::Arbitrum {
            considerations.push("L2 최종성 대기 시간 고려".to_string());
        }
        
        // 토큰별 특수 고려사항
        if route_key.token_symbol == "USDC" {
            considerations.push("네이티브 USDC vs 브리지 USDC 확인 필요".to_string());
        }
        
        considerations
    }
    
    /// 시장 조건 업데이트
    pub async fn update_market_conditions(&self, conditions: MarketConditions) -> Result<()> {
        let mut market_conditions = self.market_conditions.write().await;
        *market_conditions = conditions;
        
        info!("📈 시장 조건 업데이트: 변동성 {:.1}%, 혼잡도 {:.1}%", 
              market_conditions.volatility_index * 100.0,
              market_conditions.network_congestion * 100.0);
        
        Ok(())
    }
    
    /// 전체 점수 스냅샷 생성
    pub async fn create_score_snapshot(&self) -> Result<ScoreSnapshot> {
        let mut bridge_scores = HashMap::new();
        let route_scores = HashMap::new(); // 구현 필요
        
        // 모든 브리지에 대해 점수 계산
        for bridge in &[
            BridgeProtocol::Stargate,
            BridgeProtocol::Hop,
            BridgeProtocol::Rubic,
            BridgeProtocol::Synapse,
            BridgeProtocol::LiFi,
            BridgeProtocol::Across,
            BridgeProtocol::Multichain,
        ] {
            if let Ok(score) = self.calculate_bridge_score(bridge.clone()).await {
                bridge_scores.insert(bridge.clone(), score);
            }
        }
        
        let market_conditions = self.market_conditions.read().await.clone();
        
        let snapshot = ScoreSnapshot {
            bridge_scores,
            route_scores,
            market_conditions,
            timestamp: Utc::now(),
        };
        
        // 히스토리에 추가 (최대 100개)
        let mut history = self.score_history.write().await;
        history.push(snapshot.clone());
        if history.len() > 100 {
            history.remove(0);
        }
        
        info!("📸 점수 스냅샷 생성 완료: {} 브리지", snapshot.bridge_scores.len());
        
        Ok(snapshot)
    }
    
    /// 라우트별 특수 설정 등록
    pub async fn register_route_config(&self, route: RouteKey, config: RouteSpecificConfig) -> Result<()> {
        let mut route_configs = self.route_configs.write().await;
        route_configs.insert(route.clone(), config);
        
        info!("⚙️ 라우트 설정 등록: {} -> {}", 
              route.source_chain.name(), route.dest_chain.name());
        
        Ok(())
    }
}

impl Default for MarketConditions {
    fn default() -> Self {
        Self {
            volatility_index: 0.3,
            market_liquidity: U256::from(1_000_000u64),
            avg_gas_price: 20.0,
            network_congestion: 0.5,
            dex_spread_percent: 0.3,
            bridge_utilization: 0.6,
            last_updated: Utc::now(),
        }
    }
}

impl Default for ComponentScores {
    fn default() -> Self {
        Self {
            success_rate_score: 0.0,
            time_efficiency_score: 0.0,
            cost_efficiency_score: 0.0,
            availability_score: 0.0,
            consistency_score: 0.0,
            liquidity_score: 0.0,
            recent_performance_score: 0.0,
            predicted_performance_score: 0.0,
        }
    }
}