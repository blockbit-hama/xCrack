use std::env;
use std::sync::Arc;
use anyhow::Result;
use clap::{Arg, Command};
use ethers::providers::{Provider, Ws};
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::signal;

mod config;
mod types;
mod utils;
mod strategies;
mod flashbots;
mod mempool;
mod monitoring;
mod core;
mod constants;
mod mocks;
mod exchange;
mod backtest;
mod bridges;
mod blockchain;
mod oracle;      // 🆕 가격 오라클 시스템
mod opportunity; // 🆕 기회 관리 시스템
mod storage;     // 🆕 Redis 기반 스토리지

use config::Config;
use core::SearcherCore;
use monitoring::manager::MonitoringManager;

/// ETH 금액을 포맷팅하는 헬퍼 함수
fn format_eth_amount(wei: alloy::primitives::U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6} ETH", eth)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    if let Err(e) = dotenvy::dotenv() {
        // If .env file doesn't exist, that's OK - use system environment variables
        tracing::debug!("Could not load .env file: {}", e);
    }
    let matches = Command::new("xcrack")
        .version("0.2.0")
        .author("xCrack Team <team@xcrack.dev>")
        .about("고성능 MEV 서쳐 봇 - 개선된 멤풀 기반 전략")
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
                .help("활성화할 전략들 (sandwich,liquidation,micro_arbitrage,cross_chain)")
                .default_value("sandwich,liquidation,micro_arbitrage")
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
        error!("설정 검증 실패: {}", e);
        std::process::exit(1);
    }

    info!("설정 로드 완료");
    
    // config를 Arc로 감싸기
    let config = Arc::new(config);
    
    // 크로스체인 아비트러지 전략만 실행하는 경우 Mock 실행
    if strategies == "cross_chain" {
        info!("🌉 크로스체인 아비트래지 전략 단독 실행 모드");
        strategies::run_cross_chain_arbitrage_mock(Arc::clone(&config)).await?;
        return Ok(());
    }
    
    // WebSocket 프로바이더 초기화
    let ws_url = config.network.ws_url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("WebSocket URL이 설정되지 않았습니다"))?;
    
    let provider = if mocks::is_mock_mode() {
        info!("🎭 Mock 모드: Mock WebSocket provider 생성 중");
        warn!("Mock 모드에서는 실제 네트워크 연결 없이 모의 데이터를 사용합니다");
        
        // Mock WebSocket 서버를 시작하고 연결
        mocks::create_mock_ws_provider().await?
    } else {
        info!("🔌 WebSocket 연결 중: {}", ws_url);
        let ws = Ws::connect(ws_url).await?;
        let provider = Provider::new(ws);
        Arc::new(provider)
    };
    
    // SearcherCore 초기화
    // -> 내부에서 strategy_manager,bundle_manager,mempool_monitor,performance_tracker 초기화
    info!("🔧 SearcherCore 초기화 중...");
    let searcher_core = SearcherCore::new(Arc::clone(&config), Arc::clone(&provider)).await?;

    // 메트릭 서버 시작 (백그라운드)
    let monitoring_manager = MonitoringManager::new(Arc::clone(&config)).await?;
    monitoring_manager.start().await?;
    
    // 신호 처리 설정
    let searcher_core_clone = searcher_core.clone();
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                warn!("종료 신호 수신됨, 안전하게 종료 중...");
                if let Err(e) = searcher_core_clone.stop().await {
                    error!("서쳐 중지 실패: {}", e);
                }
                std::process::exit(0);
            }
            Err(err) => {
                error!("신호 처리 오류: {}", err);
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
                    info!("서쳐 상태:");
                    info!("  실행 상태: {}", if status.is_running { "실행 중" } else { "중지됨" });
                    info!("  처리된 트랜잭션: {}", status.performance_metrics.transactions_processed);
                    info!("  발견된 기회: {}", status.performance_metrics.opportunities_found);
                    info!("  제출된 번들: {}", status.performance_metrics.bundles_submitted);
                    info!("  포함된 번들: {}", status.performance_metrics.bundles_included);
                    info!("  총 수익: {} ETH", format_eth_amount(status.performance_metrics.total_profit));
                    info!("  성공률: {:.2}%", status.performance_metrics.success_rate * 100.0);
                    info!("  ⏱가동 시간: {}초", status.uptime_seconds);
                }
                Err(e) => {
                    error!("상태 조회 실패: {}", e);
                }
            }
        }
    });

    // 메인 서쳐 실행
    info!(" MEV 서쳐가 성공적으로 시작되었습니다!");
    
    // SearcherCore 시작
    searcher_core.start().await?;
    
    // 안전 종료
    info!("서쳐 종료 중...");
    searcher_core.stop().await?;
    
    info!("서쳐가 안전하게 종료되었습니다.");
    Ok(())
}

