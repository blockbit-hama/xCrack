use super::types;
use super::stats;
use super::dex_router;
use super::mempool_monitor;
use super::strategy_manager;
use super::bundle_builder;
use super::executor;
use anyhow::{Result, anyhow};
use ethers::prelude::*;
use ethers::types::Address;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{info, debug, error};

/// 통합 샌드위치 매니저 - 최상위 오케스트레이터
pub struct IntegratedSandwichManager {
    config: Arc<crate::config::Config>,
    provider: Arc<Provider<Ws>>,
    wallet: LocalWallet,

    // 핵심 컴포넌트
    dex_manager: Arc<dex_router::DexRouterManager>,
    mempool_monitor: Arc<RwLock<Option<mempool_monitor::MempoolMonitor>>>,
    strategy_manager: Arc<RwLock<Option<strategy_manager::SandwichStrategyManager>>>,
    bundle_builder: Arc<bundle_builder::SandwichBundleBuilder>,
    executor: Arc<RwLock<Option<executor::SandwichExecutor>>>,
    stats: Arc<stats::SandwichStatsManager>,

    // 제어
    is_running: Arc<RwLock<bool>>,
}

impl IntegratedSandwichManager {
    pub async fn new(
        config: Arc<crate::config::Config>,
        provider: Arc<Provider<Ws>>,
        wallet: LocalWallet,
        contract_address: Address,
    ) -> Result<Self> {
        info!("🥪 ========================================");
        info!("🥪   통합 샌드위치 매니저 초기화 중...");
        info!("🥪 ========================================");

        // DEX 관리자
        let dex_manager = Arc::new(dex_router::DexRouterManager::new()?);

        // 번들 빌더
        let chain_id = provider.get_chainid().await?.as_u64();
        let bundle_builder = Arc::new(bundle_builder::SandwichBundleBuilder::new(
            contract_address,
            chain_id,
        ));

        // 통계
        let stats = Arc::new(stats::SandwichStatsManager::new());

        let manager = Self {
            config,
            provider,
            wallet,
            dex_manager,
            mempool_monitor: Arc::new(RwLock::new(None)),
            strategy_manager: Arc::new(RwLock::new(None)),
            bundle_builder,
            executor: Arc::new(RwLock::new(None)),
            stats,
            is_running: Arc::new(RwLock::new(false)),
        };

        info!("✅ 통합 샌드위치 매니저 초기화 완료");
        Ok(manager)
    }

