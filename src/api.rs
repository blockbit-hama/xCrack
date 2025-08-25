use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::{get, post}, Json, Router};
use tower_http::cors::{Any, CorsLayer};
use axum::response::sse::{Sse, Event};
use futures_util::stream::{Stream, StreamExt};
use serde::Serialize;

use crate::config::Config;
use serde_json::json;
use crate::core::SearcherCore;
use crate::core::bundle_manager::BundleStats;
use crate::core::performance_tracker::PerformanceReport;
#[derive(Serialize)]
struct EnvVarStatus { key: String, set: bool }

#[derive(Serialize)]
struct ExternalApiInfo {
    name: String,
    category: String,
    description: String,
    docs: Option<String>,
    env: Vec<EnvVarStatus>,
}

#[derive(Serialize)]
struct SystemInfoResponse {
    api_mode: String,
    network: String,
    rpc_url: String,
    ws_url: Option<String>,
    flashbots_relay_url: String,
    simulation_mode: bool,
    external_apis: Vec<ExternalApiInfo>,
}

async fn get_system(config: Arc<crate::config::Config>, _core: SearcherCore) -> Json<SystemInfoResponse> {
    // Derive API_MODE from env observed at startup context is not tracked; expose guess by simulation flag
    let api_mode = if config.flashbots.simulation_mode { "mock" } else { "real" }.to_string();
    fn env_status(keys: &[&str]) -> Vec<EnvVarStatus> {
        keys.iter().map(|k| EnvVarStatus { key: k.to_string(), set: std::env::var(k).is_ok() }).collect()
    }

    let mut external: Vec<ExternalApiInfo> = Vec::new();
    // Flashbots
    external.push(ExternalApiInfo {
        name: "Flashbots Relay".to_string(),
        category: "MEV bundle submission".to_string(),
        description: "Flashbots 리레이를 통해 프라이빗 번들을 제출합니다. 사용 전략: Sandwich, Liquidation (번들 제출)".to_string(),
        docs: Some("https://docs.flashbots.net".to_string()),
        env: env_status(&["FLASHBOTS_RELAY_URL", "PRIVATE_KEY"]),
    });
    // 0x
    external.push(ExternalApiInfo {
        name: "0x Quotes".to_string(),
        category: "DEX aggregation".to_string(),
        description: "0x Aggregator API로 스왑 경로 견적을 조회합니다. 사용 전략: Sandwich(백업/슬리피지 검증), Liquidation(담보 매각 경로 견적)".to_string(),
        docs: Some("https://docs.0x.org/".to_string()),
        env: env_status(&[]),
    });
    // 1inch
    external.push(ExternalApiInfo {
        name: "1inch Quotes".to_string(),
        category: "DEX aggregation".to_string(),
        description: "1inch API로 최적 스왑 견적을 조회합니다(일부 네트워크는 API 키 필요). 사용 전략: Liquidation(담보 매각), Sandwich(경로 비교)".to_string(),
        docs: Some("https://docs.1inch.io".to_string()),
        env: env_status(&["ONEINCH_API_KEY"]),
    });
    // Oracles
    external.push(ExternalApiInfo {
        name: "Oracles (Chainlink / Uniswap TWAP)".to_string(),
        category: "Price oracles".to_string(),
        description: "체인링크 피드와 Uniswap TWAP을 조합해 가격을 산출합니다. 사용 전략: Liquidation(건전성/청산 트리거), Sandwich(리스크 가드)".to_string(),
        docs: Some("https://docs.chain.link/".to_string()),
        env: env_status(&[]),
    });
    // LiFi
    external.push(ExternalApiInfo {
        name: "LiFi Bridge".to_string(),
        category: "Cross-chain bridging".to_string(),
        description: "Li.Fi로 체인 간 브리지 경로/수수료/유효시간을 조회·실행합니다. 사용 전략: Cross-Chain Arbitrage".to_string(),
        docs: Some("https://docs.li.fi".to_string()),
        env: env_status(&["LIFI_API_KEY"]),
    });

    Json(SystemInfoResponse {
        api_mode,
        network: config.network.name.clone(),
        rpc_url: config.network.rpc_url.clone(),
        ws_url: config.network.ws_url.clone(),
        flashbots_relay_url: config.flashbots.relay_url.clone(),
        simulation_mode: config.flashbots.simulation_mode,
        external_apis: external,
    })
}

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
        let core_system = self.core.clone();

        let config_for_settings = Arc::clone(&self.config);
        let core_for_settings_get = self.core.clone();
        let core_for_settings_post = self.core.clone();
        let config_for_system = Arc::clone(&self.config);

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
            .route("/api/system", get(move || get_system(Arc::clone(&config_for_system), core_system.clone())))
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

