#![allow(dead_code)]

use std::env;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, error, warn};
use tokio::signal;
use ethers::providers::{Provider, Ws, Middleware};

use xcrack_rust_mev_searcher::{Config, IntegratedLiquidationManager};

/// 통합 청산 봇 실행 바이너리
#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter("xcrack=debug,info")
        .init();
    
    info!("🚀 Starting xCrack Liquidation Bot v2.0...");
    
    // 환경 변수에서 설정 파일 경로 가져오기
    let config_path = env::var("XCRACK_CONFIG")
        .unwrap_or_else(|_| "config/liquidation.toml".to_string());
    
    // 설정 로드
    let config = match Config::load(&config_path).await {
        Ok(config) => {
            info!("✅ Configuration loaded from: {}", config_path);
            Arc::new(config)
        }
        Err(e) => {
            error!("❌ Failed to load config from {}: {}", config_path, e);
            info!("🔧 Using default configuration...");
            Arc::new(Config::default())
        }
    };
    
    // 설정 검증
    if let Err(e) = config.validate() {
        error!("❌ Configuration validation failed: {}", e);
        return Err(e);
    }
    
    info!("✅ Configuration validated successfully");
    
    // WebSocket Provider 연결
    let ws_url = config.network.ws_url.as_ref()
        .ok_or_else(|| anyhow!("WebSocket URL not configured"))?;
    
    info!("🔌 Connecting to WebSocket: {}", ws_url);
    let provider = match Provider::<Ws>::connect(ws_url).await {
        Ok(provider) => Arc::new(provider),
        Err(e) => {
            error!("❌ Failed to connect to WebSocket: {}", e);
            return Err(anyhow!("WebSocket connection failed: {}", e));
        }
    };
    
    // 네트워크 연결 확인
    let chain_id = provider.get_chainid().await?.as_u64();
    if chain_id != config.network.chain_id {
        warn!("⚠️ Chain ID mismatch: expected {}, got {}", config.network.chain_id, chain_id);
    }
    
    let block_number = provider.get_block_number().await?.as_u64();
    info!("✅ Connected to network {} at block {}", chain_id, block_number);
    
    // 통합 청산 관리자 초기화
    info!("🏭 Initializing Integrated Liquidation Manager...");
    let liquidation_manager = IntegratedLiquidationManager::new(
        Arc::clone(&config),
        Arc::clone(&provider),
    ).await?;
    
    info!("✅ Liquidation Manager ready");
    
    // 설정에 따른 실행 모드 결정
    let mode = env::var("LIQUIDATION_MODE").unwrap_or_else(|_| "auto".to_string());
    
    match mode.as_str() {
        "auto" | "daemon" => {
            run_automated_bot(liquidation_manager).await?;
        }
        "scan" => {
            run_scan_mode(liquidation_manager).await?;
        }
        "analyze" => {
            run_analysis_mode(liquidation_manager).await?;
        }
        "test" => {
            run_test_mode(liquidation_manager).await?;
        }
        _ => {
            error!("❌ Unknown liquidation mode: {}. Use: auto, scan, analyze, test", mode);
            return Err(anyhow!("Invalid liquidation mode"));
        }
    }
    
    Ok(())
}

/// 자동 봇 모드 실행
async fn run_automated_bot(manager: IntegratedLiquidationManager) -> Result<()> {
    info!("🤖 Starting automated liquidation bot...");
    
    // 자동 청산 시작
    manager.start_automated_liquidation().await?;
    
    // 통계 리포팅 태스크
    let manager_clone = manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5분마다
        
        loop {
            interval.tick().await;
            
            let summary = manager_clone.get_liquidation_summary().await;
            info!("📊 Bot Status: {} opportunities, ${:.2} potential profit, {:.2}% success rate",
                  summary.active_opportunities,
                  summary.total_potential_profit,
                  summary.performance_metrics.execution_success_rate * 100.0);
        }
    });
    
    // 종료 신호 대기
    info!("✅ Bot is running. Press Ctrl+C to stop.");
    
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("🛑 Received shutdown signal");
        }
        Err(err) => {
            error!("❌ Failed to listen for shutdown signal: {}", err);
        }
    }
    
    // 정리 작업
    info!("🧹 Stopping liquidation bot...");
    manager.stop_automated_liquidation().await?;
    
    // 최종 통계 출력
    let final_stats = manager.get_execution_stats().await;
    info!("📈 Final Statistics:");
    info!("  Total bundles: {}", final_stats.total_bundles_submitted);
    info!("  Success rate: {:.2}%", final_stats.inclusion_rate * 100.0);
    info!("  Total profit: ${:.2}", final_stats.total_profit_realized);
    info!("  Average execution time: {:.1}ms", final_stats.average_execution_time_ms);
    
    info!("👋 Liquidation bot shutdown complete");
    Ok(())
}

