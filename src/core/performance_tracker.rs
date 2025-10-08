use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, error, warn};
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

use crate::config::Config;
use crate::types::PerformanceMetrics;
use ethers::types::U256;

pub struct PerformanceTracker {
    config: Arc<Config>,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    detailed_stats: Arc<RwLock<DetailedStats>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    start_time: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedStats {
    pub strategy_performance: HashMap<String, StrategyPerformance>,
    pub bundle_performance: BundlePerformance,
    pub mempool_performance: MempoolPerformance,
    pub network_performance: NetworkPerformance,
    pub error_stats: ErrorStats,
    pub uptime_seconds: u64,
    pub last_update: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub name: String,
    pub transactions_analyzed: u64,
    pub opportunities_found: u64,
    pub opportunities_executed: u64,
    pub total_profit: U256,
    pub avg_analysis_time_ms: f64,
    pub success_rate: f64,
    pub last_activity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePerformance {
    pub bundles_created: u64,
    pub bundles_submitted: u64,
    pub bundles_included: u64,
    pub bundles_failed: u64,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub avg_submission_time_ms: f64,
    pub avg_inclusion_time_ms: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolPerformance {
    pub transactions_received: u64,
    pub transactions_processed: u64,
    pub transactions_filtered: u64,
    pub avg_processing_time_ms: f64,
    pub cache_hit_rate: f64,
    pub last_transaction_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPerformance {
    pub rpc_calls: u64,
    pub rpc_errors: u64,
    pub avg_response_time_ms: f64,
    pub websocket_reconnections: u64,
    pub last_block_number: u64,
    pub block_time_avg_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStats {
    pub total_errors: u64,
    pub errors_by_type: HashMap<String, u64>,
    pub last_error_time: u64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub level: AlertLevel,
    pub message: String,
    pub timestamp: u64,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl PerformanceTracker {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let initial_metrics = PerformanceMetrics {
            transactions_processed: 0,
            opportunities_found: 0,
            bundles_submitted: 0,
            bundles_included: 0,
            total_profit: U256::zero(),
            total_gas_spent: U256::zero(),
            avg_analysis_time: 0.0,
            avg_submission_time: 0.0,
            success_rate: 0.0,
            uptime: 0,
        };
        
        let detailed_stats = DetailedStats {
            strategy_performance: HashMap::new(),
            bundle_performance: BundlePerformance {
                bundles_created: 0,
                bundles_submitted: 0,
                bundles_included: 0,
                bundles_failed: 0,
                total_profit: U256::zero(),
                total_gas_spent: U256::zero(),
                avg_submission_time_ms: 0.0,
                avg_inclusion_time_ms: 0.0,
                success_rate: 0.0,
            },
            mempool_performance: MempoolPerformance {
                transactions_received: 0,
                transactions_processed: 0,
                transactions_filtered: 0,
                avg_processing_time_ms: 0.0,
                cache_hit_rate: 0.0,
                last_transaction_time: 0,
            },
            network_performance: NetworkPerformance {
                rpc_calls: 0,
                rpc_errors: 0,
                avg_response_time_ms: 0.0,
                websocket_reconnections: 0,
                last_block_number: 0,
                block_time_avg_ms: 0.0,
            },
            error_stats: ErrorStats {
                total_errors: 0,
                errors_by_type: HashMap::new(),
                last_error_time: 0,
                error_rate: 0.0,
            },
            uptime_seconds: 0,
            last_update: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        
        Ok(Self {
            config,
            metrics: Arc::new(RwLock::new(initial_metrics)),
            detailed_stats: Arc::new(RwLock::new(detailed_stats)),
            alerts: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
        })
    }

    /// 트랜잭션 처리 통계 업데이트
    pub async fn record_transaction_processed(&self, analysis_time_ms: f64) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let mut detailed = self.detailed_stats.write().await;
        
        metrics.transactions_processed += 1;
        metrics.uptime = self.start_time.elapsed().as_secs();
        
        // 평균 분석 시간 업데이트
        let total_processed = metrics.transactions_processed as f64;
        metrics.avg_analysis_time = (metrics.avg_analysis_time * (total_processed - 1.0) + analysis_time_ms) / total_processed;
        
        // 상세 통계 업데이트
        detailed.mempool_performance.transactions_processed += 1;
        detailed.mempool_performance.avg_processing_time_ms = 
            (detailed.mempool_performance.avg_processing_time_ms * (total_processed - 1.0) + analysis_time_ms) / total_processed;
        
        detailed.last_update = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        Ok(())
    }

    /// 기회 발견 통계 업데이트
    pub async fn record_opportunity_found(&self, strategy_name: &str, profit: U256) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let mut detailed = self.detailed_stats.write().await;
        
        metrics.opportunities_found += 1;
        
        // 전략별 성능 업데이트
        let strategy_stats = detailed.strategy_performance.entry(strategy_name.to_string()).or_insert_with(|| {
            StrategyPerformance {
                name: strategy_name.to_string(),
                transactions_analyzed: 0,
                opportunities_found: 0,
                opportunities_executed: 0,
                total_profit: U256::zero(),
                avg_analysis_time_ms: 0.0,
                success_rate: 0.0,
                last_activity: 0,
            }
        });
        
        strategy_stats.opportunities_found += 1;
        strategy_stats.total_profit += profit;
        strategy_stats.last_activity = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        Ok(())
    }

    /// 번들 제출 통계 업데이트
    pub async fn record_bundle_submitted(&self, submission_time_ms: f64) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let mut detailed = self.detailed_stats.write().await;
        
        metrics.bundles_submitted += 1;
        detailed.bundle_performance.bundles_submitted += 1;
        
        // 평균 제출 시간 업데이트
        let total_submitted = detailed.bundle_performance.bundles_submitted as f64;
        detailed.bundle_performance.avg_submission_time_ms = 
            (detailed.bundle_performance.avg_submission_time_ms * (total_submitted - 1.0) + submission_time_ms) / total_submitted;
        
        metrics.avg_submission_time = detailed.bundle_performance.avg_submission_time_ms;
        
        Ok(())
    }

    /// 번들 포함 통계 업데이트
    pub async fn record_bundle_included(&self, profit: U256, gas_spent: U256, inclusion_time_ms: f64) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let mut detailed = self.detailed_stats.write().await;
        
        metrics.bundles_included += 1;
        metrics.total_profit += profit;
        metrics.total_gas_spent += gas_spent;
        
        detailed.bundle_performance.bundles_included += 1;
        detailed.bundle_performance.total_profit += profit;
        detailed.bundle_performance.total_gas_spent += gas_spent;
        
        // 성공률 업데이트
        if detailed.bundle_performance.bundles_submitted > 0 {
            detailed.bundle_performance.success_rate = 
                detailed.bundle_performance.bundles_included as f64 / detailed.bundle_performance.bundles_submitted as f64;
            metrics.success_rate = detailed.bundle_performance.success_rate;
        }
        
        // 평균 포함 시간 업데이트
        let total_included = detailed.bundle_performance.bundles_included as f64;
        detailed.bundle_performance.avg_inclusion_time_ms = 
            (detailed.bundle_performance.avg_inclusion_time_ms * (total_included - 1.0) + inclusion_time_ms) / total_included;
        
        Ok(())
    }

    /// 번들 실패 통계 업데이트
    pub async fn record_bundle_failed(&self) -> Result<()> {
        let mut detailed = self.detailed_stats.write().await;
        detailed.bundle_performance.bundles_failed += 1;
        
        // 성공률 재계산
        if detailed.bundle_performance.bundles_submitted > 0 {
            detailed.bundle_performance.success_rate = 
                detailed.bundle_performance.bundles_included as f64 / detailed.bundle_performance.bundles_submitted as f64;
        }
        
        Ok(())
    }

    /// RPC 호출 통계 업데이트
    pub async fn record_rpc_call(&self, response_time_ms: f64, success: bool) -> Result<()> {
        let mut detailed = self.detailed_stats.write().await;
        
        detailed.network_performance.rpc_calls += 1;
        
        if !success {
            detailed.network_performance.rpc_errors += 1;
        }
        
        // 평균 응답 시간 업데이트
        let total_calls = detailed.network_performance.rpc_calls as f64;
        detailed.network_performance.avg_response_time_ms = 
            (detailed.network_performance.avg_response_time_ms * (total_calls - 1.0) + response_time_ms) / total_calls;
        
        Ok(())
    }

    /// 에러 통계 업데이트
    pub async fn record_error(&self, error_type: &str, error_message: &str) -> Result<()> {
        let mut detailed = self.detailed_stats.write().await;
        
        detailed.error_stats.total_errors += 1;
        *detailed.error_stats.errors_by_type.entry(error_type.to_string()).or_insert(0) += 1;
        detailed.error_stats.last_error_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // 에러율 계산
        let total_operations = detailed.mempool_performance.transactions_processed + detailed.bundle_performance.bundles_submitted;
        if total_operations > 0 {
            detailed.error_stats.error_rate = detailed.error_stats.total_errors as f64 / total_operations as f64;
        }
        
        // 알림 생성
        self.create_alert(
            AlertLevel::Error,
            &format!("{}: {}", error_type, error_message)
        ).await?;
        
        Ok(())
    }

    /// 알림 생성
    pub async fn create_alert(&self, level: AlertLevel, message: &str) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        
        let alert = Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level,
            message: message.to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            acknowledged: false,
        };
        
        alerts.push(alert);
        
        // 알림 수 제한 (최대 100개)
        if alerts.len() > 100 {
            alerts.remove(0);
        }
        
        // 중요 알림은 로그로 출력
        match level {
            AlertLevel::Critical => error!("🚨 CRITICAL: {}", message),
            AlertLevel::Error => error!("❌ ERROR: {}", message),
            AlertLevel::Warning => warn!("⚠️ WARNING: {}", message),
            AlertLevel::Info => info!("ℹ️ INFO: {}", message),
        }
        
        Ok(())
    }

