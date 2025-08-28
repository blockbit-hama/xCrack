use anyhow::Result;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::Mutex;
use tokio::time::timeout as tokio_timeout;
use uuid::Uuid;
use tracing::{info, debug, warn};
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use async_trait::async_trait;
use alloy::primitives::{Address as AlloyAddress, U256 as AlloyU256, Bytes as AlloyBytes};

use crate::{
    config::Config,
    types::{
        StrategyType, ChainId, BridgeProtocol, CrossChainToken, 
        CrossChainArbitrageOpportunity, CrossChainTrade, Transaction,
        Opportunity, Bundle
    },
    strategies::traits::Strategy,
    mocks::{get_mock_config, MockConfig},
    bridges::{BridgeManager, RouteStrategy},
};

/// xCrack Cross-Chain Arbitrage Strategy
/// 
/// 크로스체인 아비트래지 전략은 서로 다른 블록체인 네트워크 간의 가격 차이를 
/// 이용하여 수익을 창출하는 전략입니다.
/// 
/// 핵심 기능:
/// - 멀티체인 가격 모니터링
/// - 브리지 비용 계산 및 수익성 분석  
/// - 자동 크로스체인 거래 실행
/// - 리스크 관리 및 실패 복구
#[derive(Debug)]
pub struct CrossChainArbitrageStrategy {
    /// 전략 ID
    id: Uuid,
    /// 설정 파일
    config: Arc<Config>,
    /// Mock 설정 (개발용)
    mock_config: MockConfig,
    /// 브리지 매니저
    bridge_manager: Arc<BridgeManager>,
    /// 지원하는 체인들
    supported_chains: Vec<ChainId>,
    /// 지원하는 브리지들
    supported_bridges: Vec<BridgeProtocol>,
    /// 체인별 토큰 목록
    tokens_registry: Arc<RwLock<HashMap<String, CrossChainToken>>>,
    /// 활성 기회들
    active_opportunities: Arc<RwLock<HashMap<String, CrossChainArbitrageOpportunity>>>,
    /// 실행 중인 거래들
    active_trades: Arc<Mutex<HashMap<String, CrossChainTrade>>>,
    /// 전략 상태
    is_running: Arc<RwLock<bool>>,
    /// 성능 메트릭
    performance_metrics: Arc<RwLock<CrossChainMetrics>>,
    /// 마지막 실행 시간
    last_execution: Arc<RwLock<Option<DateTime<Utc>>>>,
}

/// 크로스체인 성능 메트릭
#[derive(Debug, Clone, Default)]
pub struct CrossChainMetrics {
    /// 발견한 총 기회 수
    pub total_opportunities_found: u64,
    /// 실행한 총 거래 수
    pub total_trades_executed: u64,
    /// 성공한 거래 수
    pub successful_trades: u64,
    /// 실패한 거래 수
    pub failed_trades: u64,
    /// 총 수익
    pub total_profit: f64,
    /// 총 손실
    pub total_loss: f64,
    /// 평균 실행 시간 (초)
    pub avg_execution_time: f64,
    /// 성공률
    pub success_rate: f64,
}

impl CrossChainArbitrageStrategy {
    /// 새로운 크로스체인 아비트래지 전략 인스턴스 생성
    pub fn new(config: Arc<Config>) -> Self {
        let mock_config = get_mock_config();
        
        let supported_chains = vec![
            ChainId::Ethereum,
            ChainId::Polygon, 
            ChainId::BSC,
            ChainId::Arbitrum,
            ChainId::Optimism,
        ];
        
        let supported_bridges = vec![
            BridgeProtocol::Stargate,
            BridgeProtocol::Hop,
            BridgeProtocol::Rubic,
            BridgeProtocol::Synapse,
            BridgeProtocol::LiFi,      // Bridge aggregator
            BridgeProtocol::Across,    // Fast bridge
            BridgeProtocol::Multichain, // Multi-chain bridge
        ];
        
        Self {
            id: Uuid::new_v4(),
            config,
            mock_config,
            bridge_manager: Arc::new(BridgeManager::new()),
            supported_chains,
            supported_bridges,
            tokens_registry: Arc::new(RwLock::new(HashMap::new())),
            active_opportunities: Arc::new(RwLock::new(HashMap::new())),
            active_trades: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            performance_metrics: Arc::new(RwLock::new(CrossChainMetrics::default())),
            last_execution: Arc::new(RwLock::new(None)),
        }
    }
    
