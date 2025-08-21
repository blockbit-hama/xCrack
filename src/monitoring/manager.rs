use std::sync::Arc;
use anyhow::Result;
use axum::{routing::get, Router};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use crate::config::Config;
use crate::core::performance_tracker::PerformanceTracker;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct MonitoringManager {
    config: Arc<Config>,
    tracker: Arc<PerformanceTracker>,
}

impl MonitoringManager {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let tracker = Arc::new(PerformanceTracker::new(Arc::clone(&config)).await?);
        Ok(Self { config, tracker })
    }

    pub async fn start(&self) -> Result<()> {
        let tracker = Arc::clone(&self.tracker);
        let api_tracker = Arc::clone(&self.tracker);
        let app = Router::new()
            .route("/metrics", get(move || metrics_handler(Arc::clone(&tracker))))
            .route("/health", get(|| async { Json(serde_json::json!({"ok": true})) }))
            .route("/status", get(move || status_handler(Arc::clone(&api_tracker))));

        let port = self.config.monitoring.metrics_port;
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("📈 Metrics server listening on http://{}", addr);

        tokio::spawn(async move {
            if let Err(e) = axum::Server::bind(&addr).serve(app.into_make_service()).await {
                tracing::error!("Metrics server error: {}", e);
            }
        });
        Ok(())
    }
}

#[derive(Serialize)]
struct MetricsJson {
    transactions_processed: u64,
    opportunities_found: u64,
    bundles_submitted: u64,
    bundles_included: u64,
    total_profit_eth: String,
    success_rate: f64,
    avg_analysis_time_ms: f64,
    avg_submission_time_ms: f64,
}

async fn metrics_handler(tracker: Arc<PerformanceTracker>) -> impl IntoResponse {
    let metrics = tracker.get_metrics().await;
    let body = MetricsJson {
        transactions_processed: metrics.transactions_processed,
        opportunities_found: metrics.opportunities_found,
        bundles_submitted: metrics.bundles_submitted,
        bundles_included: metrics.bundles_included,
        total_profit_eth: ethers::utils::format_ether(ethers::types::U256::from_big_endian(&metrics.total_profit.to_be_bytes::<32>())),
        success_rate: metrics.success_rate,
        avg_analysis_time_ms: metrics.avg_analysis_time,
        avg_submission_time_ms: metrics.avg_submission_time,
    };
    Json(body)
}

#[derive(Serialize)]
struct StatusJson {
    uptime_seconds: u64,
    bundles_submitted: u64,
    bundles_included: u64,
}

async fn status_handler(tracker: Arc<PerformanceTracker>) -> impl IntoResponse {
    let m = tracker.get_metrics().await;
    Json(StatusJson {
        uptime_seconds: m.uptime,
        bundles_submitted: m.bundles_submitted,
        bundles_included: m.bundles_included,
    })
}