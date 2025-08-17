use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use tracing::{info, debug, warn, error};
use chrono::Timelike;
use serde::{Serialize, Deserialize};
use alloy::primitives::U256;

use crate::types::{ChainId, BridgeProtocol};

/// 브리지 성능 추적 시스템
/// 
/// 실시간으로 각 브리지의 성능 지표를 추적하고 분석합니다.
/// - 성공률, 지연시간, 비용 추적
/// - 체인별, 토큰별 성능 분석
/// - 시간대별 성능 패턴 분석
#[derive(Debug)]
pub struct BridgePerformanceTracker {
    /// 브리지별 성능 데이터
    bridge_metrics: Arc<RwLock<HashMap<BridgeProtocol, BridgePerformanceData>>>,
    
    /// 체인 페어별 성능 데이터
    route_metrics: Arc<RwLock<HashMap<RouteKey, RoutePerformanceData>>>,
    
    /// 실행 히스토리 (최근 1000개)
    execution_history: Arc<RwLock<Vec<BridgeExecution>>>,
    
    /// 성능 임계값 설정
    thresholds: PerformanceThresholds,
    
    /// 통계 계산 설정
    stats_config: StatsConfig,
}

/// 라우트 키 (소스체인 -> 대상체인)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RouteKey {
    pub source_chain: ChainId,
    pub dest_chain: ChainId,
    pub token_symbol: String,
}

/// 브리지별 성능 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgePerformanceData {
    /// 브리지 프로토콜
    pub protocol: BridgeProtocol,
    
    /// 총 실행 횟수
    pub total_executions: u64,
    
    /// 성공한 실행 횟수
    pub successful_executions: u64,
    
    /// 실패한 실행 횟수
    pub failed_executions: u64,
    
    /// 현재 성공률 (0.0 ~ 1.0)
    pub success_rate: f64,
    
    /// 평균 완료 시간 (초)
    pub avg_completion_time: f64,
    
    /// 최소 완료 시간 (초)
    pub min_completion_time: f64,
    
    /// 최대 완료 시간 (초)
    pub max_completion_time: f64,
    
    /// 평균 비용 (USD)
    pub avg_cost_usd: f64,
    
    /// 최소 비용 (USD)
    pub min_cost_usd: f64,
    
    /// 최대 비용 (USD)
    pub max_cost_usd: f64,
    
    /// 평균 슬리피지 (%)
    pub avg_slippage_percent: f64,
    
    /// 최근 24시간 가용률
    pub availability_24h: f64,
    
    /// 신뢰도 점수 (0.0 ~ 1.0)
    pub reliability_score: f64,
    
    /// 최근 업데이트 시간
    pub last_updated: DateTime<Utc>,
    
    /// 시간대별 성능 데이터 (24시간)
    pub hourly_stats: Vec<HourlyStats>,
}

/// 라우트별 성능 데이터
#[derive(Debug, Clone)]
pub struct RoutePerformanceData {
    /// 라우트 키
    pub route: RouteKey,
    
    /// 브리지별 성과
    pub bridge_performance: HashMap<BridgeProtocol, RouteBridgeStats>,
    
    /// 최적 브리지 (현재 기준)
    pub best_bridge: Option<BridgeProtocol>,
    
    /// 라우트 전체 성공률
    pub overall_success_rate: f64,
    
    /// 평균 완료 시간
    pub avg_completion_time: f64,
    
    /// 평균 비용
    pub avg_cost_usd: f64,
    
    /// 최근 업데이트
    pub last_updated: DateTime<Utc>,
}

/// 라우트-브리지별 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBridgeStats {
    pub executions: u64,
    pub successes: u64,
    pub success_rate: f64,
    pub avg_time: f64,
    pub avg_cost: f64,
    pub avg_slippage: f64,
    pub last_execution: Option<DateTime<Utc>>,
}

/// 시간대별 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyStats {
    pub hour: u8, // 0-23
    pub executions: u64,
    pub success_rate: f64,
    pub avg_completion_time: f64,
    pub avg_cost: f64,
}

