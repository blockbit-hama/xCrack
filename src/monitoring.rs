//! Monitoring and metrics collection

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Metrics data structure
#[derive(Debug, Clone)]
pub struct Metrics {
    pub timestamp: DateTime<Utc>,
    pub counters: HashMap<String, u64>,
    pub gauges: HashMap<String, f64>,
    pub histograms: HashMap<String, Vec<f64>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            counters: HashMap::new(),
            gauges: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    pub fn increment_counter(&mut self, name: &str, value: u64) {
        *self.counters.entry(name.to_string()).or_insert(0) += value;
    }

    pub fn set_gauge(&mut self, name: &str, value: f64) {
        self.gauges.insert(name.to_string(), value);
    }

    pub fn add_histogram_value(&mut self, name: &str, value: f64) {
        self.histograms
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(value);
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub total_gas_used: u64,
    pub total_profit: f64,
    pub average_gas_price: f64,
    pub average_execution_time: f64,
    pub last_updated: DateTime<Utc>,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            total_transactions: 0,
            successful_transactions: 0,
            failed_transactions: 0,
            total_gas_used: 0,
            total_profit: 0.0,
            average_gas_price: 0.0,
            average_execution_time: 0.0,
            last_updated: Utc::now(),
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_transactions == 0 {
            0.0
        } else {
            self.successful_transactions as f64 / self.total_transactions as f64
        }
    }

    pub fn profit_per_transaction(&self) -> f64 {
        if self.successful_transactions == 0 {
            0.0
        } else {
            self.total_profit / self.successful_transactions as f64
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// System health status
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// System health check
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub status: HealthStatus,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub details: HashMap<String, String>,
}

impl HealthCheck {
    pub fn new(status: HealthStatus, message: String) -> Self {
        Self {
            status,
            message,
            timestamp: Utc::now(),
            details: HashMap::new(),
        }
    }

    pub fn add_detail(&mut self, key: String, value: String) {
        self.details.insert(key, value);
    }
}

/// Metrics collector
pub struct MetricsCollector {
    metrics: Arc<RwLock<Metrics>>,
    performance: Arc<RwLock<PerformanceMetrics>>,
    health_checks: Arc<RwLock<Vec<HealthCheck>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Metrics::new())),
            performance: Arc::new(RwLock::new(PerformanceMetrics::new())),
            health_checks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.increment_counter(name, value);
    }

    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.set_gauge(name, value);
    }

    pub async fn add_histogram_value(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.add_histogram_value(name, value);
    }

    pub async fn record_transaction(&self, success: bool, gas_used: u64, gas_price: f64, execution_time: f64, profit: f64) {
        let mut perf = self.performance.write().await;
        perf.total_transactions += 1;
        if success {
            perf.successful_transactions += 1;
        } else {
            perf.failed_transactions += 1;
        }
        perf.total_gas_used += gas_used;
        perf.total_profit += profit;
        perf.average_gas_price = (perf.average_gas_price * (perf.total_transactions - 1) as f64 + gas_price) / perf.total_transactions as f64;
        perf.average_execution_time = (perf.average_execution_time * (perf.total_transactions - 1) as f64 + execution_time) / perf.total_transactions as f64;
        perf.last_updated = Utc::now();
    }

    pub async fn add_health_check(&self, health_check: HealthCheck) {
        let mut checks = self.health_checks.write().await;
        checks.push(health_check);
        // Keep only last 100 health checks
        if checks.len() > 100 {
            checks.drain(0..checks.len() - 100);
        }
    }

    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }

    pub async fn get_performance(&self) -> PerformanceMetrics {
        self.performance.read().await.clone()
    }

    pub async fn get_health_checks(&self) -> Vec<HealthCheck> {
        self.health_checks.read().await.clone()
    }

    pub async fn get_overall_health(&self) -> HealthStatus {
        let checks = self.health_checks.read().await;
        if checks.is_empty() {
            return HealthStatus::Unknown;
        }

        let critical_count = checks.iter().filter(|c| c.status == HealthStatus::Critical).count();
        let warning_count = checks.iter().filter(|c| c.status == HealthStatus::Warning).count();

        if critical_count > 0 {
            HealthStatus::Critical
        } else if warning_count > 0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Alert system
pub struct AlertManager {
    alerts: Arc<RwLock<Vec<Alert>>>,
    thresholds: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Arc::new(RwLock::new(Vec::new())),
            thresholds: HashMap::new(),
        }
    }

    pub fn set_threshold(&mut self, metric: String, threshold: f64) {
        self.thresholds.insert(metric, threshold);
    }

    pub async fn check_metric(&self, metric: &str, value: f64) -> Result<()> {
        if let Some(&threshold) = self.thresholds.get(metric) {
            if value > threshold {
                let alert = Alert {
                    id: format!("{}_{}", metric, Utc::now().timestamp()),
                    severity: AlertSeverity::Warning,
                    message: format!("{} exceeded threshold: {} > {}", metric, value, threshold),
                    timestamp: Utc::now(),
                    resolved: false,
                };
                self.add_alert(alert).await;
            }
        }
        Ok(())
    }

    pub async fn add_alert(&self, alert: Alert) {
        let mut alerts = self.alerts.write().await;
        alerts.push(alert);
    }

    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.iter().filter(|a| !a.resolved).cloned().collect()
    }

    pub async fn resolve_alert(&self, alert_id: &str) {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved = true;
        }
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}