    /// 성능 메트릭 조회
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let mut metrics = self.metrics.write().await;
        metrics.uptime = self.start_time.elapsed().as_secs();
        metrics.clone()
    }

    /// 상세 통계 조회
    pub async fn get_detailed_stats(&self) -> DetailedStats {
        let mut detailed = self.detailed_stats.write().await;
        detailed.uptime_seconds = self.start_time.elapsed().as_secs();
        detailed.last_update = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        detailed.clone()
    }

    /// 알림 조회
    pub async fn get_alerts(&self, unacknowledged_only: bool) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        
        if unacknowledged_only {
            alerts.iter().filter(|a| !a.acknowledged).cloned().collect()
        } else {
            alerts.clone()
        }
    }

    /// 알림 확인 처리
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
        }
        
        Ok(())
    }

    /// 성능 리포트 생성
    pub async fn generate_performance_report(&self) -> Result<PerformanceReport> {
        let metrics = self.get_metrics().await;
        let detailed = self.get_detailed_stats().await;
        let alerts = self.get_alerts(false).await;
        
        let report = PerformanceReport {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            uptime_seconds: metrics.uptime,
            summary: PerformanceSummary {
                transactions_processed: metrics.transactions_processed,
                opportunities_found: metrics.opportunities_found,
                bundles_submitted: metrics.bundles_submitted,
                bundles_included: metrics.bundles_included,
                total_profit_eth: ethers::utils::format_ether(ethers::types::U256::from_big_endian(&crate::common::abi::u256_to_be_bytes(metrics.total_profit))),
                success_rate: metrics.success_rate,
                avg_analysis_time_ms: metrics.avg_analysis_time,
                avg_submission_time_ms: metrics.avg_submission_time,
            },
            detailed_stats: detailed.clone(),
            recent_alerts: alerts.into_iter().take(10).collect(),
            recommendations: self.generate_recommendations(&metrics, &detailed).await,
        };
        
        Ok(report)
    }

    /// 성능 개선 권장사항 생성
    async fn generate_recommendations(&self, metrics: &PerformanceMetrics, detailed: &DetailedStats) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // 성공률이 낮은 경우
        if metrics.success_rate < 0.5 {
            recommendations.push("번들 성공률이 낮습니다. 가스 가격을 높이거나 번들 크기를 줄여보세요.".to_string());
        }
        
        // 분석 시간이 긴 경우
        if metrics.avg_analysis_time > 100.0 {
            recommendations.push("트랜잭션 분석 시간이 깁니다. 전략 최적화를 고려해보세요.".to_string());
        }
        
        // 에러율이 높은 경우
        if detailed.error_stats.error_rate > 0.1 {
            recommendations.push("에러율이 높습니다. 네트워크 연결과 설정을 확인해보세요.".to_string());
        }
        
        // 수익이 낮은 경우
        if metrics.total_profit < U256::from(1000000000000000000u128) { // 1 ETH 미만
            recommendations.push("총 수익이 낮습니다. 더 수익성 높은 기회를 찾기 위해 전략을 조정해보세요.".to_string());
        }
        
        recommendations
    }

    /// 통계 초기화
    pub async fn reset_stats(&self) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let mut detailed = self.detailed_stats.write().await;
        let mut alerts = self.alerts.write().await;
        
        *metrics = PerformanceMetrics {
            transactions_processed: 0,
            opportunities_found: 0,
            bundles_submitted: 0,
            bundles_included: 0,
            total_profit: U256::zero(),
            total_gas_spent: U256::zero(),
            avg_analysis_time: 0.0,
            avg_submission_time: 0.0,
            success_rate: 0.0,
            uptime: 0,
        };
        
        *detailed = DetailedStats {
            strategy_performance: HashMap::new(),
            bundle_performance: BundlePerformance {
                bundles_created: 0,
                bundles_submitted: 0,
                bundles_included: 0,
                bundles_failed: 0,
                total_profit: U256::zero(),
                total_gas_spent: U256::zero(),
                avg_submission_time_ms: 0.0,
                avg_inclusion_time_ms: 0.0,
                success_rate: 0.0,
            },
            mempool_performance: MempoolPerformance {
                transactions_received: 0,
                transactions_processed: 0,
                transactions_filtered: 0,
                avg_processing_time_ms: 0.0,
                cache_hit_rate: 0.0,
                last_transaction_time: 0,
            },
            network_performance: NetworkPerformance {
                rpc_calls: 0,
                rpc_errors: 0,
                avg_response_time_ms: 0.0,
                websocket_reconnections: 0,
                last_block_number: 0,
                block_time_avg_ms: 0.0,
            },
            error_stats: ErrorStats {
                total_errors: 0,
                errors_by_type: HashMap::new(),
                last_error_time: 0,
                error_rate: 0.0,
            },
            uptime_seconds: 0,
            last_update: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        
        alerts.clear();
        
        info!("📊 성능 통계가 초기화되었습니다");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub summary: PerformanceSummary,
    pub detailed_stats: DetailedStats,
    pub recent_alerts: Vec<Alert>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub transactions_processed: u64,
    pub opportunities_found: u64,
    pub bundles_submitted: u64,
    pub bundles_included: u64,
    pub total_profit_eth: String,
    pub success_rate: f64,
    pub avg_analysis_time_ms: f64,
    pub avg_submission_time_ms: f64,
}