/// 브리지 실행 기록
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeExecution {
    /// 실행 ID
    pub execution_id: String,
    
    /// 브리지 프로토콜
    pub bridge: BridgeProtocol,
    
    /// 소스 체인
    pub source_chain: ChainId,
    
    /// 대상 체인
    pub dest_chain: ChainId,
    
    /// 토큰 심볼
    pub token_symbol: String,
    
    /// 거래 금액 (토큰 단위)
    pub amount: U256,
    
    /// 거래 금액 (USD)
    pub amount_usd: f64,
    
    /// 실행 시작 시간
    pub started_at: DateTime<Utc>,
    
    /// 실행 완료 시간
    pub completed_at: Option<DateTime<Utc>>,
    
    /// 실행 상태
    pub status: ExecutionStatus,
    
    /// 실제 완료 시간 (초)
    pub actual_time: Option<f64>,
    
    /// 예상 완료 시간 (초)
    pub estimated_time: f64,
    
    /// 실제 비용 (USD)
    pub actual_cost: Option<f64>,
    
    /// 예상 비용 (USD)
    pub estimated_cost: f64,
    
    /// 실제 슬리피지 (%)
    pub actual_slippage: Option<f64>,
    
    /// 예상 슬리피지 (%)
    pub estimated_slippage: f64,
    
    /// 오류 메시지 (실패 시)
    pub error_message: Option<String>,
    
    /// 트랜잭션 해시들
    pub transaction_hashes: Vec<String>,
}

/// 실행 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    /// 진행 중
    InProgress,
    /// 성공 완료
    Completed,
    /// 실패
    Failed,
    /// 타임아웃
    Timeout,
    /// 취소됨
    Cancelled,
}

/// 성능 임계값 설정
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    /// 최소 허용 성공률
    pub min_success_rate: f64,
    
    /// 최대 허용 완료 시간 (초)
    pub max_completion_time: f64,
    
    /// 최대 허용 비용 (USD)
    pub max_cost_usd: f64,
    
    /// 최대 허용 슬리피지 (%)
    pub max_slippage_percent: f64,
    
    /// 최소 가용률
    pub min_availability: f64,
}

/// 통계 계산 설정
#[derive(Debug, Clone)]
pub struct StatsConfig {
    /// 성능 평가 윈도우 (시간)
    pub evaluation_window_hours: i64,
    
    /// 최소 실행 횟수 (통계 유효성)
    pub min_executions_for_stats: u64,
    
    /// 신뢰도 계산 가중치
    pub reliability_weights: ReliabilityWeights,
}

/// 신뢰도 계산 가중치
#[derive(Debug, Clone)]
pub struct ReliabilityWeights {
    pub success_rate: f64,     // 성공률 가중치
    pub completion_time: f64,  // 완료 시간 가중치
    pub cost: f64,            // 비용 가중치
    pub availability: f64,     // 가용률 가중치
    pub consistency: f64,      // 일관성 가중치
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            min_success_rate: 0.95,      // 95% 이상
            max_completion_time: 600.0,   // 10분 이하
            max_cost_usd: 50.0,          // $50 이하
            max_slippage_percent: 1.0,    // 1% 이하
            min_availability: 0.98,       // 98% 이상
        }
    }
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            evaluation_window_hours: 24,  // 24시간 윈도우
            min_executions_for_stats: 10, // 최소 10회 실행
            reliability_weights: ReliabilityWeights {
                success_rate: 0.4,     // 40%
                completion_time: 0.2,  // 20%
                cost: 0.15,           // 15%
                availability: 0.15,    // 15%
                consistency: 0.1,      // 10%
            },
        }
    }
}

impl BridgePerformanceTracker {
    /// 새로운 성능 추적기 생성
    pub fn new() -> Self {
        Self {
            bridge_metrics: Arc::new(RwLock::new(HashMap::new())),
            route_metrics: Arc::new(RwLock::new(HashMap::new())),
            execution_history: Arc::new(RwLock::new(Vec::new())),
            thresholds: PerformanceThresholds::default(),
            stats_config: StatsConfig::default(),
        }
    }
    
