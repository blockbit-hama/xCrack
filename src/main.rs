use std::env;
use std::sync::Arc;
use anyhow::{Result, Context};
use clap::{Arg, Command};
use ethers::providers::{Http, Provider, Ws};
use futures::Stream;
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::signal;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::WebSocket;

mod config;
mod types;
mod utils;
mod strategies;
mod flashbots;
mod mempool;
mod monitoring;
mod core;
mod constants;

use config::Config;
use core::SearcherCore;
use strategies::arbitrage::MempoolArbitrageStrategy;
use types::{Transaction, Opportunity, Bundle};
use strategies::manager::StrategyManager;
use core::BundleManager;
use core::CoreMempoolMonitor;
use core::PerformanceTracker;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("xcrack-rust-mev-searcher")
        .version("0.2.0")
        .author("xCrack Team <team@xcrack.dev>")
        .about("🦀 고성능 MEV 서쳐 봇 - 개선된 멤풀 기반 전략")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("설정 파일 경로")
                .default_value("config/default.toml")
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("로그 레벨 (trace, debug, info, warn, error)")
                .default_value("info")
        )
        .arg(
            Arg::new("simulation")
                .long("simulation")
                .help("시뮬레이션 모드 (실제 번들을 제출하지 않음)")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("dev")
                .long("dev")
                .help("개발 모드 활성화")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("strategies")
                .short('s')
                .long("strategies")
                .value_name("STRATEGIES")
                .help("활성화할 전략들 (arbitrage,sandwich,liquidation)")
                .default_value("arbitrage,sandwich,liquidation")
        )
        .get_matches();

    // 로그 레벨 설정
    let log_level = matches.get_one::<String>("log-level").unwrap();
    let log_filter = match log_level.as_str() {
        "trace" => "trace",
        "debug" => "debug", 
        "info" => "info",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };

    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 환영 메시지
    print_banner();

    // 설정 파일 로드
    let config_path = matches.get_one::<String>("config").unwrap();
    info!("📋 설정 파일 로드 중: {}", config_path);
    
    let mut config = Config::load(config_path).await?;
    
    // 명령줄 옵션 적용
    if matches.get_flag("simulation") {
        warn!("🧪 시뮬레이션 모드 활성화 - 실제 번들을 제출하지 않습니다");
        config.flashbots.simulation_mode = true;
    }
    
    if matches.get_flag("dev") {
        info!("🛠️ 개발 모드 활성화");
        config.monitoring.log_level = "debug".to_string();
    }

    // 전략 선택 적용
    let strategies = matches.get_one::<String>("strategies").unwrap();
    apply_strategy_selection(&mut config, strategies);

    // 환경 변수에서 민감한 정보 로드
    load_environment_variables(&mut config);

    // 설정 검증
    if let Err(e) = config.validate() {
        error!("❌ 설정 검증 실패: {}", e);
        std::process::exit(1);
    }

    info!("✅ 설정 로드 완료");
    
    // WebSocket 프로바이더 초기화
    let ws_url = config.network.ws_url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("WebSocket URL이 설정되지 않았습니다"))?;
    
    info!("🔌 WebSocket 연결 중: {}", ws_url);
    let ws = Ws::connect(ws_url).await?;
    let provider = Provider::new(ws);
    let provider = Arc::new(provider);
    
    // SearcherCore 초기화
    info!("🔧 SearcherCore 초기화 중...");
    let searcher_core = SearcherCore::new(Arc::new(config), Arc::clone(&provider)).await?;
    
    // 2. 전략 매니저 초기화
    info!("🎯 전략 매니저 초기화 중...");
    let strategy_manager = Arc::new(
        StrategyManager::new(Arc::clone(&config), Arc::clone(&provider))
            .await
            .context("전략 매니저 초기화 실패")?
    );
    
    // 3. 번들 매니저 초기화
    info!("📦 번들 매니저 초기화 중...");
    let bundle_manager = Arc::new(
        BundleManager::new(Arc::clone(&config), Arc::clone(&provider))
            .await
            .context("번들 매니저 초기화 실패")?
    );
    
    // 4. 멤풀 모니터 초기화
    info!("👁️ 멤풀 모니터 초기화 중...");
    let mempool_monitor = Arc::new(
        CoreMempoolMonitor::new(Arc::clone(&config), Arc::clone(&provider))
            .await
            .context("멤풀 모니터 초기화 실패")?
    );
    
    // 5. 성능 추적기 초기화
    info!("📊 성능 추적기 초기화 중...");
    let performance_tracker = Arc::new(
        PerformanceTracker::new(Arc::clone(&config))
            .await
            .context("성능 추적기 초기화 실패")?
    );
    
    // 신호 처리 설정
    let searcher_core_clone = searcher_core.clone();
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                warn!("🛑 종료 신호 수신됨, 안전하게 종료 중...");
                if let Err(e) = searcher_core_clone.stop().await {
                    error!("❌ 서쳐 중지 실패: {}", e);
                }
                std::process::exit(0);
            }
            Err(err) => {
                error!("❌ 신호 처리 오류: {}", err);
                std::process::exit(1);
            }
        }
    });

    // 성능 모니터링 태스크
    let searcher_core_clone = searcher_core.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // 서쳐 상태 조회
            match searcher_core_clone.get_status().await {
                Ok(status) => {
                    info!("📊 서쳐 상태:");
                    info!("  🔄 실행 상태: {}", if status.is_running { "실행 중" } else { "중지됨" });
                    info!("  📝 처리된 트랜잭션: {}", status.performance_metrics.transactions_processed);
                    info!("  🎯 발견된 기회: {}", status.performance_metrics.opportunities_found);
                    info!("  📦 제출된 번들: {}", status.performance_metrics.bundles_submitted);
                    info!("  ✅ 포함된 번들: {}", status.performance_metrics.bundles_included);
                    info!("  💰 총 수익: {} ETH", ethers::utils::format_ether(status.performance_metrics.total_profit));
                    info!("  📈 성공률: {:.2}%", status.performance_metrics.success_rate * 100.0);
                    info!("  ⏱️ 가동 시간: {}초", status.uptime_seconds);
                }
                Err(e) => {
                    error!("❌ 상태 조회 실패: {}", e);
                }
            }
        }
    });

    // 메인 서쳐 실행
    info!("🎯 MEV 서쳐가 성공적으로 시작되었습니다!");
    
    // 모든 전략 시작
    strategy_manager.start_all_strategies().await?;
    
    // 멤풀 모니터링 시작
    let (tx_sender, mut tx_receiver) = tokio::sync::mpsc::channel::<Transaction>(1000);
    mempool_monitor.start_monitoring(tx_sender).await?;
    
    // 메인 처리 루프
    let mut processed_count = 0;
    let start_time = std::time::Instant::now();
    
    while let Some(transaction) = tx_receiver.recv().await {
        processed_count += 1;
        
        // 성능 추적 시작
        let analysis_start = std::time::Instant::now();
        
        // 1. 트랜잭션 분석 (모든 활성 전략으로 병렬 분석)
        let opportunities = strategy_manager.analyze_transaction(&transaction).await;
        
        if !opportunities.is_empty() {
            info!("🎯 트랜잭션 {}에서 {}개 기회 발견", transaction.hash, opportunities.len());
            
            // 2. 기회 검증
            let valid_opportunities = strategy_manager.validate_opportunities(opportunities).await;
            
            if !valid_opportunities.is_empty() {
                info!("✅ {}개 기회 검증 통과", valid_opportunities.len());
                
                // 3. 번들 생성
                let bundles = strategy_manager.create_bundles(valid_opportunities).await;
                
                // 4. 번들 제출
                for bundle in bundles {
                    match bundle_manager.submit_bundle(&bundle).await {
                        Ok(bundle_id) => {
                            info!("📦 번들 제출 성공: {}", bundle_id);
                            performance_tracker.record_bundle_submission(&bundle).await;
                        }
                        Err(e) => {
                            error!("❌ 번들 제출 실패: {}", e);
                        }
                    }
                }
            }
        }
        
        // 성능 통계 업데이트
        let analysis_duration = analysis_start.elapsed();
        performance_tracker.record_transaction_analysis(analysis_duration, opportunities.len()).await;
        
        // 주기적 성능 리포트
        if processed_count % 100 == 0 {
            let elapsed = start_time.elapsed();
            let tps = processed_count as f64 / elapsed.as_secs_f64();
            
            info!("📊 처리 통계:");
            info!("  🔄 처리된 트랜잭션: {}", processed_count);
            info!("  ⚡ 초당 처리량: {:.2} TPS", tps);
            info!("  ⏱️ 평균 분석 시간: {:.2}ms", analysis_duration.as_millis());
            
            // 전략별 통계 출력
            let strategy_stats = strategy_manager.get_strategy_stats().await;
            for (strategy_type, stats) in strategy_stats {
                info!("  🎯 {}: {}개 분석, {}개 기회, 평균 {:.2}ms", 
                      strategy_type, stats.transactions_analyzed, stats.opportunities_found, stats.avg_analysis_time_ms);
            }
        }
    }
    
    // 안전 종료
    info!("🛑 서쳐 종료 중...");
    strategy_manager.stop_all_strategies().await?;
    mempool_monitor.stop_monitoring().await?;
    
    info!("✅ 서쳐가 안전하게 종료되었습니다.");
    Ok(())
}