#[derive(Serialize)]
struct StrategyParamsResponse {
    sandwich: crate::config::SandwichConfig,
    liquidation: crate::config::LiquidationConfig,
    micro_arbitrage: crate::config::MicroArbitrageConfig,
}

async fn get_strategy_params(config: Arc<crate::config::Config>) -> Json<StrategyParamsResponse> {
    Json(StrategyParamsResponse {
        sandwich: config.strategies.sandwich.clone(),
        liquidation: config.strategies.liquidation.clone(),
        micro_arbitrage: config.strategies.micro_arbitrage.clone(),
    })
}

#[derive(serde::Deserialize)]
struct StrategyParamsUpdatePayload {
    strategy: String,
    updates: serde_json::Value,
    #[serde(default)]
    config_path: Option<String>,
}

async fn post_strategy_params(config: Arc<crate::config::Config>, Json(payload): Json<StrategyParamsUpdatePayload>) -> Json<serde_json::Value> {
    // Clone Config to modify and save; runtime config remains unchanged until restart
    let mut updated = (*config).clone();

    // Merge helper
    fn merge_into<T: serde::de::DeserializeOwned + serde::Serialize + Clone>(original: &T, updates: &serde_json::Value) -> Result<T, String> {
        let mut val = serde_json::to_value(original).map_err(|e| e.to_string())?;
        merge_json(&mut val, updates);
        serde_json::from_value(val).map_err(|e| e.to_string())
    }

    // Shallow JSON merge (recursive for objects)
    fn merge_json(base: &mut serde_json::Value, updates: &serde_json::Value) {
        use serde_json::Value;
        match (base, updates) {
            (Value::Object(base_map), Value::Object(update_map)) => {
                for (k, v) in update_map.iter() {
                    match base_map.get_mut(k) {
                        Some(b) => merge_json(b, v),
                        None => { base_map.insert(k.clone(), v.clone()); },
                    }
                }
            }
            (base_slot, new_val) => { *base_slot = new_val.clone(); }
        }
    }

    let result = match payload.strategy.as_str() {
        "sandwich" => {
            match merge_into(&updated.strategies.sandwich, &payload.updates) {
                Ok(new_section) => { updated.strategies.sandwich = new_section; Ok(()) }
                Err(e) => Err(e)
            }
        }
        , "liquidation" => {
            match merge_into(&updated.strategies.liquidation, &payload.updates) {
                Ok(new_section) => { updated.strategies.liquidation = new_section; Ok(()) }
                Err(e) => Err(e)
            }
        }
        , "micro" | "micro_arbitrage" => {
            match merge_into(&updated.strategies.micro_arbitrage, &payload.updates) {
                Ok(new_section) => { updated.strategies.micro_arbitrage = new_section; Ok(()) }
                Err(e) => Err(e)
            }
        }
        _ => Err("unknown strategy".to_string()),
    };

    if let Err(err) = result { return Json(json!({"ok": false, "error": err})); }

    // Attempt to save to file
    let path = payload.config_path
        .or_else(|| std::env::var("XCRACK_CONFIG_PATH").ok())
        .unwrap_or_else(|| "config/default.toml".to_string());

    match updated.save(&path).await {
        Ok(_) => Json(json!({"ok": true, "saved": true, "path": path, "restart_required": true})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}