    /// 커스텀 설정으로 생성
    pub fn with_config(thresholds: PerformanceThresholds, stats_config: StatsConfig) -> Self {
        Self {
            bridge_metrics: Arc::new(RwLock::new(HashMap::new())),
            route_metrics: Arc::new(RwLock::new(HashMap::new())),
            execution_history: Arc::new(RwLock::new(Vec::new())),
            thresholds,
            stats_config,
        }
    }
    
    /// 브리지 실행 시작 기록
    pub async fn record_execution_start(
        &self,
        execution_id: String,
        bridge: BridgeProtocol,
        source_chain: ChainId,
        dest_chain: ChainId,
        token_symbol: String,
        amount: U256,
        amount_usd: f64,
        estimated_time: f64,
        estimated_cost: f64,
        estimated_slippage: f64,
    ) -> Result<()> {
        let execution = BridgeExecution {
            execution_id: execution_id.clone(),
            bridge: bridge.clone(),
            source_chain,
            dest_chain,
            token_symbol,
            amount,
            amount_usd,
            started_at: Utc::now(),
            completed_at: None,
            status: ExecutionStatus::InProgress,
            actual_time: None,
            estimated_time,
            actual_cost: None,
            estimated_cost,
            actual_slippage: None,
            estimated_slippage,
            error_message: None,
            transaction_hashes: Vec::new(),
        };
        
        let mut history = self.execution_history.write().await;
        history.push(execution);
        
        // 최대 1000개 히스토리 유지
        if history.len() > 1000 {
            history.remove(0);
        }
        
        let bridge_for_log = bridge.clone();
        info!("🚀 브리지 실행 시작 기록: {} via {}", execution_id, bridge_for_log.name());
        Ok(())
    }
    
    /// 브리지 실행 완료 기록
    pub async fn record_execution_completion(
        &self,
        execution_id: String,
        status: ExecutionStatus,
        actual_cost: Option<f64>,
        actual_slippage: Option<f64>,
        error_message: Option<String>,
        transaction_hashes: Vec<String>,
    ) -> Result<()> {
        let mut history = self.execution_history.write().await;
        
        // 해당 실행 찾기
        if let Some(execution) = history.iter_mut().find(|e| e.execution_id == execution_id) {
            let completed_at = Utc::now();
            let actual_time = (completed_at - execution.started_at).num_milliseconds() as f64 / 1000.0;
            
            execution.completed_at = Some(completed_at);
            execution.status = status.clone();
            execution.actual_time = Some(actual_time);
            execution.actual_cost = actual_cost;
            execution.actual_slippage = actual_slippage;
            execution.error_message = error_message.clone();
            execution.transaction_hashes = transaction_hashes;
            
            // 성능 메트릭 업데이트
            self.update_bridge_metrics(&execution).await?;
            self.update_route_metrics(&execution).await?;
            
            match status {
                ExecutionStatus::Completed => {
                    info!("✅ 브리지 실행 완료: {} ({:.1}s, ${:.2})", 
                          execution_id, actual_time, actual_cost.unwrap_or(0.0));
                }
                ExecutionStatus::Failed => {
                    warn!("❌ 브리지 실행 실패: {} - {}", 
                          execution_id, error_message.unwrap_or_default());
                }
                ExecutionStatus::Timeout => {
                    warn!("⏰ 브리지 실행 타임아웃: {}", execution_id);
                }
                _ => {
                    debug!("📝 브리지 실행 상태 업데이트: {} -> {:?}", execution_id, status);
                }
            }
        } else {
            warn!("⚠️ 실행 ID를 찾을 수 없음: {}", execution_id);
        }
        
        Ok(())
    }
    
