use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::{get, post}, Json, Router};
use tower_http::cors::{Any, CorsLayer};
use axum::response::sse::{Sse, Event};
use futures_util::stream::Stream;
use serde::Serialize;
use serde_json::{json, Value};
use tracing::info;

use crate::config::Config;
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
        description: "0x Aggregator API로 최적 스왑 경로를 조회합니다. 주 사용: CrossChain Arbitrage(크로스체인 차익거래 메인), 보조: Liquidation(담보 매각 경로 견적)".to_string(),
        docs: Some("https://docs.0x.org/".to_string()),
        env: env_status(&[]),
    });
    // 1inch
    external.push(ExternalApiInfo {
        name: "1inch Quotes".to_string(),
        category: "DEX aggregation".to_string(),
        description: "1inch API로 최적 스왑 견적을 조회합니다. 주 사용: Liquidation(담보 매각 경로), Micro Arbitrage(DEX 경로 비교), 보조: Sandwich(백업 견적)".to_string(),
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
    // Flashloan Providers
    external.push(ExternalApiInfo {
        name: "Aave V3 Pool".to_string(),
        category: "Flashloan provider".to_string(),
        description: "Aave V3 프로토콜에서 플래시론을 실행합니다. 주 사용: 모든 전략의 flashloan 지원 (설정에서 use_flashloan=true 시)".to_string(),
        docs: Some("https://docs.aave.com/developers/guides/flash-loans".to_string()),
        env: env_status(&["FLASHLOAN_RECEIVER"]),
    });
    external.push(ExternalApiInfo {
        name: "Balancer Vault".to_string(),
        category: "Flashloan provider".to_string(),
        description: "Balancer V2 Vault에서 무료 플래시론을 실행합니다. 백업 플래시론 제공자로 사용됩니다.".to_string(),
        docs: Some("https://docs.balancer.fi/reference/contracts/flash-loans.html".to_string()),
        env: env_status(&[]),
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
        let config_for_params_get = Arc::clone(&self.config);
        let config_for_params_post = Arc::clone(&self.config);

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Clone cores for new endpoints
        let core_mempool = self.core.clone();
        let core_performance = self.core.clone();
        let core_alerts = self.core.clone();
        let core_micro = self.core.clone();
        let core_onchain = self.core.clone();
        let core_network = self.core.clone();
        let core_risk = self.core.clone();
        let core_flashloan = self.core.clone();
        let core_cross = self.core.clone();
        let core_liquidation_dashboard = self.core.clone();
        let core_liquidation_opportunities = self.core.clone();
        let core_liquidation_config = self.core.clone();
        let core_protocol_status = self.core.clone();
        let core_sandwich_dashboard = self.core.clone();
        let core_sandwich_opportunities = self.core.clone();
        let core_sandwich_config = self.core.clone();
        let core_sandwich_start = self.core.clone();
        let core_sandwich_stop = self.core.clone();
        let core_sandwich_status = self.core.clone();
        let core_micro_arbitrage_dashboard = self.core.clone();
        let core_micro_arbitrage_opportunities = self.core.clone();
        let core_micro_arbitrage_config = self.core.clone();
        let core_micro_arbitrage_start = self.core.clone();
        let core_micro_arbitrage_stop = self.core.clone();
        let core_micro_arbitrage_status = self.core.clone();
        let core_complex_arbitrage_dashboard = self.core.clone();
        let core_complex_arbitrage_opportunities = self.core.clone();
        let core_complex_arbitrage_config = self.core.clone();
        let core_complex_arbitrage_start = self.core.clone();
        let core_complex_arbitrage_stop = self.core.clone();
        let core_complex_arbitrage_status = self.core.clone();
        let config_for_liquidation = Arc::clone(&self.config);
        let config_for_sandwich = Arc::clone(&self.config);
        let config_for_micro_arbitrage = Arc::clone(&self.config);
        let config_for_complex_arbitrage = Arc::clone(&self.config);

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
            .route("/api/strategies/params", get(move || get_strategy_params(Arc::clone(&config_for_params_get))))
            .route("/api/strategies/params", post(move |payload| post_strategy_params(Arc::clone(&config_for_params_post), payload)))
            .route("/api/system", get(move || get_system(Arc::clone(&config_for_system), core_system.clone())))
            // New endpoints for UI expansion
            .route("/api/mempool/status", get(move || get_mempool_status(core_mempool.clone())))
            .route("/api/performance/dashboard", get(move || get_performance_dashboard(core_performance.clone())))
            .route("/api/alerts", get(move || get_alerts_list(core_alerts.clone())))
            .route("/api/strategies/micro/dashboard", get(move || get_micro_dashboard(core_micro.clone())))
            .route("/api/onchain/dashboard", get(move || get_onchain_dashboard(core_onchain.clone())))
            .route("/api/network/health", get(move || get_network_health(core_network.clone())))
            .route("/api/risk/dashboard", get(move || get_risk_dashboard(core_risk.clone())))
            .route("/api/flashloan/dashboard", get(move || get_flashloan_dashboard(core_flashloan.clone())))
            .route("/api/strategies/cross/dashboard", get(move || get_cross_dashboard(core_cross.clone())))
            // Liquidation endpoints
            .route("/api/liquidation/dashboard", get(move || get_liquidation_dashboard(core_liquidation_dashboard.clone())))
            .route("/api/liquidation/opportunities", get(move || get_liquidation_opportunities(core_liquidation_opportunities.clone())))
            .route("/api/liquidation/config", get(move || get_liquidation_config(Arc::clone(&config_for_liquidation))))
            .route("/api/liquidation/config", post(move |payload| post_liquidation_config(core_liquidation_config.clone(), payload)))
            .route("/api/sandwich/dashboard", get(move || get_sandwich_dashboard(core_sandwich_dashboard.clone())))
            .route("/api/sandwich/opportunities", get(move || get_sandwich_opportunities(core_sandwich_opportunities.clone())))
            .route("/api/sandwich/config", get(move || get_sandwich_config(Arc::clone(&config_for_sandwich))))
            .route("/api/sandwich/config", post(move |payload| post_sandwich_config(core_sandwich_config.clone(), payload)))
            .route("/api/sandwich/start", post(move || start_sandwich_strategy(core_sandwich_start.clone())))
            .route("/api/sandwich/stop", post(move || stop_sandwich_strategy(core_sandwich_stop.clone())))
            .route("/api/sandwich/status", get(move || get_sandwich_status(core_sandwich_status.clone())))
            .route("/api/cex-dex-arbitrage/dashboard", get(move || get_cex_dex_arbitrage_dashboard(core_micro_arbitrage_dashboard.clone())))
            .route("/api/cex-dex-arbitrage/opportunities", get(move || get_cex_dex_arbitrage_opportunities(core_micro_arbitrage_opportunities.clone())))
            .route("/api/cex-dex-arbitrage/config", get(move || get_cex_dex_arbitrage_config(Arc::clone(&config_for_micro_arbitrage))))
            .route("/api/cex-dex-arbitrage/config", post(move |payload| post_cex_dex_arbitrage_config(core_micro_arbitrage_config.clone(), payload)))
            .route("/api/cex-dex-arbitrage/start", post(move || start_cex_dex_arbitrage_strategy(core_micro_arbitrage_start.clone())))
            .route("/api/cex-dex-arbitrage/stop", post(move || stop_cex_dex_arbitrage_strategy(core_micro_arbitrage_stop.clone())))
            .route("/api/cex-dex-arbitrage/status", get(move || get_cex_dex_arbitrage_status(core_micro_arbitrage_status.clone())))
            .route("/api/complex-arbitrage/dashboard", get(move || get_complex_arbitrage_dashboard(core_complex_arbitrage_dashboard.clone())))
            .route("/api/complex-arbitrage/opportunities", get(move || get_complex_arbitrage_opportunities(core_complex_arbitrage_opportunities.clone())))
            .route("/api/complex-arbitrage/config", get(move || get_complex_arbitrage_config(Arc::clone(&config_for_complex_arbitrage))))
            .route("/api/complex-arbitrage/config", post(move |payload| post_complex_arbitrage_config(core_complex_arbitrage_config.clone(), payload)))
            .route("/api/complex-arbitrage/start", post(move || start_complex_arbitrage_strategy(core_complex_arbitrage_start.clone())))
            .route("/api/complex-arbitrage/stop", post(move || stop_complex_arbitrage_strategy(core_complex_arbitrage_stop.clone())))
            .route("/api/complex-arbitrage/status", get(move || get_complex_arbitrage_status(core_complex_arbitrage_status.clone())))
            .route("/api/protocols/status", get(move || get_protocol_status(core_protocol_status.clone())))
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
                total_profit: U256::zero(),
                total_gas_spent: U256::zero(),
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
    submitted: Vec<crate::mev::bundle::Bundle>,
    pending: Vec<crate::mev::bundle::Bundle>,
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
        "liquidation" => {
            match merge_into(&updated.strategies.liquidation, &payload.updates) {
                Ok(new_section) => { updated.strategies.liquidation = new_section; Ok(()) }
                Err(e) => Err(e)
            }
        }
        "micro" | "micro_arbitrage" => {
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

// New API endpoint implementations

async fn get_mempool_status(_core: SearcherCore) -> Json<serde_json::Value> {
    Json(json!({
        "is_monitoring": true,
        "connected": true,
        "last_block": 19234567,
        "stats": {
            "pending_transactions": 15432,
            "transactions_per_second": 12.5,
            "avg_gas_price": "25.3",
            "high_gas_price": "89.2",
            "low_gas_price": "18.1",
            "mempool_size_mb": 156.7
        },
        "recent_transactions": [
            {
                "hash": "0x1234...abcd",
                "from": "0x742d...1234",
                "to": "0x8765...5678", 
                "value": "1.5",
                "gas_price": "32.1",
                "timestamp": 1703123456,
                "type": "DEX Swap",
                "detected_mev": true,
                "frontrun_opportunity": true
            },
            {
                "hash": "0x5678...efgh",
                "from": "0x9abc...def0",
                "to": "0x1234...5678",
                "value": "0.8",
                "gas_price": "28.5",
                "timestamp": 1703123455,
                "type": "Transfer",
                "detected_mev": false,
                "frontrun_opportunity": false
            }
        ],
        "dex_metrics": {
            "uniswap_v3_volume": "2.5M",
            "uniswap_v2_volume": "1.2M",
            "sushiswap_volume": "450K",
            "pancakeswap_volume": "780K"
        },
        "mev_metrics": {
            "sandwich_opportunities": 23,
            "arbitrage_opportunities": 12,
            "liquidation_opportunities": 5,
            "total_extractable_value": "15.6"
        }
    }))
}

async fn get_performance_dashboard(core: SearcherCore) -> Json<serde_json::Value> {
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
                total_profit: U256::zero(),
                total_gas_spent: U256::zero(),
                avg_analysis_time: 0.0,
                avg_submission_time: 0.0,
                success_rate: 0.0,
                uptime: 0,
            },
            uptime_seconds: 0,
            micro_arbitrage_status: None,
        }
    });
    
    Json(json!({
        "profitability": {
            "total_profit_eth": "12.456",
            "total_profit_usd": "24912.34",
            "profit_24h": "2.134",
            "profit_7d": "15.678",
            "roi_percentage": "156.7"
        },
        "strategy_performance": {
            "sandwich": {
                "profit": "8.234",
                "success_rate": 0.87,
                "avg_profit_per_tx": "0.012",
                "total_transactions": 687
            },
            "liquidation": {
                "profit": "3.145",
                "success_rate": 0.94,
                "avg_profit_per_tx": "0.089",
                "total_transactions": 35
            },
            "micro_arbitrage": {
                "profit": "1.077",
                "success_rate": 0.76,
                "avg_profit_per_tx": "0.004",
                "total_transactions": 269
            }
        },
        "gas_analytics": {
            "total_gas_spent": "4.567",
            "avg_gas_price": "28.5",
            "gas_efficiency_score": 0.82,
            "gas_saved_via_optimization": "0.234"
        },
        "system_health": {
            "uptime_hours": status.uptime_seconds / 3600,
            "memory_usage_mb": 128.5,
            "cpu_usage_percentage": 15.2,
            "active_connections": 12
        },
        "recent_performance": [
            {"timestamp": 1703123456, "profit": "0.045", "strategy": "sandwich"},
            {"timestamp": 1703123400, "profit": "0.123", "strategy": "liquidation"},
            {"timestamp": 1703123350, "profit": "0.008", "strategy": "micro_arbitrage"}
        ]
    }))
}

async fn get_alerts_list(core: SearcherCore) -> Json<serde_json::Value> {
    let _alerts = core.get_alerts(false).await;
    
    Json(json!({
        "alerts": [
            {
                "id": "alert_001",
                "severity": "warning",
                "title": "High Gas Price",
                "message": "Gas price above threshold: 45 gwei",
                "timestamp": 1703123456,
                "acknowledged": false,
                "category": "performance"
            },
            {
                "id": "alert_002", 
                "severity": "info",
                "title": "Strategy Performance",
                "message": "Sandwich strategy success rate: 87%",
                "timestamp": 1703123400,
                "acknowledged": true,
                "category": "strategy"
            }
        ],
        "summary": {
            "total": 15,
            "critical": 0,
            "warning": 3,
            "info": 12,
            "unacknowledged": 3
        },
        "recent_alerts": [
            {
                "id": "alert_001",
                "severity": "warning", 
                "title": "High Gas Price",
                "message": "Gas price above threshold: 45 gwei",
                "timestamp": 1703123456,
                "acknowledged": false
            },
            {
                "id": "alert_002",
                "severity": "info",
                "title": "New Opportunity",
                "message": "Liquidation opportunity detected: 0.45 ETH",
                "timestamp": 1703123350,
                "acknowledged": false
            }
        ]
    }))
}

async fn get_micro_dashboard(_core: SearcherCore) -> Json<serde_json::Value> {
    Json(json!({
        "exchange_connections": {
            "binance": {"connected": true, "latency_ms": 45, "last_update": 1703123456},
            "coinbase": {"connected": true, "latency_ms": 52, "last_update": 1703123455},
            "kraken": {"connected": false, "latency_ms": 0, "last_update": 1703120000},
            "okx": {"connected": true, "latency_ms": 38, "last_update": 1703123457}
        },
        "opportunities": {
            "active": 3,
            "total_today": 47,
            "success_rate": 0.76,
            "avg_profit_per_opportunity": "0.0045"
        },
        "current_opportunities": [
            {
                "pair": "ETH/USDC",
                "buy_exchange": "binance",
                "sell_exchange": "coinbase",
                "profit_estimate": "0.012",
                "confidence": 0.89,
                "expiry": 1703123500
            },
            {
                "pair": "BTC/USDT", 
                "buy_exchange": "okx",
                "sell_exchange": "binance",
                "profit_estimate": "0.008",
                "confidence": 0.92,
                "expiry": 1703123480
            }
        ],
        "recent_trades": [
            {
                "timestamp": 1703123400,
                "pair": "WETH/USDC",
                "profit": "0.0067",
                "buy_exchange": "coinbase",
                "sell_exchange": "binance",
                "status": "completed"
            }
        ],
        "risk_analysis": {
            "exposure_limit_used": 0.34,
            "max_position_size": "1000.0",
            "current_exposure": "340.0",
            "risk_score": 0.25
        }
    }))
}

async fn get_cross_dashboard(core: SearcherCore) -> Json<serde_json::Value> {
    // Try to read metrics via typed handle; fallback to mock if unavailable
    let metrics = if let Some(strat) = core.strategy_manager.get_sandwich_strategy() {
        let m = strat.get_performance_metrics();
        serde_json::json!({
            "total_opportunities": m.total_opportunities_found,
            "trades_executed": m.total_trades_executed,
            "success_rate": m.success_rate,
            "total_profit": m.total_profit,
            "avg_execution_time": m.avg_execution_time,
            "failed_trades": m.failed_trades,
        })
    } else {
        serde_json::json!({
            "total_opportunities": 0,
            "trades_executed": 0,
            "success_rate": 0.0,
            "total_profit": 0.0,
            "avg_execution_time": 0.0,
            "failed_trades": 0,
        })
    };

    Json(json!({
        "summary": metrics,
        "recent_routes": [
            {"protocol": "lifi", "from": "polygon", "to": "ethereum", "avg_time": 320, "success_rate": 0.92},
            {"protocol": "stargate", "from": "bsc", "to": "arbitrum", "avg_time": 410, "success_rate": 0.88}
        ]
    }))
}

async fn get_onchain_dashboard(_core: SearcherCore) -> Json<serde_json::Value> {
    Json(json!({
        "block_info": {
            "latest_block": 19234567,
            "block_time": 12.1,
            "gas_limit": "30000000",
            "gas_used": "29456789",
            "base_fee": "25.3"
        },
        "mev_metrics": {
            "total_mev_extracted_24h": "145.6",
            "sandwich_volume": "89.2",
            "arbitrage_volume": "34.5", 
            "liquidation_volume": "21.9"
        },
        "liquidity_analysis": {
            "uniswap_v3_tvl": "2.1B",
            "uniswap_v2_tvl": "890M",
            "sushiswap_tvl": "234M",
            "total_dex_tvl": "3.2B"
        },
        "trending_tokens": [
            {"symbol": "WETH", "volume_24h": "234M", "price_change": "+2.3%"},
            {"symbol": "USDC", "volume_24h": "567M", "price_change": "+0.1%"},
            {"symbol": "WBTC", "volume_24h": "89M", "price_change": "+1.8%"}
        ],
        "dex_activity": [
            {"dex": "Uniswap V3", "volume": "1.2B", "transactions": 45678, "unique_traders": 12345},
            {"dex": "Uniswap V2", "volume": "456M", "transactions": 23456, "unique_traders": 8901},
            {"dex": "SushiSwap", "volume": "123M", "transactions": 12345, "unique_traders": 4567}
        ]
    }))
}

async fn get_network_health(_core: SearcherCore) -> Json<serde_json::Value> {
    Json(json!({
        "node_status": {
            "ethereum": {"connected": true, "sync_status": "synced", "peer_count": 45, "latest_block": 19234567},
            "polygon": {"connected": true, "sync_status": "synced", "peer_count": 32, "latest_block": 52345678},
            "bsc": {"connected": false, "sync_status": "disconnected", "peer_count": 0, "latest_block": 0}
        },
        "flashbots_relay": {
            "status": "connected",
            "response_time_ms": 89,
            "success_rate": 0.94,
            "bundles_submitted_24h": 156,
            "bundles_included_24h": 147
        },
        "system_resources": {
            "cpu_usage_percentage": 15.2,
            "memory_usage_percentage": 68.4,
            "disk_usage_percentage": 45.7,
            "network_io": {"in": "12.5 MB/s", "out": "8.9 MB/s"}
        },
        "api_endpoints": {
            "rpc_ethereum": {"status": "healthy", "response_time": 234},
            "ws_ethereum": {"status": "healthy", "response_time": 89},
            "0x_api": {"status": "healthy", "response_time": 156},
            "1inch_api": {"status": "degraded", "response_time": 890}
        },
        "alerts": [
            {"type": "warning", "message": "1inch API response time above threshold", "timestamp": 1703123400}
        ]
    }))
}

async fn get_risk_dashboard(_core: SearcherCore) -> Json<serde_json::Value> {
    Json(json!({
        "portfolio_risk": {
            "total_value_at_risk": "12.45",
            "max_drawdown": "3.21",
            "sharpe_ratio": 2.34,
            "volatility": 0.156,
            "beta": 1.23
        },
        "position_monitoring": {
            "total_positions": 8,
            "largest_position": "2.34 ETH",
            "longest_held": "4.5 hours",
            "position_concentration": 0.23
        },
        "risk_limits": {
            "max_position_size": {"limit": "5.0 ETH", "current": "2.34 ETH", "utilization": 0.47},
            "daily_loss_limit": {"limit": "1.0 ETH", "current": "0.12 ETH", "utilization": 0.12},
            "gas_budget": {"limit": "0.5 ETH", "current": "0.23 ETH", "utilization": 0.46}
        },
        "stress_testing": {
            "scenario_1": {"name": "Market Crash -20%", "estimated_loss": "2.1 ETH", "probability": 0.05},
            "scenario_2": {"name": "Gas Price Spike 10x", "estimated_loss": "0.8 ETH", "probability": 0.15},
            "scenario_3": {"name": "DEX Liquidity Drain", "estimated_loss": "1.5 ETH", "probability": 0.08}
        },
        "emergency_controls": {
            "emergency_stop": {"enabled": false, "last_triggered": null},
            "position_limits": {"enabled": true, "triggered_today": 2},
            "gas_limits": {"enabled": true, "triggered_today": 0}
        },
        "recent_risk_events": [
            {
                "timestamp": 1703123400,
                "type": "position_limit_hit",
                "description": "Max position size reached for WETH",
                "action_taken": "Position rejected"
            }
        ]
    }))
}

async fn get_flashloan_dashboard(_core: SearcherCore) -> Json<serde_json::Value> {
    Json(json!({
        "flashloan_providers": {
            "aave_v3": {
                "available": true,
                "max_amount": "100000.0 USDC",
                "fee_rate": "0.05%",
                "gas_cost": "~150k",
                "last_update": 1703123456
            },
            "aave_v2": {
                "available": true,
                "max_amount": "50000.0 USDC",
                "fee_rate": "0.09%", 
                "gas_cost": "~120k",
                "last_update": 1703123450
            },
            "balancer": {
                "available": true,
                "max_amount": "200000.0 USDC",
                "fee_rate": "0.00%",
                "gas_cost": "~180k",
                "last_update": 1703123445
            },
            "uniswap_v3": {
                "available": false,
                "max_amount": "0.0 USDC",
                "fee_rate": "0.30%",
                "gas_cost": "~200k",
                "last_update": 1703120000
            }
        },
        "recent_flashloans": [
            {
                "tx_hash": "0xabcd1234...",
                "timestamp": 1703123400,
                "provider": "aave_v3",
                "token": "USDC",
                "amount": "10000.0",
                "fee_paid": "5.0",
                "strategy": "liquidation",
                "profit": "45.67",
                "gas_used": "145623",
                "status": "success"
            },
            {
                "tx_hash": "0xef567890...",
                "timestamp": 1703123350,
                "provider": "balancer",
                "token": "WETH", 
                "amount": "50.0",
                "fee_paid": "0.0",
                "strategy": "sandwich",
                "profit": "12.34",
                "gas_used": "178934",
                "status": "success"
            },
            {
                "tx_hash": "0x12345678...",
                "timestamp": 1703123300,
                "provider": "aave_v2",
                "token": "USDC",
                "amount": "25000.0",
                "fee_paid": "22.5",
                "strategy": "arbitrage",
                "profit": "-5.23",
                "gas_used": "156789",
                "status": "failed"
            }
        ],
        "performance_metrics": {
            "total_flashloans": 147,
            "total_volume": "2450000.0 USD",
            "total_fees_paid": "1234.56 USD",
            "total_profit": "12345.67 USD",
            "success_rate": 0.89,
            "avg_profit_per_loan": "84.02 USD",
            "most_used_provider": "aave_v3"
        },
        "flashloan_contracts": {
            "aave_v3_pool": {
                "address": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
                "name": "Aave V3 Pool",
                "verified": true,
                "proxy": true,
                "implementation": "0x34339f94350EC5274ea44d0C37DAe9e968c44081"
            },
            "balancer_vault": {
                "address": "0xBA12222222228d8Ba445958a75a0704d566BF2C8",
                "name": "Balancer V2 Vault",
                "verified": true,
                "proxy": false,
                "implementation": null
            },
            "our_flashloan_contract": {
                "address": "0x1234567890123456789012345678901234567890",
                "name": "xCrack Flashloan Executor",
                "verified": true,
                "proxy": true,
                "implementation": "0x9876543210987654321098765432109876543210"
            }
        },
        "smart_contracts": {
            "liquidation_strategy": {
                "solidity_version": "0.8.19",
                "source_code": "pragma solidity ^0.8.19;\\n\\nimport \\\"@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol\\\";\\nimport \\\"@aave/core-v3/contracts/interfaces/IPool.sol\\\";\\nimport \\\"@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol\\\";\\nimport \\\"@openzeppelin/contracts/token/ERC20/IERC20.sol\\\";\\n\\n/**\\n * @title xCrack Liquidation Strategy\\n * @dev Flashloan-based liquidation bot for Aave/Compound protocols\\n */\\ncontract LiquidationStrategy is FlashLoanSimpleReceiverBase {\\n    address private owner;\\n    \\n    struct LiquidationParams {\\n        address protocol;\\n        address user;\\n        address collateralAsset;\\n        address debtAsset;\\n        uint256 debtToCover;\\n        address dexRouter;\\n        bytes swapCalldata;\\n    }\\n    \\n    modifier onlyOwner() {\\n        require(msg.sender == owner, \\\"Not authorized\\\");\\n        _;\\n    }\\n    \\n    constructor(IPoolAddressesProvider provider) FlashLoanSimpleReceiverBase(provider) {\\n        owner = msg.sender;\\n    }\\n    \\n    function executeLiquidation(\\n        address asset,\\n        uint256 amount,\\n        LiquidationParams calldata params\\n    ) external onlyOwner {\\n        bytes memory data = abi.encode(params);\\n        POOL.flashLoanSimple(address(this), asset, amount, data, 0);\\n    }\\n    \\n    function executeOperation(\\n        address asset,\\n        uint256 amount,\\n        uint256 premium,\\n        address initiator,\\n        bytes calldata params\\n    ) external override returns (bool) {\\n        require(msg.sender == address(POOL), \\\"Invalid caller\\\");\\n        \\n        LiquidationParams memory liquidationParams = abi.decode(params, (LiquidationParams));\\n        \\n        // 1. Liquidate the user position\\n        _liquidatePosition(liquidationParams, asset, amount);\\n        \\n        // 2. Swap collateral for debt asset to repay flashloan\\n        _swapCollateralForDebt(liquidationParams);\\n        \\n        // 3. Repay flashloan\\n        uint256 amountOwed = amount + premium;\\n        IERC20(asset).approve(address(POOL), amountOwed);\\n        \\n        return true;\\n    }\\n    \\n    function _liquidatePosition(LiquidationParams memory params, address asset, uint256 amount) private {\\n        // Protocol-specific liquidation logic\\n        // For Aave: call liquidationCall\\n        // For Compound: call liquidateBorrow\\n    }\\n    \\n    function _swapCollateralForDebt(LiquidationParams memory params) private {\\n        // Use DEX router to swap collateral to debt token\\n        // Ensure we get enough to repay flashloan + profit\\n    }\\n}"
            },
            "sandwich_strategy": {
                "solidity_version": "0.8.19",
                "source_code": "pragma solidity ^0.8.19;\\n\\nimport \\\"@openzeppelin/contracts/token/ERC20/IERC20.sol\\\";\\n\\n/**\\n * @title xCrack Sandwich Strategy (policy: no flashloan)\\n * @dev Executes frontrun/backrun via direct public transactions; flashloans are not used.\\n */\\ncontract SandwichStrategyDocStub {\\n    address private owner;\\n    \\n    struct SandwichParams {\\n        address dexRouter;\\n        bytes frontRunCalldata;\\n        bytes backRunCalldata;\\n    }\\n    \\n    modifier onlyOwner() {\\n        require(msg.sender == owner, \\\"Not authorized\\\");\\n        _;\\n    }\\n    \\n    constructor() {\\n        owner = msg.sender;\\n    }\\n    \\n    function executeSandwich(SandwichParams calldata params) external onlyOwner {\\n        // 1) Call router with frontRunCalldata\\n        // 2) Submit victim tx separately (mempool coordination)\\n        // 3) Call router with backRunCalldata\\n        // Note: No flashloan involved.\\n    }\\n}"
            },
            "arbitrage_strategy": {
                "solidity_version": "0.8.19",
                "source_code": "pragma solidity ^0.8.19;\\n\\nimport \\\"@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol\\\";\\nimport \\\"@aave/core-v3/contracts/interfaces/IPool.sol\\\";\\nimport \\\"@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol\\\";\\nimport \\\"@openzeppelin/contracts/token/ERC20/IERC20.sol\\\";\\n\\n/**\\n * @title xCrack Arbitrage Strategy\\n * @dev Cross-DEX arbitrage using flashloans\\n */\\ncontract ArbitrageStrategy is FlashLoanSimpleReceiverBase {\\n    address private owner;\\n    \\n    struct ArbitrageParams {\\n        address tokenA;\\n        address tokenB;\\n        address dexA;\\n        address dexB;\\n        uint256 amountIn;\\n        uint256 expectedProfitMin;\\n        bytes swapCallDataA;\\n        bytes swapCallDataB;\\n    }\\n    \\n    modifier onlyOwner() {\\n        require(msg.sender == owner, \\\"Not authorized\\\");\\n        _;\\n    }\\n    \\n    constructor(IPoolAddressesProvider provider) FlashLoanSimpleReceiverBase(provider) {\\n        owner = msg.sender;\\n    }\\n    \\n    function executeArbitrage(\\n        address asset,\\n        uint256 amount,\\n        ArbitrageParams calldata params\\n    ) external onlyOwner {\\n        bytes memory data = abi.encode(params);\\n        POOL.flashLoanSimple(address(this), asset, amount, data, 0);\\n    }\\n    \\n    function executeOperation(\\n        address asset,\\n        uint256 amount,\\n        uint256 premium,\\n        address initiator,\\n        bytes calldata params\\n    ) external override returns (bool) {\\n        require(msg.sender == address(POOL), \\\"Invalid caller\\\");\\n        \\n        ArbitrageParams memory arbParams = abi.decode(params, (ArbitrageParams));\\n        \\n        // 1. Buy token on DEX A (lower price)\\n        uint256 tokensBought = _buyOnDexA(arbParams, amount);\\n        \\n        // 2. Sell token on DEX B (higher price)\\n        uint256 tokensReceived = _sellOnDexB(arbParams, tokensBought);\\n        \\n        // 3. Check profitability\\n        require(tokensReceived > amount + premium + arbParams.expectedProfitMin, \\\"Insufficient profit\\\");\\n        \\n        // 4. Repay flashloan\\n        uint256 amountOwed = amount + premium;\\n        IERC20(asset).approve(address(POOL), amountOwed);\\n        \\n        return true;\\n    }\\n    \\n    function _buyOnDexA(ArbitrageParams memory params, uint256 amount) private returns (uint256) {\\n        // Execute buy order on DEX A\\n        // Return amount of tokens received\\n    }\\n    \\n    function _sellOnDexB(ArbitrageParams memory params, uint256 tokenAmount) private returns (uint256) {\\n        // Execute sell order on DEX B\\n        // Return amount of base tokens received\\n    }\\n    \\n    function calculateProfitability(ArbitrageParams calldata params) external view returns (uint256 expectedProfit) {\\n        // Calculate expected profit from arbitrage opportunity\\n        // Consider gas costs, slippage, and flashloan fees\\n    }\\n}"
            },
        },
        "gas_analytics": {
            "avg_gas_per_flashloan": "165000",
            "most_expensive_flashloan": "245000",
            "cheapest_flashloan": "98000",
            "gas_optimization_savings": "23.5%"
        }
    }))
}

// Liquidation Dashboard Endpoint
async fn get_liquidation_dashboard(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/liquidation/dashboard");

    // TODO: Integrate with IntegratedLiquidationManager when available in SearcherCore
    // For now, return mock data
    Json(json!({
        "total_liquidations": 147,
        "total_profit": "12345.67",
        "active_positions": 23,
        "success_rate": 0.89,
        "pending_executions": 5,
        "performance_metrics": {
            "avg_execution_time_ms": 245.5,
            "uptime_seconds": 86400,
            "execution_success_rate": 0.89
        }
    }))
}

// Liquidation Opportunities Endpoint
async fn get_liquidation_opportunities(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/liquidation/opportunities");

    // TODO: Integrate with IntegratedLiquidationManager when available in SearcherCore
    // For now, return mock data
    Json(json!({
        "opportunities": [
            {
                "id": "liq-001",
                "protocol": "aave_v3",
                "position": "0x1234...5678",
                "collateral": "50.5 ETH",
                "debt": "45000.0 USDC",
                "liquidation_threshold": 0.825,
                "health_factor": 1.05,
                "estimated_profit": "$450.50",
                "execution_cost": "$125.00",
                "timestamp": 1703123456
            },
            {
                "id": "liq-002",
                "protocol": "compound_v3",
                "position": "0xabcd...ef01",
                "collateral": "120.0 ETH",
                "debt": "95000.0 USDC",
                "liquidation_threshold": 0.80,
                "health_factor": 1.12,
                "estimated_profit": "$890.25",
                "execution_cost": "$210.00",
                "timestamp": 1703123450
            }
        ],
        "total": 2
    }))
}

// Protocol Status Endpoint
async fn get_protocol_status(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/protocols/status");

    // TODO: Integrate with IntegratedLiquidationManager when available in SearcherCore
    // For now, return mock data
    Json(json!([
        {
            "protocol": "Aave v3",
            "status": "active",
            "users_monitored": 1234,
            "total_tvl": "$2,450,000.00",
            "liquidatable_positions": 23,
            "last_update": chrono::Utc::now().timestamp()
        },
        {
            "protocol": "Compound v3",
            "status": "active",
            "users_monitored": 567,
            "total_tvl": "$1,200,000.00",
            "liquidatable_positions": 12,
            "last_update": chrono::Utc::now().timestamp()
        },
        {
            "protocol": "MakerDAO",
            "status": "active",
            "users_monitored": 890,
            "total_tvl": "$3,100,000.00",
            "liquidatable_positions": 8,
            "last_update": chrono::Utc::now().timestamp()
        }
    ]))
}

// Get Liquidation Configuration
async fn get_liquidation_config(config: Arc<Config>) -> Json<Value> {
    info!("GET /api/liquidation/config");

    Json(json!({
        "network": {
            "chain_id": config.network.chain_id,
            "rpc_url": config.network.rpc_url,
            "ws_url": config.network.ws_url
        },
        "liquidation": {
            "min_profit_threshold_usd": config.liquidation.min_profit_threshold_usd,
            "scan_interval_seconds": config.liquidation.scan_interval_seconds,
            "max_concurrent_liquidations": config.liquidation.max_concurrent_liquidations,
            "use_flashloan": config.liquidation.use_flashloan,
            "preferred_flashloan_provider": config.liquidation.preferred_flashloan_provider.clone()
        },
        "execution": {
            "gas_price_gwei": config.execution.gas_price_gwei,
            "gas_multiplier": config.execution.gas_multiplier,
            "transaction_timeout_ms": config.execution.transaction_timeout_ms,
            "max_pending_transactions": config.execution.max_pending_transactions
        }
    }))
}

// Update Liquidation Configuration
async fn post_liquidation_config(
    core: SearcherCore,
    Json(payload): Json<Value>
) -> Json<Value> {
    info!("POST /api/liquidation/config: {:?}", payload);

    // 동적 설정 저장
    match core.config.set_strategy_config("liquidation", payload.clone()).await {
        Ok(_) => {
            info!("✅ 청산 전략 설정 저장 완료");
            Json(json!({
                "success": true,
                "message": "청산 전략 설정이 저장되었습니다",
                "updated_fields": payload
            }))
        }
        Err(e) => {
            error!("❌ 청산 전략 설정 저장 실패: {}", e);
            Json(json!({
                "success": false,
                "message": format!("설정 저장 실패: {}", e)
            }))
        }
    }
}

// 샌드위치 API 핸들러들
async fn get_sandwich_dashboard(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/sandwich/dashboard");
    
    json!({
        "total_sandwiches": 23,
        "total_profit": "0.1247",
        "active_opportunities": 3,
        "success_rate": 0.78,
        "pending_bundles": 1,
        "performance_metrics": {
            "avg_execution_time_ms": 1250.5,
            "uptime_seconds": 86400,
            "execution_success_rate": 0.78
        }
    })
}

async fn get_sandwich_opportunities(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/sandwich/opportunities");
    
    json!({
        "opportunities": [
            {
                "id": "sandwich_001",
                "dex_type": "Uniswap V2",
                "token_pair": "ETH/USDC",
                "amount": "5.2 ETH",
                "price_impact": 0.025,
                "estimated_profit": "0.0089 ETH",
                "success_probability": 0.65,
                "competition_level": "Medium",
                "detected_at": 1703123400
            },
            {
                "id": "sandwich_002",
                "dex_type": "SushiSwap",
                "token_pair": "WETH/DAI",
                "amount": "12.8 ETH",
                "price_impact": 0.018,
                "estimated_profit": "0.0156 ETH",
                "success_probability": 0.82,
                "competition_level": "Low",
                "detected_at": 1703123600
            },
            {
                "id": "sandwich_003",
                "dex_type": "Uniswap V3",
                "token_pair": "WBTC/ETH",
                "amount": "2.1 WBTC",
                "price_impact": 0.035,
                "estimated_profit": "0.0234 ETH",
                "success_probability": 0.45,
                "competition_level": "High",
                "detected_at": 1703123800
            }
        ],
        "total": 3
    })
}

async fn get_sandwich_config(config: Arc<Config>) -> Json<Value> {
    info!("GET /api/sandwich/config");
    
    json!({
        "min_value_eth": 0.1,
        "max_gas_price_gwei": 200,
        "min_profit_eth": 0.01,
        "min_profit_percentage": 0.02,
        "max_price_impact": 0.05,
        "kelly_risk_factor": 0.5,
        "contract_address": "0x0000000000000000000000000000000000000000",
        "flashbots_relay_url": "https://relay.flashbots.net",
        "gas_limit": 200000,
        "gas_per_tx": 200000,
        "front_run_priority_fee_gwei": 5,
        "back_run_priority_fee_gwei": 2,
        "priority_fee_low_gwei": 1,
        "priority_fee_medium_gwei": 2,
        "priority_fee_high_gwei": 5,
        "priority_fee_critical_gwei": 10,
        "uniswap_v2_fee": 0.003,
        "uniswap_v3_fee": 0.003,
        "default_fee": 0.003,
        "uniswap_v3_fee_tier": 3000,
        "deadline_secs": 300,
        "max_wait_blocks": 3,
        "wait_seconds": 3,
        "stats_interval_secs": 60
    })
}

async fn post_sandwich_config(
    core: SearcherCore,
    Json(payload): Json<Value>
) -> Json<Value> {
    info!("POST /api/sandwich/config: {:?}", payload);

    // 동적 설정 저장
    match core.config.set_strategy_config("sandwich", payload.clone()).await {
        Ok(_) => {
            info!("✅ 샌드위치 전략 설정 저장 완료");
            Json(json!({
                "success": true,
                "message": "샌드위치 전략 설정이 저장되었습니다",
                "updated_fields": payload
            }))
        }
        Err(e) => {
            error!("❌ 샌드위치 전략 설정 저장 실패: {}", e);
            Json(json!({
                "success": false,
                "message": format!("설정 저장 실패: {}", e)
            }))
        }
    }
}

async fn start_sandwich_strategy(core: SearcherCore) -> Json<Value> {
    info!("POST /api/sandwich/start");
    
    match core.strategy_manager.start_strategy(crate::types::StrategyType::Sandwich).await {
        Ok(_) => {
            json!({
                "success": true,
                "message": "샌드위치 전략이 성공적으로 시작되었습니다"
            })
        }
        Err(e) => {
            error!("샌드위치 전략 시작 실패: {}", e);
            json!({
                "success": false,
                "message": format!("샌드위치 전략 시작 실패: {}", e)
            })
        }
    }
}

async fn stop_sandwich_strategy(core: SearcherCore) -> Json<Value> {
    info!("POST /api/sandwich/stop");
    
    match core.strategy_manager.stop_strategy(crate::types::StrategyType::Sandwich).await {
        Ok(_) => {
            json!({
                "success": true,
                "message": "샌드위치 전략이 성공적으로 중지되었습니다"
            })
        }
        Err(e) => {
            error!("샌드위치 전략 중지 실패: {}", e);
            json!({
                "success": false,
                "message": format!("샌드위치 전략 중지 실패: {}", e)
            })
        }
    }
}

async fn get_sandwich_status(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/sandwich/status");
    json!({
        "is_running": false,
        "uptime_seconds": 0,
        "last_scan": ""
    })
}

// CEX/DEX 아비트리지 API 핸들러들
async fn get_cex_dex_arbitrage_dashboard(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/micro-arbitrage/dashboard");
    
    json!({
        "total_trades": 156,
        "total_profit": "2847.32",
        "success_rate": 0.82,
        "active_opportunities": 5,
        "pending_executions": 2,
        "performance_metrics": {
            "avg_execution_time_ms": 850.5,
            "uptime_seconds": 172800,
            "execution_success_rate": 0.82
        }
    })
}

async fn get_cex_dex_arbitrage_opportunities(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/cex-dex-arbitrage/opportunities");
    
    json!({
        "opportunities": [
            {
                "id": "micro_001",
                "pair": "ETH/USDC",
                "cex_price": 2456.78,
                "dex_price": 2458.45,
                "price_difference": 1.67,
                "estimated_profit": "12.45",
                "profit_percentage": 0.68,
                "exchange": "Binance",
                "detected_at": 1703123400
            },
            {
                "id": "micro_002",
                "pair": "BTC/USDT",
                "cex_price": 43250.12,
                "dex_price": 43280.45,
                "price_difference": 30.33,
                "estimated_profit": "8.92",
                "profit_percentage": 0.70,
                "exchange": "Coinbase",
                "detected_at": 1703123600
            },
            {
                "id": "micro_003",
                "pair": "LINK/ETH",
                "cex_price": 0.004567,
                "dex_price": 0.004589,
                "price_difference": 0.000022,
                "estimated_profit": "15.67",
                "profit_percentage": 0.48,
                "exchange": "Binance",
                "detected_at": 1703123800
            },
            {
                "id": "micro_004",
                "pair": "UNI/USDC",
                "cex_price": 6.234,
                "dex_price": 6.267,
                "price_difference": 0.033,
                "estimated_profit": "9.34",
                "profit_percentage": 0.53,
                "exchange": "Coinbase",
                "detected_at": 1703124000
            },
            {
                "id": "micro_005",
                "pair": "AAVE/ETH",
                "cex_price": 0.0234,
                "dex_price": 0.0236,
                "price_difference": 0.0002,
                "estimated_profit": "7.89",
                "profit_percentage": 0.85,
                "exchange": "Binance",
                "detected_at": 1703124200
            }
        ],
        "total": 5
    })
}

async fn get_cex_dex_arbitrage_config(config: Arc<Config>) -> Json<Value> {
    info!("GET /api/cex-dex-arbitrage/config");
    
    json!({
        "min_profit_usd": 10.0,
        "max_position_size_usd": 10000.0,
        "max_daily_volume_usd": 100000.0,
        "max_slippage_percent": 0.5,
        "max_price_impact_percent": 1.0,
        "risk_factor": 0.5,
        "funding_mode": "auto",
        "max_flashloan_fee_bps": 9,
        "gas_buffer_percent": 20.0,
        "price_update_interval_ms": 1000,
        "orderbook_refresh_interval_ms": 500,
        "opportunity_scan_interval_ms": 2000,
        "allow_aggregator_execution": true,
        "preferred_aggregators": ["0x", "1inch"],
        "max_concurrent_trades": 3,
        "execution_timeout_ms": 30000,
        "binance_api_key": "",
        "binance_secret_key": "",
        "coinbase_api_key": "",
        "coinbase_secret_key": "",
        "uniswap_v2_router": "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",
        "uniswap_v3_router": "0xE592427A0AEce92De3Edee1F18E0157C05861564",
        "sushiswap_router": "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F",
        "gas_limit": 300000,
        "gas_price_gwei": 20,
        "priority_fee_gwei": 2,
        "max_drawdown_percent": 10.0,
        "stop_loss_percent": 5.0,
        "take_profit_percent": 2.0,
        "max_daily_loss_usd": 1000.0
    })
}

async fn post_cex_dex_arbitrage_config(
    core: SearcherCore,
    Json(payload): Json<Value>
) -> Json<Value> {
    info!("POST /api/cex-dex-arbitrage/config: {:?}", payload);

    // 동적 설정 저장
    match core.config.set_strategy_config("cex_dex_arbitrage", payload.clone()).await {
        Ok(_) => {
            info!("✅ CEX/DEX 아비트리지 전략 설정 저장 완료");
            Json(json!({
                "success": true,
                "message": "CEX/DEX 아비트리지 전략 설정이 저장되었습니다",
                "updated_fields": payload
            }))
        }
        Err(e) => {
            error!("❌ CEX/DEX 아비트리지 전략 설정 저장 실패: {}", e);
            Json(json!({
                "success": false,
                "message": format!("설정 저장 실패: {}", e)
            }))
        }
    }
}

async fn start_cex_dex_arbitrage_strategy(core: SearcherCore) -> Json<Value> {
    info!("POST /api/cex-dex-arbitrage/start");
    
    match core.strategy_manager.start_strategy(crate::types::StrategyType::CexDexArbitrage).await {
        Ok(_) => {
            json!({
                "success": true,
                "message": "CEX/DEX 아비트리지 전략이 성공적으로 시작되었습니다"
            })
        }
        Err(e) => {
            error!("CEX/DEX 아비트리지 전략 시작 실패: {}", e);
            json!({
                "success": false,
                "message": format!("CEX/DEX 아비트리지 전략 시작 실패: {}", e)
            })
        }
    }
}

async fn stop_cex_dex_arbitrage_strategy(core: SearcherCore) -> Json<Value> {
    info!("POST /api/cex-dex-arbitrage/stop");
    
    match core.strategy_manager.stop_strategy(crate::types::StrategyType::CexDexArbitrage).await {
        Ok(_) => {
            json!({
                "success": true,
                "message": "CEX/DEX 아비트리지 전략이 성공적으로 중지되었습니다"
            })
        }
        Err(e) => {
            error!("CEX/DEX 아비트리지 전략 중지 실패: {}", e);
            json!({
                "success": false,
                "message": format!("CEX/DEX 아비트리지 전략 중지 실패: {}", e)
            })
        }
    }
}

async fn get_cex_dex_arbitrage_status(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/cex-dex-arbitrage/status");
    json!({
        "is_running": false,
        "uptime_seconds": 0,
        "last_scan": ""
    })
}

// 복잡한 아비트리지 API 핸들러들
async fn get_complex_arbitrage_dashboard(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/complex-arbitrage/dashboard");
    
    json!({
        "total_trades": 23,
        "total_profit": "15647.89",
        "success_rate": 0.78,
        "active_opportunities": 4,
        "pending_executions": 1,
        "performance_metrics": {
            "avg_execution_time_ms": 2150.5,
            "uptime_seconds": 259200,
            "execution_success_rate": 0.78
        }
    })
}

async fn get_complex_arbitrage_opportunities(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/complex-arbitrage/opportunities");
    
    json!({
        "opportunities": [
            {
                "id": "complex_001",
                "strategy": "triangular",
                "path": "ETH → USDC → DAI → ETH",
                "assets": ["ETH", "USDC", "DAI"],
                "estimated_profit": "245.67",
                "profit_percentage": 1.85,
                "complexity": "medium",
                "detected_at": 1703123400
            },
            {
                "id": "complex_002",
                "strategy": "position_migration",
                "path": "Aave → Compound",
                "assets": ["USDC", "USDT"],
                "estimated_profit": "1250.34",
                "profit_percentage": 2.45,
                "complexity": "high",
                "detected_at": 1703123600
            },
            {
                "id": "complex_003",
                "strategy": "complex",
                "path": "ETH → USDC → DAI → USDT → ETH",
                "assets": ["ETH", "USDC", "DAI", "USDT"],
                "estimated_profit": "890.12",
                "profit_percentage": 3.12,
                "complexity": "high",
                "detected_at": 1703123800
            },
            {
                "id": "complex_004",
                "strategy": "triangular",
                "path": "WBTC → ETH → USDC → WBTC",
                "assets": ["WBTC", "ETH", "USDC"],
                "estimated_profit": "567.89",
                "profit_percentage": 1.23,
                "complexity": "low",
                "detected_at": 1703124000
            }
        ],
        "total": 4
    })
}

async fn get_complex_arbitrage_config(config: Arc<Config>) -> Json<Value> {
    info!("GET /api/complex-arbitrage/config");
    
    json!({
        "min_profit_usd": 50.0,
        "max_position_size_usd": 100000.0,
        "max_path_length": 5,
        "min_profit_percentage": 0.5,
        "max_concurrent_trades": 2,
        "execution_timeout_ms": 60000,
        "strategies": ["triangular", "position_migration", "complex"],
        "flashloan_protocols": ["aave_v3"],
        "max_flashloan_fee_bps": 9,
        "gas_buffer_pct": 25.0,
        "max_drawdown_percent": 15.0,
        "stop_loss_percent": 8.0,
        "take_profit_percent": 3.0,
        "max_daily_loss_usd": 5000.0,
        "max_gas_price_gwei": 100,
        "priority_fee_gwei": 5,
        "deadline_secs": 300,
        "aave_v3_pool": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
        "compound_comptroller": "0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B",
        "uniswap_v2_factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
        "uniswap_v3_factory": "0x1F98431c8aD98523631AE4a59f267346ea31F984"
    })
}

async fn post_complex_arbitrage_config(
    core: SearcherCore,
    Json(payload): Json<Value>
) -> Json<Value> {
    info!("POST /api/complex-arbitrage/config: {:?}", payload);

    // 동적 설정 저장
    match core.config.set_strategy_config("complex_arbitrage", payload.clone()).await {
        Ok(_) => {
            info!("✅ 복잡한 아비트리지 전략 설정 저장 완료");
            Json(json!({
                "success": true,
                "message": "복잡한 아비트리지 전략 설정이 저장되었습니다",
                "updated_fields": payload
            }))
        }
        Err(e) => {
            error!("❌ 복잡한 아비트리지 전략 설정 저장 실패: {}", e);
            Json(json!({
                "success": false,
                "message": format!("설정 저장 실패: {}", e)
            }))
        }
    }
}

async fn start_complex_arbitrage_strategy(core: SearcherCore) -> Json<Value> {
    info!("POST /api/complex-arbitrage/start");
    
    match core.strategy_manager.start_strategy(crate::types::StrategyType::ComplexArbitrage).await {
        Ok(_) => {
            json!({
                "success": true,
                "message": "복잡한 아비트리지 전략이 성공적으로 시작되었습니다"
            })
        }
        Err(e) => {
            error!("복잡한 아비트리지 전략 시작 실패: {}", e);
            json!({
                "success": false,
                "message": format!("복잡한 아비트리지 전략 시작 실패: {}", e)
            })
        }
    }
}

async fn stop_complex_arbitrage_strategy(core: SearcherCore) -> Json<Value> {
    info!("POST /api/complex-arbitrage/stop");
    
    match core.strategy_manager.stop_strategy(crate::types::StrategyType::ComplexArbitrage).await {
        Ok(_) => {
            json!({
                "success": true,
                "message": "복잡한 아비트리지 전략이 성공적으로 중지되었습니다"
            })
        }
        Err(e) => {
            error!("복잡한 아비트리지 전략 중지 실패: {}", e);
            json!({
                "success": false,
                "message": format!("복잡한 아비트리지 전략 중지 실패: {}", e)
            })
        }
    }
}

async fn get_complex_arbitrage_status(_core: SearcherCore) -> Json<Value> {
    info!("GET /api/complex-arbitrage/status");
    json!({
        "is_running": false,
        "uptime_seconds": 0,
        "last_scan": ""
    })
}