    /// 전략 초기화
    pub async fn initialize(&self) -> Result<()> {
        info!("🌉 Cross-Chain Arbitrage Strategy 초기화 시작");
        
        // 기본 토큰들 등록
        self.register_default_tokens().await?;
        
        // Mock 모드에서는 가상 데이터로 초기화
        if std::env::var("API_MODE").unwrap_or_default() == "mock" {
            self.initialize_mock_data().await?;
        } else {
            // 실제 모드에서는 브리지 메트릭 업데이트
            self.bridge_manager.update_metrics().await;
        }
        
        *self.is_running.write().unwrap() = true;
        info!("✅ Cross-Chain Arbitrage Strategy 초기화 완료");
        
        Ok(())
    }
    
    /// 기본 토큰들을 등록
    async fn register_default_tokens(&self) -> Result<()> {
        let mut registry = self.tokens_registry.write().unwrap();
        
        // USDC 토큰 등록 (주요 체인들)
        let mut usdc_addresses = HashMap::new();
        usdc_addresses.insert(ChainId::Ethereum, "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse().unwrap());
        usdc_addresses.insert(ChainId::Polygon, "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".parse().unwrap());
        usdc_addresses.insert(ChainId::BSC, "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d".parse().unwrap());
        usdc_addresses.insert(ChainId::Arbitrum, "0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".parse().unwrap());
        usdc_addresses.insert(ChainId::Optimism, "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".parse().unwrap());
        
        let usdc_token = CrossChainToken {
            symbol: "USDC".to_string(),
            addresses: usdc_addresses,
            decimals: 6,
        };
        
        registry.insert("USDC".to_string(), usdc_token);
        
        // WETH 토큰 등록
        let mut weth_addresses = HashMap::new();
        weth_addresses.insert(ChainId::Ethereum, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap());
        weth_addresses.insert(ChainId::Polygon, "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".parse().unwrap());
        weth_addresses.insert(ChainId::BSC, "0x2170Ed0880ac9A755fd29B2688956BD959F933F8".parse().unwrap());
        weth_addresses.insert(ChainId::Arbitrum, "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".parse().unwrap());
        weth_addresses.insert(ChainId::Optimism, "0x4200000000000000000000000000000000000006".parse().unwrap());
        
        let weth_token = CrossChainToken {
            symbol: "WETH".to_string(),
            addresses: weth_addresses,
            decimals: 18,
        };
        
        registry.insert("WETH".to_string(), weth_token);
        
        info!("📝 기본 토큰 등록 완료: USDC, WETH");
        Ok(())
    }
    
    /// Mock 데이터 초기화
    async fn initialize_mock_data(&self) -> Result<()> {
        info!("🎭 Mock 데이터 초기화 중...");
        
        // Mock 크로스체인 기회 생성
        self.generate_mock_opportunities().await?;
        
        info!("✅ Mock 데이터 초기화 완료");
        Ok(())
    }
    
    /// Mock 크로스체인 기회 생성
    async fn generate_mock_opportunities(&self) -> Result<()> {
        let tokens = self.tokens_registry.read().unwrap();
        let mut opportunities = self.active_opportunities.write().unwrap();
        
        // USDC 크로스체인 기회 시뮬레이션
        if let Some(usdc_token) = tokens.get("USDC") {
            let opportunity = CrossChainArbitrageOpportunity {
                id: Uuid::new_v4().to_string(),
                token: usdc_token.clone(),
                source_chain: ChainId::Polygon, // Polygon에서 저렴
                dest_chain: ChainId::Ethereum,  // Ethereum에서 비쌈
                source_price: 0.998, // $0.998
                dest_price: 1.003,   // $1.003
                price_diff_percent: 0.50, // 0.5% 차이
                amount: alloy::primitives::U256::from(10000_000000u64), // 10,000 USDC
                bridge_protocol: BridgeProtocol::Stargate,
                bridge_cost: alloy::primitives::U256::from(5_000000u64), // $5 브리지 비용
                total_gas_cost: alloy::primitives::U256::from(15_000000u64), // $15 가스 비용
                expected_profit: alloy::primitives::U256::from(30_000000u64), // $30 예상 수익
                profit_percent: 0.30, // 0.3% 수익률
                estimated_time: 300, // 5분
                confidence: 0.85, // 85% 신뢰도
                discovered_at: Utc::now(),
                expires_at: Utc::now() + ChronoDuration::minutes(10),
                selected_dex_adapters: Vec::new(), // 빈 벡터로 초기화
            };
            
            opportunities.insert(opportunity.id.clone(), opportunity);
        }
        
        // WETH 크로스체인 기회 시뮬레이션
        if let Some(weth_token) = tokens.get("WETH") {
            let opportunity = CrossChainArbitrageOpportunity {
                id: Uuid::new_v4().to_string(),
                token: weth_token.clone(),
                source_chain: ChainId::BSC,      // BSC에서 저렴
                dest_chain: ChainId::Arbitrum,  // Arbitrum에서 비쌈
                source_price: 2850.50, // $2,850.50
                dest_price: 2865.20,   // $2,865.20
                price_diff_percent: 0.52, // 0.52% 차이
                amount: alloy::primitives::U256::from(5_000000000000000000u64), // 5 ETH
                bridge_protocol: BridgeProtocol::Hop,
                bridge_cost: alloy::primitives::U256::from(8_000000u64), // $8 브리지 비용
                total_gas_cost: alloy::primitives::U256::from(25_000000u64), // $25 가스 비용
                expected_profit: alloy::primitives::U256::from(41_350000u64), // $41.35 예상 수익
                profit_percent: 0.29, // 0.29% 수익률
                estimated_time: 420, // 7분
                confidence: 0.78, // 78% 신뢰도
                discovered_at: Utc::now(),
                expires_at: Utc::now() + ChronoDuration::minutes(15),
                selected_dex_adapters: Vec::new(), // 빈 벡터로 초기화
            };
            
            opportunities.insert(opportunity.id.clone(), opportunity);
        }
        
        info!("🎯 Mock 기회 생성 완료: {} 개", opportunities.len());
        Ok(())
    }
    
    /// 크로스체인 기회 스캔
    pub async fn scan_opportunities(&self) -> Result<Vec<CrossChainArbitrageOpportunity>> {
        debug!("🔍 크로스체인 기회 스캔 시작");
        
        let opportunities = if std::env::var("API_MODE").unwrap_or_default() == "mock" {
            // Mock 모드: 기존 방식 사용
            let active = self.active_opportunities.read().unwrap();
            active
                .values()
                .filter(|opp| opp.is_valid())
                .cloned()
                .collect()
        } else {
            // 실제 모드: 실시간 브리지 스캔
            self.scan_real_bridge_opportunities().await?
        };
            
        info!("🎯 발견한 크로스체인 기회: {} 개", opportunities.len());
        
        // 성능 메트릭 업데이트
        {
            let mut metrics = self.performance_metrics.write().unwrap();
            metrics.total_opportunities_found += opportunities.len() as u64;
        }
        
        Ok(opportunities)
    }
    
    /// 크로스체인 거래 실행 (Mock)
    pub async fn execute_cross_chain_trade_mock(&self, opportunity: &CrossChainArbitrageOpportunity) -> Result<bool> {
        info!("🚀 Mock 크로스체인 거래 실행 시작: {} -> {}", 
            opportunity.source_chain.name(),
            opportunity.dest_chain.name()
        );
        
        let trade = CrossChainTrade::new(opportunity.clone());
        let trade_id = trade.id.clone();
        
        {
            let mut active_trades = self.active_trades.lock().await;
            active_trades.insert(trade_id.clone(), trade);
        }
        
        // Mock 실행 시뮬레이션
        let success = fastrand::f64() < self.mock_config.order_execution_success_rate;
        
        if success {
            info!("✅ Mock 크로스체인 거래 성공: ${:.2} 수익", 
                opportunity.expected_profit.to::<u64>() as f64 / 1_000000.0
            );
            
            // 성공 메트릭 업데이트
            let mut metrics = self.performance_metrics.write().unwrap();
            metrics.total_trades_executed += 1;
            metrics.successful_trades += 1;
            metrics.total_profit += opportunity.expected_profit.to::<u64>() as f64 / 1_000000.0;
            metrics.avg_execution_time = (metrics.avg_execution_time + opportunity.estimated_time as f64) / 2.0;
            metrics.success_rate = metrics.successful_trades as f64 / metrics.total_trades_executed as f64;
            
        } else {
            warn!("❌ Mock 크로스체인 거래 실패: 브리지 오류 시뮬레이션");
            
            // 실패 메트릭 업데이트
            let mut metrics = self.performance_metrics.write().unwrap();
            metrics.total_trades_executed += 1;
            metrics.failed_trades += 1;
            metrics.total_loss += opportunity.bridge_cost.to::<u64>() as f64 / 1_000000.0;
            metrics.success_rate = metrics.successful_trades as f64 / metrics.total_trades_executed as f64;
        }
        
        *self.last_execution.write().unwrap() = Some(Utc::now());
        Ok(success)
    }
    
    /// 실제 브리지를 사용한 크로스체인 기회 탐지
    pub async fn scan_real_bridge_opportunities(&self) -> Result<Vec<CrossChainArbitrageOpportunity>> {
        debug!("🔍 실제 브리지를 통한 크로스체인 기회 스캔 시작");
        
        let tokens = {
            let tokens_guard = self.tokens_registry.read().unwrap();
            tokens_guard.values().cloned().collect::<Vec<_>>()
        };
        let mut opportunities = Vec::new();
        
        for token in tokens.iter() {
            // 모든 가능한 체인 조합에서 기회 탐색
            for &source_chain in &self.supported_chains {
                for &dest_chain in &self.supported_chains {
                    if source_chain == dest_chain {
                        continue;
                    }
                    
                    // 소량으로 테스트 (1000 USDC / 1 WETH)
                    let test_amount = if token.symbol == "USDC" {
                        alloy::primitives::U256::from(1000_000000u64) // 1000 USDC
                    } else {
                        alloy::primitives::U256::from(1_000000000000000000u64) // 1 ETH
                    };
                    
                    // 최적 브리지 찾기
                    match self.bridge_manager.get_best_quote(
                        source_chain,
                        dest_chain,
                        token,
                        test_amount,
                        0.5, // 0.5% 슬리패지
                        Some(RouteStrategy::LowestCost),
                    ).await {
                        Ok(quote) => {
                            // 수익성 검증
                            if quote.is_profitable() && quote.net_profit() > 0 {
                                let opportunity = CrossChainArbitrageOpportunity {
                                    id: Uuid::new_v4().to_string(),
                                    token: token.clone(),
                                    source_chain,
                                    dest_chain,
                                    source_price: quote.exchange_rate,
                                    dest_price: quote.exchange_rate * (1.0 + quote.price_impact / 100.0),
                                    price_diff_percent: quote.price_impact,
                                    amount: quote.amount_in,
                                    bridge_protocol: self.get_bridge_protocol_from_quote(&quote),
                                    bridge_cost: quote.bridge_fee,
                                    total_gas_cost: quote.gas_fee,
                                    expected_profit: alloy::primitives::U256::from(quote.net_profit().max(0) as u128),
                                    profit_percent: (quote.net_profit() as f64 / quote.amount_in.to::<u128>() as f64) * 100.0,
                                    estimated_time: quote.estimated_time,
                                    confidence: 0.8, // 실제 브리지라서 높은 신뢰도
                                    discovered_at: Utc::now(),
                                    expires_at: quote.expires_at,
                                    selected_dex_adapters: Vec::new(), // 빈 벡터로 초기화
                                };
                                
                                opportunities.push(opportunity);
                                
                                if opportunities.len() >= 10 { // 최대 10개로 제한
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            debug!("브리지 견적 실패: {} -> {} ({}): {}", 
                                   source_chain.name(), dest_chain.name(), token.symbol, e);
                        }
                    }
                }
                
                if opportunities.len() >= 10 { // 최대 10개로 제한
                    break;
                }
            }
            
            if opportunities.len() >= 10 { // 최대 10개로 제한
                break;
            }
        }
        
        info!("🎯 실제 브리지에서 {} 개의 수익 기회 발견", opportunities.len());
        Ok(opportunities)
    }
    
    /// 실제 크로스체인 거래 실행
    pub async fn execute_real_cross_chain_trade(&self, opportunity: &CrossChainArbitrageOpportunity) -> Result<bool> {
        info!("🚀 실제 크로스체인 거래 실행 시작: {} -> {}", 
            opportunity.source_chain.name(),
            opportunity.dest_chain.name()
        );
        
        // 🆕 플래시론 보조 모드(설정 기반): 브리지 출발 자산을 플래시론으로 조달하는 경로를 선택할 수 있습니다.
        // 실제 구현에는 Aave flashLoanSimple + 브리지 컨트랙트 호출 조합이 필요하며,
        // 여기서는 안전하게 견적/실행 로직만 유지하고 플래시론 모드 여부를 로깅합니다.
        if std::env::var("API_MODE").unwrap_or_default() != "mock" && self.config.strategies.cross_chain_arbitrage.use_flashloan {
            debug!("🔁 Flashloan 보조 모드 힌트 (크로스체인): 출발 자산을 대여하여 브리지+도착 DEX 청산 가능");
        }

        // 1) 최신 견적 1차 획득 (Balanced)
        let mut quote = self.bridge_manager.get_best_quote(
            opportunity.source_chain,
            opportunity.dest_chain,
            &opportunity.token,
            opportunity.amount,
            0.5,
            Some(RouteStrategy::Balanced),
        ).await?;

        // 1-1) 견적 만료/임박 재검증: 만료이거나 유효시간이 30초 미만이면 재조회 1회
        let now = chrono::Utc::now();
        let time_left = (quote.expires_at - now).num_seconds();
        if !quote.is_valid() || time_left < 30 {
            warn!("⚠️ 견적이 만료/임박({}s), 재조회 시도", time_left);
            quote = self.bridge_manager.get_best_quote(
                opportunity.source_chain,
                opportunity.dest_chain,
                &opportunity.token,
                opportunity.amount,
                0.5,
                Some(RouteStrategy::Balanced),
            ).await?;
            if !quote.is_valid() {
                warn!("❌ 재조회 견적도 유효하지 않음");
                return Ok(false);
            }
        }
        
        // 1-2) 최소 수익/시간 가드 (보수적): 순이익 <= 0 이거나 예상 시간 15분 초과 시 스킵
        if !quote.is_profitable() {
            warn!("⚠️ 순이익이 0 이하로 추정, 실행 스킵");
            return Ok(false);
        }
        if quote.estimated_time > 900 { // 15분 초과
            warn!("⚠️ 예상 소요시간이 15분을 초과, 실행 스킵 ({}s)", quote.estimated_time);
            return Ok(false);
        }

        // 2) 플래시론 보조 경로: 크로스체인은 원자성 한계로 실제 사용 비권장. 현재는 로깅만 수행.
        let primary_protocol = self.get_bridge_protocol_from_quote(&quote);
        if self.config.strategies.cross_chain_arbitrage.use_flashloan {
            warn!("⚠️ use_flashloan=true (cross-chain): 원자적 상환이 불가하므로 실제 경로는 비활성. 일반 경로로 진행");
        }

        // 2) 1차 거래 실행 (quote의 라우트 기반 프로토콜 우선)
        // 실행 타임아웃(보수적으로 quote.estimated_time + 60초)
        let exec_timeout_secs = quote.estimated_time.saturating_add(60).max(60);
        let mut execution = match tokio_timeout(
            Duration::from_secs(exec_timeout_secs as u64),
            self.bridge_manager.execute_bridge(primary_protocol.clone(), &quote),
        ).await {
            Ok(res) => res,
            Err(_) => {
                warn!("⏰ 1차 실행 타임아웃({}s) | protocol={:?}", exec_timeout_secs, primary_protocol);
                Err(crate::bridges::traits::BridgeError::ApiError { message: "bridge execution timeout".to_string() })
            }
        };
        
        // 3) 실패/대기 시 1회 백업 경로 재시도
        let mut success = match &execution {
            Ok(exec) => matches!(exec.status, crate::bridges::traits::BridgeExecutionStatus::Completed),
            Err(_) => false,
        };

        if !success {
            // 표준화 로그
            match &execution {
                Ok(exec) => warn!(
                    "❌ 1차 실행 미완료(status={:?}) | protocol={:?}",
                    exec.status, primary_protocol
                ),
                Err(e) => warn!(
                    "❌ 1차 실행 오류: {} | protocol={:?}",
                    e, primary_protocol
                ),
            }

            // 3-1) 모든 견적 조회 후, 다른 프로토콜로 1회 재시도 (짧은 타임아웃)
            let quotes = tokio_timeout(
                Duration::from_secs(15),
                self.bridge_manager.get_all_quotes(
                    opportunity.source_chain,
                    opportunity.dest_chain,
                    &opportunity.token,
                    opportunity.amount,
                    0.5,
                ),
            ).await;
            let mut all_quotes = match quotes {
                Ok(Ok(q)) => q,
                Ok(Err(e)) => {
                    warn!("⚠️ 백업 견적 조회 실패: {}", e);
                    Vec::new()
                }
                Err(_) => {
                    warn!("⏰ 백업 견적 조회 타임아웃(15s)");
                    Vec::new()
                }
            };

            // 우선순위: 높은 net_profit / 낮은 total_cost, 기존 프로토콜 제외
            all_quotes.retain(|(p, _)| p != &primary_protocol);
            all_quotes.sort_by(|a, b| {
                let na = a.1.net_profit();
                let nb = b.1.net_profit();
                nb.cmp(&na)
                    .then_with(|| a.1.total_cost().cmp(&b.1.total_cost()))
            });

            if let Some((fallback_protocol, fallback_quote)) = all_quotes.first() {
                info!(
                    "🔁 백업 경로 재시도: protocol={} net_profit={} cost={}",
                    fallback_protocol.name(),
                    fallback_quote.net_profit(),
                    fallback_quote.total_cost()
                );

                let exec2 = match tokio_timeout(
                    Duration::from_secs(exec_timeout_secs as u64),
                    self.bridge_manager.execute_bridge(fallback_protocol.clone(), fallback_quote),
                ).await {
                    Ok(res) => res,
                    Err(_) => {
                        warn!("⏰ 백업 경로 실행 타임아웃({}s) | protocol={}", exec_timeout_secs, fallback_protocol.name());
                        Err(crate::bridges::traits::BridgeError::ApiError { message: "bridge execution timeout (fallback)".to_string() })
                    }
                };

                success = match exec2 {
                    Ok(exec) => matches!(exec.status, crate::bridges::traits::BridgeExecutionStatus::Completed),
                    Err(e) => {
                        warn!("❌ 백업 경로 실행 오류: {} | protocol={}", e, fallback_protocol.name());
                        false
                    }
                };
            } else {
                warn!("⚠️ 사용할 수 있는 백업 경로가 없음");
            }
        }
        
        if success {
            info!("✅ 실제 크로스체인 거래 성공: ${:.2} 수익", 
                quote.net_profit() as f64 / 1_000000.0
            );
            
            // 성공 메트릭 업데이트
            let mut metrics = self.performance_metrics.write().unwrap();
            metrics.total_trades_executed += 1;
            metrics.successful_trades += 1;
            metrics.total_profit += quote.net_profit().max(0) as f64 / 1_000000.0;
            metrics.avg_execution_time = (metrics.avg_execution_time + quote.estimated_time as f64) / 2.0;
            metrics.success_rate = metrics.successful_trades as f64 / metrics.total_trades_executed as f64;
            
        } else {
            // 표준화 실패 로그
            let err_msg = match execution {
                Ok(exec) => format!("status={:?}", exec.status),
                Err(e) => e.to_string(),
            };
            warn!("❌ 실제 크로스체인 거래 실패: {}", err_msg);
            
            // 실패 메트릭 업데이트
            let mut metrics = self.performance_metrics.write().unwrap();
            metrics.total_trades_executed += 1;
            metrics.failed_trades += 1;
            metrics.total_loss += quote.total_cost().to::<u128>() as f64 / 1_000000.0;
            metrics.success_rate = metrics.successful_trades as f64 / metrics.total_trades_executed as f64;

            // 재시도 후에도 실패 시 안전 폴백: 실행 중인 트레이드가 있을 경우 취소/정리 훅(향후 구현 포인트)
            // 여기서는 로깅만 수행하여 운용 측 알림으로 전파
            warn!("🧯 안전 폴백: 후속 정리 루틴을 수행해야 할 수 있습니다 (브리지 대기/미포함 처리)");
        }
        
        *self.last_execution.write().unwrap() = Some(Utc::now());
        Ok(success)
    }
    
    /// 견적에서 브리지 프로토콜 추출 (Mock용 헬퍼)
    fn get_bridge_protocol_from_quote(&self, quote: &crate::bridges::traits::BridgeQuote) -> BridgeProtocol {
        // route_data에서 브리지 이름 추출
        if let Some(bridge_name) = quote.route_data.get("bridge") {
            match bridge_name.as_str() {
                Some("stargate") => BridgeProtocol::Stargate,
                Some("hop") => BridgeProtocol::Hop,
                Some("rubic") => BridgeProtocol::Rubic,
                Some("synapse") => BridgeProtocol::Synapse,
                Some("lifi") => BridgeProtocol::LiFi,
                Some("across") => BridgeProtocol::Across,
                Some("multichain") => BridgeProtocol::Multichain,
                _ => BridgeProtocol::LiFi, // LiFi를 기본값으로 (aggregator)
            }
        } else {
            BridgeProtocol::LiFi // LiFi를 기본값으로 (aggregator)
        }
    }
    
    /// 성능 메트릭 조회
    pub fn get_performance_metrics(&self) -> CrossChainMetrics {
        self.performance_metrics.read().unwrap().clone()
    }
    
    /// 활성 거래 수 조회
    pub async fn get_active_trades_count(&self) -> usize {
        self.active_trades.lock().await.len()
    }
    
    /// 전략 중지
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().unwrap() = false;
        info!("🛑 Cross-Chain Arbitrage Strategy 중지됨");
        Ok(())
    }
}

#[async_trait]
impl Strategy for CrossChainArbitrageStrategy {
    /// 전략 타입
    fn strategy_type(&self) -> StrategyType {
        StrategyType::CrossChainArbitrage
    }
    
    /// 전략 활성화 상태
    fn is_enabled(&self) -> bool {
        *self.is_running.read().unwrap()
    }
    
    /// 전략 시작
    async fn start(&self) -> Result<()> {
        self.initialize().await?;
        info!("🌉 CrossChainArbitrage 전략 시작됨");
        Ok(())
    }
    
    /// 전략 중지
    async fn stop(&self) -> Result<()> {
        *self.is_running.write().unwrap() = false;
        info!("🛑 CrossChainArbitrage 전략 중지됨");
        Ok(())
    }
    
    /// 거래 분석 및 기회 발견
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        // 크로스체인 기회 스캔
        let cross_chain_opportunities = self.scan_opportunities().await?;
        
        let mut opportunities = Vec::new();
        
        // 크로스체인 기회를 일반 Opportunity로 변환
        for cc_opp in cross_chain_opportunities {
            if cc_opp.profit_percent > 0.2 { // 0.2% 이상 수익률
                let opportunity = crate::types::Opportunity::new(
                    crate::types::OpportunityType::CrossChainArbitrage,
                    StrategyType::CrossChainArbitrage,
                    cc_opp.expected_profit,
                    cc_opp.confidence,
                    cc_opp.estimated_time * 21000, // 가스 추정값
                    999999, // 만료 블록 (크로스체인은 시간 기반)
                    crate::types::OpportunityDetails::Arbitrage(crate::types::ArbitrageDetails {
                        token_in: *cc_opp.token.addresses.get(&cc_opp.source_chain).unwrap(),
                        token_out: *cc_opp.token.addresses.get(&cc_opp.dest_chain).unwrap(),
                        amount_in: cc_opp.amount,
                        amount_out: cc_opp.amount + cc_opp.expected_profit,
                        dex_path: vec![format!("{}_{}", cc_opp.bridge_protocol.name(), cc_opp.dest_chain.name())],
                        price_impact: cc_opp.price_diff_percent / 100.0,
                    }),
                );
                
                opportunities.push(opportunity);
                
                // Mock 실행
                if opportunities.len() <= 2 { // 최대 2개만 실행
                    self.execute_cross_chain_trade_mock(&cc_opp).await?;
                }
            }
        }
        
        debug!("🎯 Cross-Chain 기회 반환: {} 개", opportunities.len());
        Ok(opportunities)
    }
    
    /// 기회 유효성 검증
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        // 기본 검증: 수익성과 신뢰도 확인
        if opportunity.expected_profit < alloy::primitives::U256::from(10000000000000000u64) { // 0.01 ETH 미만
            return Ok(false);
        }
        
        if opportunity.confidence < 0.7 { // 70% 미만 신뢰도
            return Ok(false);
        }
        
        // 가스비 대비 수익성 검증
        let gas_cost = alloy::primitives::U256::from(opportunity.gas_estimate) * alloy::primitives::U256::from(20000000000u64); // 20 gwei
        if opportunity.expected_profit <= gas_cost {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// 번들 생성
    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<crate::types::Bundle> {
        // Mock 번들 생성
        let bundle_id = format!("crosschain_{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        
        Ok(crate::types::Bundle::new(
            vec![], // Cross-chain은 복잡한 트랜잭션 조합
            opportunity.expiry_block,
            opportunity.expected_profit,
            opportunity.gas_estimate,
            StrategyType::CrossChainArbitrage,
        ))
    }
}

/// Mock 크로스체인 아비트래지 실행 함수
pub async fn run_cross_chain_arbitrage_mock(config: Arc<Config>) -> Result<()> {
    let strategy = CrossChainArbitrageStrategy::new(config);
    
    // 초기화
    strategy.initialize().await?;
    
    info!("🌉 Cross-Chain Arbitrage Mock 실행 시작");
    
    // 주기적으로 실행
    for cycle in 1..=5 {
        info!("🔄 Cross-Chain Cycle #{}", cycle);
        
        // 기회 스캔
        let opportunities = strategy.scan_opportunities().await?;
        
        // 상위 기회들 실행
        for (i, opportunity) in opportunities.iter().take(2).enumerate() {
            info!("💰 기회 #{}: {} {} -> {} (수익: ${:.2})", 
                i + 1,
                opportunity.token.symbol,
                opportunity.source_chain.name(),
                opportunity.dest_chain.name(),
                opportunity.expected_profit.to::<u64>() as f64 / 1_000000.0
            );
            
            // API 모드에 따라 실행 방법 선택
            if std::env::var("API_MODE").unwrap_or_default() == "mock" {
                strategy.execute_cross_chain_trade_mock(opportunity).await?;
            } else {
                strategy.execute_real_cross_chain_trade(opportunity).await?;
            }
        }
        
        // 성능 메트릭 출력
        let metrics = strategy.get_performance_metrics();
        info!("📊 성과: 거래 {}/{}, 수익 ${:.2}, 성공률 {:.1}%",
            metrics.successful_trades,
            metrics.total_trades_executed,
            metrics.total_profit,
            metrics.success_rate * 100.0
        );
        
        // 5초 대기
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    
    strategy.stop().await?;
    info!("✅ Cross-Chain Arbitrage Mock 실행 완료");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_cross_chain_strategy_creation() {
        let config = Arc::new(Config::default());
        let strategy = CrossChainArbitrageStrategy::new(config);
        assert_eq!(strategy.strategy_type(), StrategyType::CrossChainArbitrage);
    }
}