    /// 브리지별 성능 메트릭 업데이트
    async fn update_bridge_metrics(&self, execution: &BridgeExecution) -> Result<()> {
        let mut metrics = self.bridge_metrics.write().await;
        
        let bridge_data = metrics.entry(execution.bridge.clone())
            .or_insert_with(|| BridgePerformanceData::new(execution.bridge.clone()));
        
        // 기본 카운터 업데이트
        bridge_data.total_executions += 1;
        
        match execution.status {
            ExecutionStatus::Completed => {
                bridge_data.successful_executions += 1;
                
                if let (Some(actual_time), Some(actual_cost)) = (execution.actual_time, execution.actual_cost) {
                    // 완료 시간 통계 업데이트
                    if bridge_data.successful_executions == 1 {
                        bridge_data.avg_completion_time = actual_time;
                        bridge_data.min_completion_time = actual_time;
                        bridge_data.max_completion_time = actual_time;
                    } else {
                        let prev_avg = bridge_data.avg_completion_time;
                        let count = bridge_data.successful_executions as f64;
                        bridge_data.avg_completion_time = (prev_avg * (count - 1.0) + actual_time) / count;
                        bridge_data.min_completion_time = bridge_data.min_completion_time.min(actual_time);
                        bridge_data.max_completion_time = bridge_data.max_completion_time.max(actual_time);
                    }
                    
                    // 비용 통계 업데이트
                    if bridge_data.successful_executions == 1 {
                        bridge_data.avg_cost_usd = actual_cost;
                        bridge_data.min_cost_usd = actual_cost;
                        bridge_data.max_cost_usd = actual_cost;
                    } else {
                        let prev_avg = bridge_data.avg_cost_usd;
                        let count = bridge_data.successful_executions as f64;
                        bridge_data.avg_cost_usd = (prev_avg * (count - 1.0) + actual_cost) / count;
                        bridge_data.min_cost_usd = bridge_data.min_cost_usd.min(actual_cost);
                        bridge_data.max_cost_usd = bridge_data.max_cost_usd.max(actual_cost);
                    }
                    
                    // 슬리피지 통계 업데이트
                    if let Some(actual_slippage) = execution.actual_slippage {
                        let prev_avg = bridge_data.avg_slippage_percent;
                        let count = bridge_data.successful_executions as f64;
                        bridge_data.avg_slippage_percent = (prev_avg * (count - 1.0) + actual_slippage) / count;
                    }
                }
            }
            ExecutionStatus::Failed | ExecutionStatus::Timeout => {
                bridge_data.failed_executions += 1;
            }
            _ => {}
        }
        
        // 성공률 계산
        bridge_data.success_rate = if bridge_data.total_executions > 0 {
            bridge_data.successful_executions as f64 / bridge_data.total_executions as f64
        } else {
            0.0
        };
        
        // 신뢰도 점수 재계산
        bridge_data.reliability_score = self.calculate_reliability_score(bridge_data).await;
        
        // 시간대별 통계 업데이트
        self.update_hourly_stats(bridge_data, execution).await;
        
        bridge_data.last_updated = Utc::now();
        
        Ok(())
    }
    
    /// 라우트별 성능 메트릭 업데이트
    async fn update_route_metrics(&self, execution: &BridgeExecution) -> Result<()> {
        let mut metrics = self.route_metrics.write().await;
        
        let route_key = RouteKey {
            source_chain: execution.source_chain,
            dest_chain: execution.dest_chain,
            token_symbol: execution.token_symbol.clone(),
        };
        
        let route_data = metrics.entry(route_key.clone())
            .or_insert_with(|| RoutePerformanceData::new(route_key));
        
        // 브리지별 통계 업데이트
        let bridge_stats = route_data.bridge_performance
            .entry(execution.bridge.clone())
            .or_insert_with(RouteBridgeStats::default);
        
        bridge_stats.executions += 1;
        bridge_stats.last_execution = Some(execution.started_at);
        
        if execution.status == ExecutionStatus::Completed {
            bridge_stats.successes += 1;
            
            if let (Some(actual_time), Some(actual_cost), Some(actual_slippage)) = 
                (execution.actual_time, execution.actual_cost, execution.actual_slippage) {
                
                // 통계 업데이트
                let count = bridge_stats.successes as f64;
                bridge_stats.avg_time = (bridge_stats.avg_time * (count - 1.0) + actual_time) / count;
                bridge_stats.avg_cost = (bridge_stats.avg_cost * (count - 1.0) + actual_cost) / count;
                bridge_stats.avg_slippage = (bridge_stats.avg_slippage * (count - 1.0) + actual_slippage) / count;
            }
        }
        
        bridge_stats.success_rate = if bridge_stats.executions > 0 {
            bridge_stats.successes as f64 / bridge_stats.executions as f64
        } else {
            0.0
        };
        
        // 라우트 전체 통계 재계산
        self.recalculate_route_stats(route_data).await;
        
        route_data.last_updated = Utc::now();
        
        Ok(())
    }
    