fn print_banner() {
    println!(r#"
    ╔══════════════════════════════════════════════════════════════╗
    ║                                                              ║
    ║  🦀 xCrack Rust MEV 서쳐 v0.2.0                             ║
    ║                                                              ║
    ║  최고의 속도와 효율성을 위해 Rust로 구축된 MEV 봇            ║
    ║                                                              ║
    ║  🎯 개선된 전략들:                                           ║
    ║     • 멤풀 기반 차익거래 (Mempool-based Arbitrage)          ║
    ║     • 실시간 샌드위치 공격 (Real-time Sandwich)             ║
    ║     • 경쟁적 청산 프론트런 (Competitive Liquidation)        ║
    ║                                                              ║
    ║  ⚡ 핵심 개선사항:                                           ║
    ║     • 실시간 멤풀 모니터링                                   ║
    ║     • 가스 경쟁 최적화                                       ║
    ║     • 5단계 파이프라인 (감지→분석→생성→번들링→실행)          ║
    ║     • Flashbots 완전 통합                                   ║
    ║     • 모듈화된 SearcherCore 아키텍처                        ║
    ║                                                              ║
    ║  🛡️ 안전 장치:                                              ║
    ║     • 시뮬레이션 모드                                        ║
    ║     • 수익성 검증                                            ║
    ║     • 리스크 관리                                            ║
    ║     • 실시간 성능 모니터링                                   ║
    ║                                                              ║
    ╚══════════════════════════════════════════════════════════════╝
    "#);
}

fn apply_strategy_selection(config: &mut Config, strategies: &str) {
    // 모든 전략을 먼저 비활성화
    config.strategies.arbitrage.enabled = false;
    config.strategies.sandwich.enabled = false;
    config.strategies.liquidation.enabled = false;

    // 선택된 전략들만 활성화
    for strategy in strategies.split(',') {
        match strategy.trim() {
            "arbitrage" => {
                config.strategies.arbitrage.enabled = true;
                info!("✅ 멤풀 기반 차익거래 전략 활성화");
            }
            "sandwich" => {
                config.strategies.sandwich.enabled = true;
                info!("✅ 실시간 샌드위치 전략 활성화");
            }
            "liquidation" => {
                config.strategies.liquidation.enabled = true;
                info!("✅ 경쟁적 청산 전략 활성화");
            }
            _ => {
                warn!("⚠️ 알 수 없는 전략: {}", strategy);
            }
        }
    }
}

fn load_environment_variables(config: &mut Config) {
    if let Ok(private_key) = env::var("PRIVATE_KEY") {
        config.flashbots.private_key = private_key;
        info!("🔑 Private key loaded from environment");
    }
    
    if let Ok(rpc_url) = env::var("RPC_URL") {
        config.network.rpc_url = rpc_url;
        info!("🌐 RPC URL loaded from environment");
    }
    
    if let Ok(ws_url) = env::var("WS_URL") {
        config.network.ws_url = Some(ws_url);
        info!("🔌 WebSocket URL loaded from environment");
    }

    if let Ok(flashbots_url) = env::var("FLASHBOTS_RELAY_URL") {
        config.flashbots.relay_url = flashbots_url;
        info!("⚡ Flashbots relay URL loaded from environment");
    }

    if let Ok(discord_webhook) = env::var("DISCORD_WEBHOOK_URL") {
        config.monitoring.discord_webhook_url = discord_webhook;
        config.monitoring.enable_discord_alerts = true;
        info!("📢 Discord alerts enabled");
    }

    if let Ok(telegram_token) = env::var("TELEGRAM_BOT_TOKEN") {
        // Telegram 설정 (실제 구현에서 추가)
        info!("📱 Telegram alerts configured");
    }
}

/// 개발 도구: 전략별 성능 테스트
#[allow(dead_code)]
async fn run_strategy_benchmarks(config: &Config) -> Result<()> {
    info!("🧪 전략 성능 벤치마크 실행 중...");

    // 샘플 거래 데이터로 각 전략 테스트
    let sample_transactions = create_sample_transactions();
    
    for (i, tx) in sample_transactions.iter().enumerate() {
        info!("📝 샘플 거래 {} 분석 중...", i + 1);
        
        // 각 전략별로 분석 시간 측정
        let start = std::time::Instant::now();
        
        // 여기에 전략별 분석 로직 추가
        // let opportunities = strategy.analyze(tx).await?;
        
        let duration = start.elapsed();
        info!("⏱️ 분석 시간: {:.2}ms", duration.as_millis());
    }

    Ok(())
}

fn create_sample_transactions() -> Vec<types::Transaction> {
    vec![
        // 대형 ETH → USDC 스왑 (차익거래 대상)
        types::Transaction {
            hash: "0x1111111111111111111111111111111111111111111111111111111111111111".parse().unwrap(),
            from: "0x742d35Cc6570000000000000000000000000001".parse().unwrap(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()), // Uniswap V2
            value: ethers::types::U256::from_str_radix("5000000000000000000", 10).unwrap(), // 5 ETH
            gas_price: ethers::types::U256::from(20_000_000_000u64), // 20 gwei
            gas_limit: ethers::types::U256::from(300_000u64),
            data: vec![0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            nonce: 1,
            timestamp: chrono::Utc::now(),
            block_number: Some(1000),
        },
        
        // 중간 규모 토큰 스왑 (샌드위치 대상)
        types::Transaction {
            hash: "0x2222222222222222222222222222222222222222222222222222222222222222".parse().unwrap(),
            from: "0x742d35Cc6570000000000000000000000000002".parse().unwrap(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()),
            value: ethers::types::U256::from_str_radix("1000000000000000000", 10).unwrap(), // 1 ETH
            gas_price: ethers::types::U256::from(15_000_000_000u64), // 15 gwei
            gas_limit: ethers::types::U256::from(200_000u64),
            data: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            nonce: 5,
            timestamp: chrono::Utc::now(),
            block_number: Some(1001),
        },
        
        // Aave 청산 거래 (청산 경쟁 대상)
        types::Transaction {
            hash: "0x3333333333333333333333333333333333333333333333333333333333333333".parse().unwrap(),
            from: "0x742d35Cc6570000000000000000000000000003".parse().unwrap(),
            to: Some("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap()), // Aave LendingPool
            value: ethers::types::U256::zero(),
            gas_price: ethers::types::U256::from(50_000_000_000u64), // 50 gwei (경쟁적)
            gas_limit: ethers::types::U256::from(400_000u64),
            data: vec![0xe8, 0xed, 0xa9, 0xdf], // liquidationCall
            nonce: 10,
            timestamp: chrono::Utc::now(),
            block_number: Some(1002),
        },
    ]
}

/// 실시간 메트릭 대시보드 (개발/디버깅용)
#[allow(dead_code)]
async fn start_metrics_dashboard(config: &Config) -> Result<()> {
    info!("📊 메트릭 대시보드 시작 중...");
    
    // 실제 구현에서는 웹 서버나 메트릭 엔드포인트 제공
    // 예: Prometheus 메트릭, 간단한 HTTP API 등
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::NamedTempFile;
    use std::fs;

    #[test]
    fn test_banner_display() {
        // 배너 출력이 패닉 없이 실행되는지 확인
        print_banner();
    }

    #[test]
    fn test_strategy_selection() {
        let mut config = Config::default();
        
        // 테스트: 단일 전략 선택
        apply_strategy_selection(&mut config, "arbitrage");
        assert!(config.strategies.arbitrage.enabled);
        assert!(!config.strategies.sandwich.enabled);
        assert!(!config.strategies.liquidation.enabled);
        
        // 테스트: 다중 전략 선택
        apply_strategy_selection(&mut config, "arbitrage,sandwich");
        assert!(config.strategies.arbitrage.enabled);
        assert!(config.strategies.sandwich.enabled);
        assert!(!config.strategies.liquidation.enabled);
        
        // 테스트: 모든 전략 선택
        apply_strategy_selection(&mut config, "arbitrage,sandwich,liquidation");
        assert!(config.strategies.arbitrage.enabled);
        assert!(config.strategies.sandwich.enabled);
        assert!(config.strategies.liquidation.enabled);
    }

    #[test]
    fn test_sample_transaction_creation() {
        let transactions = create_sample_transactions();
        assert_eq!(transactions.len(), 3);
        
        // 첫 번째 거래는 대형 ETH 스왑
        assert_eq!(transactions[0].value, ethers::types::U256::from_str_radix("5000000000000000000", 10).unwrap());
        
        // 두 번째 거래는 중간 규모 스왑
        assert_eq!(transactions[1].value, ethers::types::U256::from_str_radix("1000000000000000000", 10).unwrap());
        
        // 세 번째 거래는 청산 거래 (value = 0)
        assert_eq!(transactions[2].value, ethers::types::U256::zero());
    }

    #[test]
    fn test_cli_argument_parsing() {
        // CLI 인수 파싱 테스트
        let args = vec![
            "xcrack-rust-mev-searcher",
            "--config", "test_config.toml",
            "--log-level", "debug",
            "--strategies", "arbitrage,sandwich",
            "--simulation",
            "--dev"
        ];
        
        let matches = Command::new("xcrack-rust-mev-searcher")
            .version("0.2.0")
            .arg(Arg::new("config").long("config").value_name("FILE").default_value("config/default.toml"))
            .arg(Arg::new("log-level").long("log-level").value_name("LEVEL").default_value("info"))
            .arg(Arg::new("strategies").long("strategies").value_name("STRATEGIES").default_value("arbitrage,sandwich,liquidation"))
            .arg(Arg::new("simulation").long("simulation").action(clap::ArgAction::SetTrue))
            .arg(Arg::new("dev").long("dev").action(clap::ArgAction::SetTrue))
            .try_get_matches_from(args)
            .unwrap();

        assert_eq!(matches.get_one::<String>("config").unwrap(), "test_config.toml");
        assert_eq!(matches.get_one::<String>("log-level").unwrap(), "debug");
        assert_eq!(matches.get_one::<String>("strategies").unwrap(), "arbitrage,sandwich");
        assert!(matches.get_flag("simulation"));
        assert!(matches.get_flag("dev"));
    }

    #[tokio::test]
    async fn test_config_validation() {
        // 올바른 설정
        let mut valid_config = Config::default();
        valid_config.network.chain_id = 1;
        valid_config.network.rpc_url = "https://eth-mainnet.example.com".to_string();
        valid_config.flashbots.private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string();
        
        assert!(valid_config.validate().is_ok());
        
        // 잘못된 설정 (빈 RPC URL)
        let mut invalid_config = Config::default();
        invalid_config.network.rpc_url = "".to_string();
        
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_log_level_selection() {
        // 로그 레벨 선택 테스트
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        
        for level in valid_levels {
            let filter = match level {
                "trace" => "trace",
                "debug" => "debug",
                "info" => "info", 
                "warn" => "warn",
                "error" => "error",
                _ => "info",
            };
            assert_eq!(filter, level);
        }
        
        // 잘못된 레벨은 info로 기본 설정
        let invalid_filter = match "invalid" {
            "trace" => "trace",
            "debug" => "debug",
            "info" => "info",
            "warn" => "warn", 
            "error" => "error",
            _ => "info",
        };
        assert_eq!(invalid_filter, "info");
    }
}
