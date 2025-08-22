use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::{get, post}, Json, Router};
use serde::Serialize;

use crate::config::Config;
use crate::core::SearcherCore;

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

        let app = Router::new()
            .route("/api/health", get(|| async { Json(serde_json::json!({"ok": true})) }))
            .route("/api/status", get(move || get_status(core_status.clone())))
            .route("/api/strategies", get(move || get_strategies(core_strategies.clone())))
            .route("/api/strategies/toggle", post(move |payload| toggle_strategy(core_toggle.clone(), payload)));

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