    /// 신뢰도 점수 계산
    async fn calculate_reliability_score(&self, data: &BridgePerformanceData) -> f64 {
        let weights = &self.stats_config.reliability_weights;
        let thresholds = &self.thresholds;
        
        // 성공률 점수 (0-1)
        let success_score = (data.success_rate / thresholds.min_success_rate).min(1.0);
        
        // 완료 시간 점수 (역비례, 빠를수록 높은 점수)
        let time_score = if data.avg_completion_time > 0.0 {
            (thresholds.max_completion_time / data.avg_completion_time).min(1.0)
        } else {
            0.0
        };
        
        // 비용 점수 (역비례, 저렴할수록 높은 점수)
        let cost_score = if data.avg_cost_usd > 0.0 {
            (thresholds.max_cost_usd / data.avg_cost_usd).min(1.0)
        } else {
            1.0
        };
        
        // 가용률 점수 (24시간 기준)
        let availability_score = (data.availability_24h / thresholds.min_availability).min(1.0);
        
        // 일관성 점수 (시간 편차가 작을수록 높은 점수)
        let time_variance = data.max_completion_time - data.min_completion_time;
        let consistency_score = if time_variance > 0.0 {
            (1.0 - (time_variance / data.avg_completion_time).min(1.0)).max(0.0)
        } else {
            1.0
        };
        
        // 가중 평균 계산
        let score = success_score * weights.success_rate +
                    time_score * weights.completion_time +
                    cost_score * weights.cost +
                    availability_score * weights.availability +
                    consistency_score * weights.consistency;
        
        score.min(1.0).max(0.0)
    }
    
    /// 시간대별 통계 업데이트
    async fn update_hourly_stats(&self, data: &mut BridgePerformanceData, execution: &BridgeExecution) {
        let hour = execution.started_at.hour() as u8;
        
        // 시간대별 통계 배열 초기화 (24시간)
        if data.hourly_stats.len() != 24 {
            data.hourly_stats = (0..24).map(|h| HourlyStats {
                hour: h,
                executions: 0,
                success_rate: 0.0,
                avg_completion_time: 0.0,
                avg_cost: 0.0,
            }).collect();
        }
        
        let hourly_stat = &mut data.hourly_stats[hour as usize];
        hourly_stat.executions += 1;
        
        if execution.status == ExecutionStatus::Completed {
            if let (Some(actual_time), Some(actual_cost)) = (execution.actual_time, execution.actual_cost) {
                let count = hourly_stat.executions as f64;
                hourly_stat.avg_completion_time = (hourly_stat.avg_completion_time * (count - 1.0) + actual_time) / count;
                hourly_stat.avg_cost = (hourly_stat.avg_cost * (count - 1.0) + actual_cost) / count;
            }
        }
        
        // 해당 시간대의 성공률 재계산 (최근 데이터 기준)
        let recent_executions = self.get_hourly_executions(data.protocol.clone(), hour).await;
        let successful_count = recent_executions.iter()
            .filter(|e| e.status == ExecutionStatus::Completed)
            .count();
        
        hourly_stat.success_rate = if recent_executions.len() > 0 {
            successful_count as f64 / recent_executions.len() as f64
        } else {
            0.0
        };
    }
    