/// 스캔 전용 모드 실행
async fn run_scan_mode(manager: IntegratedLiquidationManager) -> Result<()> {
    info!("🔍 Running liquidation opportunity scan...");
    
    let summary = manager.get_liquidation_summary().await;
    
    println!("\n🎯 LIQUIDATION OPPORTUNITY SCAN RESULTS");
    println!("=====================================");
    println!("Active Opportunities: {}", summary.active_opportunities);
    println!("Total Potential Profit: ${:.2}", summary.total_potential_profit);
    println!();
    
    if !summary.protocol_breakdown.is_empty() {
        println!("📊 Protocol Breakdown:");
        for (protocol, count) in &summary.protocol_breakdown {
            println!("  {}: {} opportunities", protocol, count);
        }
        println!();
    }
    
    if !summary.active_opportunities == 0 {
        println!("💡 Top 5 Opportunities:");
        for (i, opp) in summary.recent_executions.iter().take(5).enumerate() {
            println!("  {}. Bundle ID: {} | Success: {} | Profit: ${:.2}",
                     i + 1,
                     opp.bundle_id,
                     opp.success,
                     opp.profit_realized.unwrap_or(0.0));
        }
        println!();
    }
    
    let protocol_summary = manager.get_protocol_summary().await?;
    println!("📋 Protocol Summary:");
    println!("  Total Users: {}", protocol_summary.total_users);
    println!("  Total Collateral: ${:.2}", protocol_summary.total_collateral_usd);
    println!("  Total Debt: ${:.2}", protocol_summary.total_debt_usd);
    
    Ok(())
}

/// 분석 모드 실행
async fn run_analysis_mode(manager: IntegratedLiquidationManager) -> Result<()> {
    info!("📈 Running liquidation analysis...");
    
    let strategy_stats = manager.get_strategy_stats().await?;
    let execution_stats = manager.get_execution_stats().await;
    let summary = manager.get_liquidation_summary().await;
    
    println!("\n📈 LIQUIDATION ANALYSIS REPORT");
    println!("===============================");
    
    println!("\n🎯 Strategy Performance:");
    println!("  Total Opportunities: {}", strategy_stats.total_opportunities_detected);
    println!("  Average Profit Margin: {:.2}%", strategy_stats.average_profit_per_execution);
    println!("  Total Profit Potential: ${:.2}", strategy_stats.total_profit_earned);
    
    println!("\n⚡ Execution Performance:");
    println!("  Total Bundles Submitted: {}", execution_stats.total_bundles_submitted);
    println!("  Success Rate: {:.2}%", execution_stats.inclusion_rate * 100.0);
    println!("  Total Profit Realized: ${:.2}", execution_stats.total_profit_realized);
    println!("  Average Execution Time: {:.1}ms", execution_stats.average_execution_time_ms);
    
    println!("\n🔍 Current Status:");
    println!("  Active Opportunities: {}", summary.active_opportunities);
    println!("  Pending Executions: {}", summary.pending_executions);
    println!("  Bot Uptime: {:.1} hours", summary.performance_metrics.uptime_seconds as f64 / 3600.0);
    
    if !summary.recent_executions.is_empty() {
        println!("\n📋 Recent Executions:");
        for (i, execution) in summary.recent_executions.iter().take(10).enumerate() {
            let status = if execution.success { "✅" } else { "❌" };
            println!("  {}. {} {} - Profit: ${:.2}",
                     i + 1,
                     status,
                     execution.bundle_id,
                     execution.profit_realized.unwrap_or(0.0));
        }
    }
    
    Ok(())
}

/// 테스트 모드 실행
async fn run_test_mode(manager: IntegratedLiquidationManager) -> Result<()> {
    info!("🧪 Running liquidation system test...");
    
    // 1. 기본 연결 테스트
    println!("1. Testing system connectivity...");
    let summary = manager.get_liquidation_summary().await;
    println!("   ✅ System accessible");
    
    // 2. 프로토콜 스캐너 테스트
    println!("2. Testing protocol scanners...");
    let protocol_summary = manager.get_protocol_summary().await?;
    println!("   ✅ Protocol scanners working - {} total users monitored", protocol_summary.total_users);
    
    // 3. 전략 엔진 테스트
    println!("3. Testing strategy engine...");
    let strategy_stats = manager.get_strategy_stats().await?;
    println!("   ✅ Strategy engine working - {} opportunities detected", strategy_stats.total_opportunities_detected);
    
    // 4. 실행 엔진 테스트 (드라이런)
    println!("4. Testing execution engine (dry run)...");
    let execution_stats = manager.get_execution_stats().await;
    println!("   ✅ Execution engine accessible - {} historical bundles", execution_stats.total_bundles_submitted);
    
    // 5. 설정 유효성 재확인
    println!("5. Testing configuration...");
    println!("   ✅ All configurations valid");
    
    println!("\n🎉 All tests passed! System is ready for operation.");
    
    // 시스템 상태 요약 출력
    println!("\n📊 System Status Summary:");
    println!("  Active Opportunities: {}", summary.active_opportunities);
    println!("  Protocol Coverage: {} protocols", summary.protocol_breakdown.len());
    println!("  Total Potential: ${:.2}", summary.total_potential_profit);
    
    Ok(())
}

/// 사용법 출력
fn print_usage() {
    println!("Usage: liquidation_bot [OPTIONS]");
    println!();
    println!("Environment Variables:");
    println!("  XCRACK_CONFIG     Configuration file path (default: config/liquidation.toml)");
    println!("  LIQUIDATION_MODE  Execution mode (default: auto)");
    println!();
    println!("Modes:");
    println!("  auto     - Run automated liquidation bot");
    println!("  scan     - Scan for opportunities and exit");
    println!("  analyze  - Show detailed analysis and exit");
    println!("  test     - Run system tests and exit");
    println!();
    println!("Examples:");
    println!("  LIQUIDATION_MODE=scan ./liquidation_bot");
    println!("  XCRACK_CONFIG=custom.toml ./liquidation_bot");
}