    /// 샌드위치 전략 시작
    pub async fn start(&self) -> Result<()> {
        if *self.is_running.read().await {
            return Err(anyhow!("샌드위치 전략이 이미 실행 중입니다"));
        }

        info!("🚀 샌드위치 전략 시작...");
        *self.is_running.write().await = true;

        // 환경변수에서 설정값 로드
        let min_value_eth = std::env::var("SANDWICH_MIN_VALUE_ETH")
            .unwrap_or_else(|_| "0.1".to_string())
            .parse::<f64>()
            .unwrap_or(0.1);
        
        let max_gas_price_gwei = std::env::var("SANDWICH_MAX_GAS_PRICE_GWEI")
            .unwrap_or_else(|_| "200".to_string())
            .parse::<u64>()
            .unwrap_or(200);
        
        let min_profit_eth = std::env::var("SANDWICH_MIN_PROFIT_ETH")
            .unwrap_or_else(|_| "0.01".to_string())
            .parse::<f64>()
            .unwrap_or(0.01);
        
        let min_profit_percentage = std::env::var("SANDWICH_MIN_PROFIT_PERCENTAGE")
            .unwrap_or_else(|_| "0.02".to_string())
            .parse::<f64>()
            .unwrap_or(0.02);
        
        let max_price_impact = std::env::var("SANDWICH_MAX_PRICE_IMPACT")
            .unwrap_or_else(|_| "0.05".to_string())
            .parse::<f64>()
            .unwrap_or(0.05);
        
        let kelly_risk_factor = std::env::var("SANDWICH_KELLY_RISK_FACTOR")
            .unwrap_or_else(|_| "0.5".to_string())
            .parse::<f64>()
            .unwrap_or(0.5);
        
        let contract_address_str = std::env::var("SANDWICH_CONTRACT_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
        
        let contract_address = contract_address_str.parse::<Address>()
            .map_err(|e| anyhow!("Invalid contract address: {}", e))?;
        
        let flashbots_relay_url = std::env::var("SANDWICH_FLASHBOTS_RELAY_URL")
            .unwrap_or_else(|_| "https://relay.flashbots.net".to_string());

        info!("📋 샌드위치 설정:");
        info!("   최소 거래 가치: {} ETH", min_value_eth);
        info!("   최대 가스 가격: {} Gwei", max_gas_price_gwei);
        info!("   최소 수익: {} ETH ({:.1}%)", min_profit_eth, min_profit_percentage * 100.0);
        info!("   최대 가격 영향: {:.1}%", max_price_impact * 100.0);
        info!("   Kelly 위험 계수: {}", kelly_risk_factor);
        info!("   컨트랙트 주소: {:?}", contract_address);
        info!("   Flashbots Relay: {}", flashbots_relay_url);

        // 1. 멤풀 모니터 시작
        let (mempool_monitor, mempool_rx) = mempool_monitor::MempoolMonitor::new(
            self.provider.clone(),
            self.dex_manager.clone(),
            min_value_eth,
            max_gas_price_gwei,
        ).await?;

        mempool_monitor.start().await?;
        *self.mempool_monitor.write().await = Some(mempool_monitor);

        // 2. 전략 매니저 시작
        let (strategy_manager, opportunity_rx) = strategy_manager::SandwichStrategyManager::new(
            self.provider.clone(),
            min_profit_eth,
            min_profit_percentage,
            max_price_impact,
            kelly_risk_factor,
        ).await?;

        strategy_manager.start(mempool_rx).await?;
        *self.strategy_manager.write().await = Some(strategy_manager);

        // 3. 실행자 초기화
        let executor = executor::SandwichExecutor::new(
            self.provider.clone(),
            self.wallet.clone(),
            contract_address,
            flashbots_relay_url,
            self.stats.clone(),
        );
        *self.executor.write().await = Some(executor);

        // 4. 실행 루프 시작
        self.start_execution_loop(opportunity_rx).await?;

        // 5. 통계 출력 루프
        self.start_stats_loop().await;

        info!("✅ 샌드위치 전략 시작 완료");
        Ok(())
    }

    /// 실행 루프 시작
    async fn start_execution_loop(
        &self,
        mut opportunity_rx: mpsc::UnboundedReceiver<types::SandwichOpportunity>,
    ) -> Result<()> {
        let bundle_builder = self.bundle_builder.clone();
        let executor = self.executor.clone();
        let provider = self.provider.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            info!("🔁 실행 루프 시작");

            while *is_running.read().await {
                if let Some(opportunity) = opportunity_rx.recv().await {
                    info!("💰 샌드위치 기회 수신");

                    // 현재 블록 번호
                    let block_number = match provider.get_block_number().await {
                        Ok(num) => num.as_u64(),
                        Err(e) => {
                            error!("❌ 블록 번호 조회 실패: {}", e);
                            continue;
                        }
                    };

                    // 번들 생성
                    let bundle = match bundle_builder.build_bundle(&opportunity, block_number).await {
                        Ok(bundle) => bundle,
                        Err(e) => {
                            error!("❌ 번들 생성 실패: {}", e);
                            continue;
                        }
                    };

                    // 실행
                    if let Some(exec) = executor.read().await.as_ref() {
                        match exec.execute_bundle(bundle).await {
                            Ok(result) => {
                                if result.success {
                                    info!("🎉 샌드위치 실행 성공!");
                                } else {
                                    debug!("⚠️ 샌드위치 실행 실패: {:?}", result.error_message);
                                }
                            }
                            Err(e) => {
                                error!("❌ 샌드위치 실행 오류: {}", e);
                            }
                        }
                    }
                }
            }

            info!("🛑 실행 루프 종료");
        });

        Ok(())
    }

    /// 통계 출력 루프
    async fn start_stats_loop(&self) {
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut stats_interval = interval(Duration::from_secs(300)); // 5분마다

            while *is_running.read().await {
                stats_interval.tick().await;
                stats.print_stats().await;
            }
        });
    }

    /// 샌드위치 전략 중지
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 샌드위치 전략 중지 중...");
        *self.is_running.write().await = false;

        // 멤풀 모니터 중지
        if let Some(monitor) = self.mempool_monitor.read().await.as_ref() {
            monitor.stop();
        }

        // 전략 매니저 중지
        if let Some(manager) = self.strategy_manager.read().await.as_ref() {
            manager.stop().await;
        }

        // 최종 통계 출력
        self.stats.print_stats().await;

        info!("✅ 샌드위치 전략 중지 완료");
        Ok(())
    }

    /// 현재 통계 조회
    pub async fn get_stats(&self) -> types::SandwichStats {
        self.stats.get_stats().await
    }

    /// 통계 출력
    pub async fn print_stats(&self) {
        self.stats.print_stats().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        // Mock test
        assert!(true);
    }
}