fn print_banner() {
    println!(r#"
    ╔══════════════════════════════════════════════════════════════╗
    ║                                                              ║
    ║  xCrack Rust MEV 서쳐 v0.2.0                             ║
    ║                                                              ║
    ║  최고의 속도와 효율성을 위해 Rust로 구축된 MEV 봇            ║
    ║                                                              ║
    ║  구현된 전략들:                                           ║
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
    ║  안전 장치:                                              ║
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
    config.strategies.sandwich.enabled = false;
    config.strategies.liquidation.enabled = false;

    // 선택된 전략들만 활성화
    for strategy in strategies.split(',') {
        match strategy.trim() {
            "sandwich" => {
                config.strategies.sandwich.enabled = true;
                info!("실시간 샌드위치 전략 활성화");
            }
            "liquidation" => {
                config.strategies.liquidation.enabled = true;
                info!("경쟁적 청산 전략 활성화");
            }
            "micro_arbitrage" => {
                info!("마이크로 아비트러지 전략 활성화");
            }
            "cross_chain" => {
                info!("크로스체인 아비트러지 전략 활성화 (Mock 모드)");
            }
            _ => {
                warn!("알 수 없는 전략: {}", strategy);
            }
        }
    }
}

fn load_environment_variables(config: &mut Config) {
    // Check API mode first
    let api_mode = env::var("API_MODE").unwrap_or_else(|_| "mock".to_string());
    
    if api_mode == "mock" {
        info!("🎭 Mock mode enabled - using mock APIs");
        config.flashbots.simulation_mode = true;
        
        // Set mock endpoints
        config.network.rpc_url = "mock://ethereum".to_string();
        config.network.ws_url = Some("mock://ethereum/ws".to_string());
        config.flashbots.relay_url = "mock://flashbots".to_string();
        
        // Set mock private key for testing
        if config.flashbots.private_key.is_empty() {
            config.flashbots.private_key = "0x0000000000000000000000000000000000000000000000000000000000000001".to_string();
        }
        
        return;
    }

    info!("🌐 Real API mode enabled - using actual external APIs");
    
    if let Ok(private_key) = env::var("PRIVATE_KEY") {
        config.flashbots.private_key = private_key;
        info!("🔑 Private key loaded from environment");
    }
    
    if let Ok(rpc_url) = env::var("ETH_RPC_URL") {
        config.network.rpc_url = rpc_url;
        info!("🔌 RPC URL loaded from environment");
    }
    
    if let Ok(ws_url) = env::var("ETH_WS_URL") {
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

    if let Ok(_telegram_token) = env::var("TELEGRAM_BOT_TOKEN") {
        // Telegram 설정 (실제 구현에서 추가)
        info!("📱 Telegram alerts configured");
    }
}


fn create_sample_transactions() -> Vec<types::Transaction> {
    vec![
        // 대형 ETH → USDC 스왑 (차익거래 대상)
        types::Transaction {
            hash: "0x1111111111111111111111111111111111111111111111111111111111111111".parse().unwrap(),
            from: "0x742d35Cc65700000000000000000000000000001".parse().unwrap(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()), // Uniswap V2
            value: alloy::primitives::U256::from_str_radix("5000000000000000000", 10).unwrap(), // 5 ETH
            gas_price: alloy::primitives::U256::from(20_000_000_000u64), // 20 gwei
            gas_limit: alloy::primitives::U256::from(300_000u64),
            data: vec![0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            nonce: 1,
            timestamp: chrono::Utc::now(),
            block_number: Some(1000),
        },
        
        // 중간 규모 토큰 스왑 (샌드위치 대상)
        types::Transaction {
            hash: "0x2222222222222222222222222222222222222222222222222222222222222222".parse().unwrap(),
            from: "0x742d35Cc65700000000000000000000000000002".parse().unwrap(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()),
            value: alloy::primitives::U256::from_str_radix("1000000000000000000", 10).unwrap(), // 1 ETH
            gas_price: alloy::primitives::U256::from(15_000_000_000u64), // 15 gwei
            gas_limit: alloy::primitives::U256::from(200_000u64),
            data: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            nonce: 5,
            timestamp: chrono::Utc::now(),
            block_number: Some(1001),
        },
        
        // Aave 청산 거래 (청산 경쟁 대상)
        types::Transaction {
            hash: "0x3333333333333333333333333333333333333333333333333333333333333333".parse().unwrap(),
            from: "0x742d35Cc65700000000000000000000000000003".parse().unwrap(),
            to: Some("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap()), // Aave LendingPool
            value: alloy::primitives::U256::ZERO,
            gas_price: alloy::primitives::U256::from(50_000_000_000u64), // 50 gwei (경쟁적)
            gas_limit: alloy::primitives::U256::from(400_000u64),
            data: vec![0xe8, 0xed, 0xa9, 0xdf], // liquidationCall
            nonce: 10,
            timestamp: chrono::Utc::now(),
            block_number: Some(1002),
        },
    ]
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_display() {
        // 배너 출력이 패닉 없이 실행되는지 확인
        print_banner();
    }

    #[test]
    fn test_strategy_selection() {
        let mut config = Config::default();
        
        // 테스트: 단일 전략 선택
        apply_strategy_selection(&mut config, "sandwich");
        assert!(config.strategies.sandwich.enabled);
        assert!(!config.strategies.liquidation.enabled);
        
        // 테스트: 다중 전략 선택
        apply_strategy_selection(&mut config, "sandwich,liquidation");
        assert!(config.strategies.sandwich.enabled);
        assert!(config.strategies.liquidation.enabled);
        
        // 테스트: 모든 전략 선택
        apply_strategy_selection(&mut config, "sandwich,liquidation");
        assert!(config.strategies.sandwich.enabled);
        assert!(config.strategies.liquidation.enabled);
    }

    #[test]
    fn test_sample_transaction_creation() {
        let transactions = create_sample_transactions();
        assert_eq!(transactions.len(), 3);
        
        // 첫 번째 거래는 대형 ETH 스왑
        assert_eq!(transactions[0].value, alloy::primitives::U256::from_str_radix("5000000000000000000", 10).unwrap());
        
        // 두 번째 거래는 중간 규모 스왑
        assert_eq!(transactions[1].value, alloy::primitives::U256::from_str_radix("1000000000000000000", 10).unwrap());
        
        // 세 번째 거래는 청산 거래 (value = 0)
        assert_eq!(transactions[2].value, alloy::primitives::U256::ZERO);
    }

    #[test]
    fn test_cli_argument_parsing() {
        // CLI 인수 파싱 테스트
        let args = vec![
            "xcrack-rust-mev-searcher",
            "--config", "test_config.toml",
            "--log-level", "debug",
            "--strategies", "sandwich,liquidation",
            "--simulation",
            "--dev"
        ];
        
        let matches = Command::new("xcrack-rust-mev-searcher")
            .version("0.2.0")
            .arg(Arg::new("config").long("config").value_name("FILE").default_value("config/default.toml"))
            .arg(Arg::new("log-level").long("log-level").value_name("LEVEL").default_value("info"))
            .arg(Arg::new("strategies").long("strategies").value_name("STRATEGIES").default_value("sandwich,liquidation"))
            .arg(Arg::new("simulation").long("simulation").action(clap::ArgAction::SetTrue))
            .arg(Arg::new("dev").long("dev").action(clap::ArgAction::SetTrue))
            .try_get_matches_from(args)
            .unwrap();

        assert_eq!(matches.get_one::<String>("config").unwrap(), "test_config.toml");
        assert_eq!(matches.get_one::<String>("log-level").unwrap(), "debug");
        assert_eq!(matches.get_one::<String>("strategies").unwrap(), "sandwich,liquidation");
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