    /// 특정 시간대의 실행 기록 조회
    async fn get_hourly_executions(&self, bridge: BridgeProtocol, hour: u8) -> Vec<BridgeExecution> {
        let history = self.execution_history.read().await;
        let cutoff = Utc::now() - ChronoDuration::hours(24);
        
        history.iter()
            .filter(|e| e.bridge == bridge && 
                       e.started_at > cutoff && 
                       e.started_at.hour() as u8 == hour)
            .cloned()
            .collect()
    }
    
    /// 라우트 전체 통계 재계산
    async fn recalculate_route_stats(&self, route_data: &mut RoutePerformanceData) {
        let mut total_executions = 0u64;
        let mut total_successes = 0u64;
        let mut total_time = 0.0;
        let mut total_cost = 0.0;
        let mut best_score = 0.0;
        let mut best_bridge = None;
        
        for (bridge, stats) in &route_data.bridge_performance {
            total_executions += stats.executions;
            total_successes += stats.successes;
            
            if stats.successes > 0 {
                total_time += stats.avg_time * stats.successes as f64;
                total_cost += stats.avg_cost * stats.successes as f64;
                
                // 브리지별 점수 계산 (성공률 + 시간 + 비용 종합)
                let score = stats.success_rate * 0.5 + 
                           (1.0 / (1.0 + stats.avg_time / 300.0)) * 0.3 + // 5분 기준 정규화
                           (1.0 / (1.0 + stats.avg_cost / 10.0)) * 0.2;   // $10 기준 정규화
                
                if score > best_score {
                    best_score = score;
                    best_bridge = Some(bridge.clone());
                }
            }
        }
        
        route_data.overall_success_rate = if total_executions > 0 {
            total_successes as f64 / total_executions as f64
        } else {
            0.0
        };
        
        route_data.avg_completion_time = if total_successes > 0 {
            total_time / total_successes as f64
        } else {
            0.0
        };
        
        route_data.avg_cost_usd = if total_successes > 0 {
            total_cost / total_successes as f64
        } else {
            0.0
        };
        
        route_data.best_bridge = best_bridge;
    }
    
    /// 브리지 성능 데이터 조회
    pub async fn get_bridge_performance(&self, bridge: BridgeProtocol) -> Option<BridgePerformanceData> {
        let metrics = self.bridge_metrics.read().await;
        metrics.get(&bridge).cloned()
    }
    
    /// 모든 브리지 성능 데이터 조회
    pub async fn get_all_bridge_performance(&self) -> HashMap<BridgeProtocol, BridgePerformanceData> {
        let metrics = self.bridge_metrics.read().await;
        metrics.clone()
    }
    
    /// 라우트 성능 데이터 조회
    pub async fn get_route_performance(&self, route: RouteKey) -> Option<RoutePerformanceData> {
        let metrics = self.route_metrics.read().await;
        metrics.get(&route).cloned()
    }
    
    /// 최적 브리지 추천
    pub async fn recommend_best_bridge(
        &self,
        source_chain: ChainId,
        dest_chain: ChainId,
        token_symbol: String,
    ) -> Option<BridgeProtocol> {
        let route_key = RouteKey {
            source_chain,
            dest_chain,
            token_symbol,
        };
        
        let metrics = self.route_metrics.read().await;
        if let Some(route_data) = metrics.get(&route_key) {
            route_data.best_bridge.clone()
        } else {
            // 라우트별 데이터가 없으면 전체 브리지 성능 기준
            let bridge_metrics = self.bridge_metrics.read().await;
            let mut best_bridge = None;
            let mut best_score = 0.0;
            
            for (bridge, data) in bridge_metrics.iter() {
                if data.reliability_score > best_score && 
                   data.total_executions >= self.stats_config.min_executions_for_stats {
                    best_score = data.reliability_score;
                    best_bridge = Some(bridge.clone());
                }
            }
            
            best_bridge
        }
    }
    
