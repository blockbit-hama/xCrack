use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::{get, post}, Json, Router};
use tower_http::cors::{Any, CorsLayer};
use axum::response::sse::{Sse, Event};
use futures_util::stream::{Stream, StreamExt};
use serde::Serialize;

use crate::config::Config;
use crate::core::SearcherCore;
use crate::core::bundle_manager::BundleStats;
use crate::core::performance_tracker::PerformanceReport;

#[derive(Clone)]
pub struct ApiServer {
    config: Arc<Config>,
    core: SearcherCore,
}

impl ApiServer {
    pub fn new(config: Arc<Config>, core: SearcherCore) -> Self {
        Self { config, core }
    }

    pub async fn start(&self) -> Result<()> {
        let core_status = self.core.clone();
        let core_strategies = self.core.clone();
        let core_toggle = self.core.clone();
        let core_bundles_list = self.core.clone();
        let core_bundles_detail = self.core.clone();
        let core_report = self.core.clone();
        let core_logs = self.core.clone();
        let core_stats = self.core.clone();

        let config_for_settings = Arc::clone(&self.config);
        let core_for_settings_get = self.core.clone();
        let core_for_settings_post = self.core.clone();

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/api/health", get(|| async { Json(serde_json::json!({"ok": true})) }))
            .route("/api/status", get(move || get_status(core_status.clone())))
            .route("/api/strategies", get(move || get_strategies(core_strategies.clone())))
            .route("/api/strategies/toggle", post(move |payload| toggle_strategy(core_toggle.clone(), payload)))
            .route("/api/strategies/stats", get(move || get_strategy_stats(core_stats.clone())))
            .route("/api/bundles", get(move || get_bundles(core_bundles_list.clone())))
            .route("/api/bundles/:id", get(move |axum::extract::Path(id): axum::extract::Path<String>| get_bundle_by_id(core_bundles_detail.clone(), id)))
            .route("/api/report", get(move || get_report(core_report.clone())))
            .route("/api/stream/logs", get(move || sse_logs(core_logs.clone())))
            .route("/api/settings", get(move || get_settings(Arc::clone(&config_for_settings), core_for_settings_get.clone())))
            .route("/api/settings", post(move |payload| post_settings(core_for_settings_post.clone(), payload)))
            .layer(cors);

        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.monitoring.api_port));
        tracing::info!("🛰️ API server listening on http://{}", addr);

        tokio::spawn(async move {
            if let Err(e) = axum::Server::bind(&addr).serve(app.into_make_service()).await {
                tracing::error!("API server error: {}", e);
            }
        });

        Ok(())
    }
}

#[derive(Serialize)]
struct StatusResponse {
    is_running: bool,
    active_opportunities: usize,
    submitted_bundles: usize,
    total_profit_eth: String,
    success_rate: f64,
    uptime_seconds: u64,
}

async fn get_status(core: SearcherCore) -> Json<StatusResponse> {
    let status = core.get_status().await.unwrap_or_else(|_| {
        crate::core::searcher_core::SearcherStatus {
            is_running: false,
            active_opportunities: 0,
            submitted_bundles: 0,
            performance_metrics: crate::types::PerformanceMetrics {
                transactions_processed: 0,
                opportunities_found: 0,
                bundles_submitted: 0,
                bundles_included: 0,
                total_profit: alloy::primitives::U256::ZERO,
                total_gas_spent: alloy::primitives::U256::ZERO,
                avg_analysis_time: 0.0,
                avg_submission_time: 0.0,
                success_rate: 0.0,
                uptime: 0,
            },
            uptime_seconds: 0,
            micro_arbitrage_status: None,
        }
    });

    let total_profit_eth = ethers::utils::format_ether({
        let mut be = [0u8; 32];
        status
            .performance_metrics
            .total_profit
            .to_be_bytes::<32>()
            .iter()
            .enumerate()
            .for_each(|(i, b)| be[i] = *b);
        ethers::types::U256::from_big_endian(&be)
    });

    Json(StatusResponse {
        is_running: status.is_running,
        active_opportunities: status.active_opportunities,
        submitted_bundles: status.submitted_bundles,
        total_profit_eth,
        success_rate: status.performance_metrics.success_rate,
        uptime_seconds: status.uptime_seconds,
    })
}

#[derive(Serialize)]
struct StrategiesResponse {
    enabled: std::collections::HashMap<crate::types::StrategyType, bool>,
}

async fn get_strategies(core: SearcherCore) -> Json<StrategiesResponse> {
    let map = core
        .strategy_manager
        .get_strategy_enabled_map()
        .await;
    Json(StrategiesResponse { enabled: map })
}

#[derive(serde::Deserialize)]
struct TogglePayload {
    strategy: String,
    enabled: bool,
}