impl std::fmt::Debug for PerformanceTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerformanceTracker")
            .field("config", &"Arc<Config>")
            .field("start_time", &self.start_time)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use ethers::types::U256;

    #[tokio::test]
    async fn test_performance_tracker_creation() {
        let config = Arc::new(Config::default());
        let tracker = PerformanceTracker::new(config).await;
        assert!(tracker.is_ok());
    }

    #[tokio::test]
    async fn test_metrics_recording() {
        let config = Arc::new(Config::default());
        let tracker = PerformanceTracker::new(config).await.unwrap();
        
        // 트랜잭션 처리 기록
        tracker.record_transaction_processed(50.0).await.unwrap();
        
        // 기회 발견 기록
        tracker.record_opportunity_found("arbitrage", U256::from(1000000000000000000u128)).await.unwrap();
        
        // 메트릭 조회
        let metrics = tracker.get_metrics().await;
        assert_eq!(metrics.transactions_processed, 1);
        assert_eq!(metrics.opportunities_found, 1);
    }

    #[tokio::test]
    async fn test_alert_creation() {
        let config = Arc::new(Config::default());
        let tracker = PerformanceTracker::new(config).await.unwrap();
        
        // 알림 생성
        tracker.create_alert(AlertLevel::Warning, "테스트 알림").await.unwrap();
        
        // 알림 조회
        let alerts = tracker.get_alerts(false).await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].message, "테스트 알림");
        assert_eq!(alerts[0].level, AlertLevel::Warning);
    }
} 