    /// 실행 히스토리 조회
    pub async fn get_execution_history(&self, limit: usize) -> Vec<BridgeExecution> {
        let history = self.execution_history.read().await;
        let start = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };
        history[start..].to_vec()
    }
    
    /// 성능 알림 확인
    pub async fn check_performance_alerts(&self) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();
        let bridge_metrics = self.bridge_metrics.read().await;
        
        for (bridge, data) in bridge_metrics.iter() {
            // 성공률 임계값 체크
            if data.success_rate < self.thresholds.min_success_rate {
                alerts.push(PerformanceAlert {
                    bridge: bridge.clone(),
                    alert_type: AlertType::LowSuccessRate,
                    message: format!(
                        "브리지 {} 성공률이 임계값 이하입니다: {:.1}% < {:.1}%",
                        bridge.name(),
                        data.success_rate * 100.0,
                        self.thresholds.min_success_rate * 100.0
                    ),
                    severity: AlertSeverity::High,
                    timestamp: Utc::now(),
                });
            }
            
            // 완료 시간 임계값 체크
            if data.avg_completion_time > self.thresholds.max_completion_time {
                alerts.push(PerformanceAlert {
                    bridge: bridge.clone(),
                    alert_type: AlertType::SlowCompletion,
                    message: format!(
                        "브리지 {} 완료 시간이 임계값 초과: {:.1}s > {:.1}s",
                        bridge.name(),
                        data.avg_completion_time,
                        self.thresholds.max_completion_time
                    ),
                    severity: AlertSeverity::Medium,
                    timestamp: Utc::now(),
                });
            }
            
            // 비용 임계값 체크
            if data.avg_cost_usd > self.thresholds.max_cost_usd {
                alerts.push(PerformanceAlert {
                    bridge: bridge.clone(),
                    alert_type: AlertType::HighCost,
                    message: format!(
                        "브리지 {} 비용이 임계값 초과: ${:.2} > ${:.2}",
                        bridge.name(),
                        data.avg_cost_usd,
                        self.thresholds.max_cost_usd
                    ),
                    severity: AlertSeverity::Low,
                    timestamp: Utc::now(),
                });
            }
        }
        
        alerts
    }
}

/// 성능 알림
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub bridge: BridgeProtocol,
    pub alert_type: AlertType,
    pub message: String,
    pub severity: AlertSeverity,
    pub timestamp: DateTime<Utc>,
}

/// 알림 타입
#[derive(Debug, Clone)]
pub enum AlertType {
    LowSuccessRate,
    SlowCompletion,
    HighCost,
    HighSlippage,
    LowAvailability,
}

/// 알림 심각도
#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl BridgePerformanceData {
    fn new(protocol: BridgeProtocol) -> Self {
        Self {
            protocol,
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            success_rate: 0.0,
            avg_completion_time: 0.0,
            min_completion_time: f64::MAX,
            max_completion_time: 0.0,
            avg_cost_usd: 0.0,
            min_cost_usd: f64::MAX,
            max_cost_usd: 0.0,
            avg_slippage_percent: 0.0,
            availability_24h: 1.0, // 기본값 100%
            reliability_score: 0.0,
            last_updated: Utc::now(),
            hourly_stats: Vec::new(),
        }
    }
}

impl RoutePerformanceData {
    fn new(route: RouteKey) -> Self {
        Self {
            route,
            bridge_performance: HashMap::new(),
            best_bridge: None,
            overall_success_rate: 0.0,
            avg_completion_time: 0.0,
            avg_cost_usd: 0.0,
            last_updated: Utc::now(),
        }
    }
}

impl Default for RouteBridgeStats {
    fn default() -> Self {
        Self {
            executions: 0,
            successes: 0,
            success_rate: 0.0,
            avg_time: 0.0,
            avg_cost: 0.0,
            avg_slippage: 0.0,
            last_execution: None,
        }
    }
}