async fn toggle_strategy(core: SearcherCore, Json(payload): Json<TogglePayload>) -> Json<serde_json::Value> {
    use crate::types::StrategyType;
    let ty = match payload.strategy.as_str() {
        "sandwich" => StrategyType::Sandwich,
        "liquidation" => StrategyType::Liquidation,
        "micro" | "micro_arbitrage" => StrategyType::MicroArbitrage,
        "cross" | "cross_chain" => StrategyType::CrossChainArbitrage,
        _ => return Json(serde_json::json!({"ok": false, "error": "unknown strategy"})),
    };

    if let Err(e) = core.set_strategy_enabled(ty, payload.enabled).await {
        return Json(serde_json::json!({"ok": false, "error": e.to_string()}));
    }
    Json(serde_json::json!({"ok": true}))
}

#[derive(Serialize)]
struct StrategyStatsOut {
    transactions_analyzed: u64,
    opportunities_found: u64,
    avg_analysis_time_ms: f64,
}

#[derive(Serialize)]
struct StrategyStatsResp {
    stats: std::collections::HashMap<String, StrategyStatsOut>,
}

async fn get_strategy_stats(core: SearcherCore) -> Json<StrategyStatsResp> {
    let internal = core.get_strategy_stats().await;
    let mut stats: std::collections::HashMap<String, StrategyStatsOut> = std::collections::HashMap::new();
    for (ty, s) in internal {
        stats.insert(
            ty.to_string(),
            StrategyStatsOut {
                transactions_analyzed: s.transactions_analyzed,
                opportunities_found: s.opportunities_found,
                avg_analysis_time_ms: s.avg_analysis_time_ms,
            },
        );
    }
    Json(StrategyStatsResp { stats })
}

#[derive(Serialize)]
struct BundlesResponse {
    stats: BundleStats,
    submitted: Vec<crate::types::Bundle>,
    pending: Vec<crate::types::Bundle>,
}

async fn get_bundles(core: SearcherCore) -> Json<BundlesResponse> {
    let stats = core.get_bundle_stats().await;
    let submitted = core.list_submitted_bundles().await;
    let pending = core.list_pending_bundles().await;
    Json(BundlesResponse { stats, submitted, pending })
}

async fn get_report(core: SearcherCore) -> Json<PerformanceReport> {
    let report = match core.generate_performance_report().await {
        Ok(r) => r,
        Err(_) => crate::core::performance_tracker::PerformanceReport {
            timestamp: 0,
            uptime_seconds: 0,
            summary: crate::core::performance_tracker::PerformanceSummary {
                transactions_processed: 0,
                opportunities_found: 0,
                bundles_submitted: 0,
                bundles_included: 0,
                total_profit_eth: "0".to_string(),
                success_rate: 0.0,
                avg_analysis_time_ms: 0.0,
                avg_submission_time_ms: 0.0,
            },
            detailed_stats: core.get_detailed_stats().await,
            recent_alerts: vec![],
            recommendations: vec![],
        },
    };
    Json(report)
}

async fn get_bundle_by_id(core: SearcherCore, id: String) -> Json<serde_json::Value> {
    if let Some(b) = core.get_bundle_by_id(&id).await {
        return Json(serde_json::json!({"ok": true, "bundle": b}));
    }
    Json(serde_json::json!({"ok": false, "error": "not_found"}))
}

async fn sse_logs(core: SearcherCore) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    use tokio::time::{sleep, Duration};
    let stream = futures_util::stream::unfold((), move |_| {
        let core_clone = core.clone();
        async move {
            sleep(Duration::from_secs(2)).await;
            let alerts = core_clone.get_alerts(true).await;
            let json = serde_json::to_string(&alerts).unwrap_or_else(|_| "[]".to_string());
            let ev = Event::default().event("alerts").data(json);
            Some((Ok(ev), ()))
        }
    });
    Sse::new(stream)
}

#[derive(Serialize)]
struct SettingsResponse {
    strategies: std::collections::HashMap<crate::types::StrategyType, bool>,
    api_port: u16,
    metrics_port: u16,
}

async fn get_settings(config: Arc<crate::config::Config>, core: SearcherCore) -> Json<SettingsResponse> {
    let strategies = core.strategy_manager.get_strategy_enabled_map().await;
    Json(SettingsResponse {
        strategies,
        api_port: config.monitoring.api_port,
        metrics_port: config.monitoring.metrics_port,
    })
}

#[derive(serde::Deserialize)]
struct SettingsActionPayload {
    action: String,
}

async fn post_settings(core: SearcherCore, Json(payload): Json<SettingsActionPayload>) -> Json<serde_json::Value> {
    match payload.action.as_str() {
        "reset_stats" => {
            if let Err(e) = core.reset_stats().await {
                return Json(serde_json::json!({"ok": false, "error": e.to_string()}));
            }
            Json(serde_json::json!({"ok": true}))
        }
        "ack_all_alerts" => {
            let alerts = core.get_alerts(true).await;
            for a in alerts {
                let _ = core.acknowledge_alert(&a.id).await;
            }
            Json(serde_json::json!({"ok": true}))
        }
        _ => Json(serde_json::json!({"ok": false, "error": "unknown action"})),
    }
}
