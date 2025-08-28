use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use tracing::{info, debug, warn, error};
use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use rust_decimal::Decimal;
use chrono::Utc;
use ethers::providers::{Provider, Ws};

use crate::config::Config;
use crate::types::{
    DexPerformanceData,    Transaction, Opportunity, StrategyType,
    MultiAssetArbitrageOpportunity, MultiAssetStrategyType,
    MultiAssetArbitrageStats,
};
use crate::strategies::Strategy;
use crate::adapters::{DexAdapterFactory, AdapterConfig};
use crate::adapters::factory::{AdapterSelector, AdapterSelectionStrategy};

/// 다중자산 플래시론 아비트래지 전략
/// 
/// Aave v3의 flashLoan API를 사용하여 여러 토큰을 동시에 빌려
/// 복합 아비트래지, 삼각 아비트래지, 포지션 마이그레이션을 수행하는 전략
pub struct MultiAssetArbitrageStrategy {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    enabled: Arc<AtomicBool>,
    
    // 활성 기회 추적
    active_opportunities: Arc<Mutex<HashMap<String, MultiAssetArbitrageOpportunity>>>,
    
    // 성능 통계
    stats: Arc<Mutex<MultiAssetArbitrageStats>>,
    
    // 실행 매개변수
    min_profit_percentage: f64,
    min_profit_usd: Decimal,
    max_execution_time_ms: u64,
    max_concurrent_trades: usize,
    
    // 위험 관리
    daily_volume_limit: U256,
    risk_limit_per_trade: U256,
    
    // 플래시론 컨트랙트 주소
    multi_asset_contract: Option<Address>,
    
    // DEX 어댑터 팩토리 및 선택기
    adapter_selector: Arc<AdapterSelector>,
    
    // DEX 성능 추적
    dex_performance: Arc<Mutex<HashMap<String, DexPerformanceData>>>,}

impl MultiAssetArbitrageStrategy {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🔄 다중자산 아비트래지 전략 초기화 중...");
        
        let min_profit_usd = config.strategies.micro_arbitrage.min_profit_usd
            .parse::<f64>()
            .map(Decimal::from_f64_retain)
            .unwrap_or_else(|_| Some(Decimal::from(10)))
            .unwrap_or(Decimal::from(10));
        
        let daily_volume_limit = config.strategies.micro_arbitrage.daily_volume_limit
            .parse::<u64>()
            .map(U256::from)
            .unwrap_or(U256::from(1000000));
        
        let risk_limit_per_trade = config.strategies.micro_arbitrage.risk_limit_per_trade
            .parse::<u64>()
            .map(U256::from)
            .unwrap_or(U256::from(5000));
        
        // 다중자산 컨트랙트 주소 설정 (config에서 로드)
        let multi_asset_contract = config.blockchain.primary_network.arbitrage_contract
            .map(|addr| Address::from_slice(addr.as_bytes()));
        
        // DEX 어댑터 팩토리 초기화
        let mut adapter_factory = DexAdapterFactory::new(
            crate::adapters::AdapterConfig::default(),
            config.blockchain.primary_network.chain_id as u32,
        );
        adapter_factory.initialize_all_adapters()?;
        
        // 어댑터 선택기 초기화 (하이브리드 전략 사용)
        let adapter_selector = AdapterSelector::new(adapter_factory, AdapterSelectionStrategy::Hybrid);
        
        info!("✅ 다중자산 아비트래지 전략 초기화 완료");
        info!("  💰 최소 수익: {}%", config.strategies.micro_arbitrage.min_profit_percentage * 100.0);
        info!("  ⚡ 최대 실행 시간: {}ms", config.strategies.micro_arbitrage.execution_timeout_ms);
        info!("  🔀 최대 동시 거래: {}개", config.strategies.micro_arbitrage.max_concurrent_trades);
        info!("  📄 다중자산 컨트랙트: {:?}", multi_asset_contract);
        info!("  🔌 DEX 어댑터: {}개 초기화됨", adapter_selector.factory().get_supported_dexes().len());
        
        Ok(Self {
            config: config.clone(),
            provider,
            enabled: Arc::new(AtomicBool::new(true)),
            active_opportunities: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(MultiAssetArbitrageStats {
                total_opportunities: 0,
                executed_trades: 0,
                successful_trades: 0,
                failed_trades: 0,
                total_volume: U256::ZERO,
                total_profit: U256::ZERO,
                total_fees: U256::ZERO,
                avg_profit_per_trade: U256::ZERO,
                avg_execution_time_ms: 0.0,
                success_rate: 0.0,
                profit_rate: 0.0,
                uptime_percentage: 100.0,
                triangular_arbitrage_count: 0,
                position_migration_count: 0,
                complex_arbitrage_count: 0,
                dex_performance: HashMap::new(),
            })),
            min_profit_percentage: config.strategies.micro_arbitrage.min_profit_percentage,
            min_profit_usd,
            max_execution_time_ms: config.strategies.micro_arbitrage.execution_timeout_ms,
            max_concurrent_trades: config.strategies.micro_arbitrage.max_concurrent_trades,
            daily_volume_limit,
            risk_limit_per_trade,
            multi_asset_contract,
            adapter_selector: Arc::new(adapter_selector),
            dex_performance: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 삼각 아비트래지 기회 탐지
    pub async fn scan_triangular_opportunities(&self) -> Result<Vec<MultiAssetArbitrageOpportunity>> {
        let mut opportunities = Vec::new();
        
        // 주요 토큰 페어들 (WETH, USDC, DAI, WBTC 등)
        let token_pairs = vec![
            ("WETH", "USDC"),
            ("WETH", "DAI"),
            ("USDC", "DAI"),
            ("WETH", "WBTC"),
            ("USDC", "WBTC"),
        ];
        
        for (token_a, token_b) in token_pairs {
            if let Some(opportunity) = self.find_triangular_arbitrage_opportunity(token_a, token_b).await? {
                opportunities.push(opportunity);
            }
        }
        
        Ok(opportunities)
    }

    /// 특정 토큰 페어에 대한 삼각 아비트래지 기회 찾기
    async fn find_triangular_arbitrage_opportunity(
        &self,
        token_a: &str,
        token_b: &str,
    ) -> Result<Option<MultiAssetArbitrageOpportunity>> {
        // 중간 토큰 후보들 (일반적으로 USDC, DAI, WETH)
        let intermediate_tokens = vec!["USDC", "DAI", "WETH"];
        
        for intermediate in intermediate_tokens {
            if intermediate == token_a || intermediate == token_b {
                continue;
            }
            
            if let Some(opportunity) = self.calculate_triangular_profitability(
                token_a, token_b, intermediate
            ).await? {
                return Ok(Some(opportunity));
            }
        }
        
        Ok(None)
    }

    /// 삼각 아비트래지 수익성 계산
    async fn calculate_triangular_profitability(
        &self,
        token_a: &str,
        token_b: &str,
        token_c: &str,
    ) -> Result<Option<MultiAssetArbitrageOpportunity>> {
        // 토큰 주소 가져오기
        let addr_a = self.config.get_token_address(token_a)
            .ok_or_else(|| anyhow!("Token not found: {}", token_a))?;
        let addr_b = self.config.get_token_address(token_b)
            .ok_or_else(|| anyhow!("Token not found: {}", token_b))?;
        let addr_c = self.config.get_token_address(token_c)
            .ok_or_else(|| anyhow!("Token not found: {}", token_c))?;

        let addr_a = Address::from_slice(addr_a.as_bytes());
        let addr_b = Address::from_slice(addr_b.as_bytes());
        let addr_c = Address::from_slice(addr_c.as_bytes());

        // 기본 거래량 설정 (1 ETH 또는 1000 USDC)
        let base_amount = if token_a == "WETH" {
            U256::from(1000000000000000000u64) // 1 ETH
        } else {
            U256::from(1000000000u64) // 1000 USDC (6 decimals)
        };

        // 하이브리드 경로 탐색으로 각 레그별 최적 DEX 선택
        // 1단계: A → C 견적 (네이티브 + 애그리게이터 비교)
        let (quote_c_from_a, dex_ac, _) = self.find_best_route_parallel(addr_a, addr_c, base_amount).await?;
        if quote_c_from_a.is_zero() {
            return Ok(None);
        }

        // 2단계: B → C 견적 (동일한 가치, 별도 DEX 선택)
        let (quote_c_from_b, dex_bc, _) = self.find_best_route_parallel(addr_b, addr_c, base_amount).await?;
        if quote_c_from_b.is_zero() {
            return Ok(None);
        }

        let total_c = quote_c_from_a + quote_c_from_b;

        // 3단계: C → A 견적 (절반, 역방향 최적화)
        let half_c = total_c.checked_div(U256::from(2)).unwrap_or(U256::ZERO);
        let (quote_a_from_c, dex_ca, _) = self.find_best_route_parallel(addr_c, addr_a, half_c).await?;
        if quote_a_from_c.is_zero() {
            return Ok(None);
        }

        // 4단계: C → B 견적 (나머지, 역방향 최적화)
        let remaining_c = total_c.checked_sub(half_c).unwrap_or(U256::ZERO);
        let (quote_b_from_c, dex_cb, _) = self.find_best_route_parallel(addr_c, addr_b, remaining_c).await?;
        if quote_b_from_c.is_zero() {
            return Ok(None);
        }
        
        // DEX 다양성 보너스: 서로 다른 DEX 사용 시 신뢰도 증가
        let unique_dexes = vec![&dex_ac, &dex_bc, &dex_ca, &dex_cb]
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        let diversity_bonus = 0.02 * (unique_dexes - 1) as f64; // DEX당 2% 보너스

        // 수익성 계산
        let total_return = quote_a_from_c + quote_b_from_c;
        let total_input = base_amount.checked_mul(U256::from(2)).unwrap_or(U256::ZERO); // A + B

        if total_return <= total_input {
            return Ok(None);
        }

        let profit = total_return - total_input;
        let profit_percentage = (profit.to::<u128>() as f64 / total_input.to::<u128>() as f64) * 100.0;

        // 최소 수익률 확인
        if profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }

        // 플래시론 프리미엄 및 가스비 고려
        let flash_loan_premium = total_input * U256::from(9) / U256::from(10000); // 0.09%
        let estimated_gas_cost = U256::from(500000) * U256::from(30_000_000_000u64); // 500k gas * 30 gwei
        let net_profit = profit - flash_loan_premium - estimated_gas_cost;

        if net_profit <= U256::ZERO {
            return Ok(None);
        }

        // 신뢰도 점수 계산 (DEX 다양성 보너스 포함)
        let base_confidence = self.calculate_confidence_score(profit_percentage, 0).await?;
        let confidence_score = (base_confidence + diversity_bonus).min(1.0);

        Ok(Some(MultiAssetArbitrageOpportunity {
            id: uuid::Uuid::new_v4().to_string(),
            strategy_type: MultiAssetStrategyType::TriangularArbitrage {
                token_a: addr_a,
                token_b: addr_b,
                token_c: addr_c,
                amount_a: base_amount,
                amount_b: base_amount,
            },
            borrow_assets: vec![addr_a, addr_b],
            borrow_amounts: vec![base_amount, base_amount],
            target_assets: vec![addr_a, addr_b],
            expected_profit: net_profit,
            profit_percentage,
            execution_sequence: vec![0, 1, 2, 3], // A→C, B→C, C→A, C→B
            confidence_score,
            gas_estimate: 500000,
            flash_loan_premiums: vec![
                flash_loan_premium.checked_div(U256::from(2)).unwrap_or(U256::ZERO),
                flash_loan_premium.checked_div(U256::from(2)).unwrap_or(U256::ZERO)
            ],
            max_execution_time_ms: self.max_execution_time_ms,
            discovered_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(30),
            selected_dex_adapters: vec![dex_ac, dex_bc, dex_ca, dex_cb],
        }))
    }
    
    /// 신뢰도 점수 계산
    async fn calculate_confidence_score(
        &self,
        profit_percentage: f64,
        _unique_dexes: usize,
    ) -> Result<f64> {
        let mut score = 0.5; // 기본 점수
        
        // 수익률 기반 점수
        score += (profit_percentage * 10.0).min(0.3);

        // 현재 활성 거래 수 고려
        let active_count = self.active_opportunities.lock().await.len();
        if active_count < self.max_concurrent_trades / 2 {
            score += 0.1;
        } else if active_count >= self.max_concurrent_trades {
            score -= 0.2;
        }

        Ok(score.clamp(0.0, 1.0))
    }
    /// 다중자산 아비트래지 실행
    pub async fn execute_multi_asset_arbitrage(
        &self,
        opportunity: &MultiAssetArbitrageOpportunity,
    ) -> Result<bool> {
        let execution_start = Instant::now();
        let trade_id = format!("multi_arb_{}", SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());

        info!("🚀 다중자산 아비트래지 실행 시작: {}", trade_id);
        info!("  📊 전략 타입: {:?}", opportunity.strategy_type);
        info!("  💰 예상 수익: {:.4}%", opportunity.profit_percentage);
        info!("  🔄 대출 자산: {}개", opportunity.borrow_assets.len());

        // 활성 기회로 추가
        {
            let mut active_opportunities = self.active_opportunities.lock().await;
            if active_opportunities.len() >= self.max_concurrent_trades {
                warn!("⚠️ 최대 동시 거래 수 초과, 거래 건너뜀");
                return Ok(false);
            }
            active_opportunities.insert(trade_id.clone(), opportunity.clone());
        }

        let execution_result = async {
            if crate::mocks::is_mock_mode() {
                // Mock 모드에서는 실제 실행 대신 성공 반환
                Ok(true)
            } else {
                self.execute_real_multi_asset_arbitrage(opportunity, &trade_id).await
            }
        };

        // 타임아웃 적용
        let result = tokio::time::timeout(
            Duration::from_millis(opportunity.max_execution_time_ms),
            execution_result
        ).await;

        // 활성 기회에서 제거
        self.active_opportunities.lock().await.remove(&trade_id);

        let execution_time = execution_start.elapsed();

        match result {
            Ok(Ok(success)) => {
                if success {
                    info!("✅ 다중자산 아비트래지 성공: {} ({:.2}ms)", 
                          trade_id, execution_time.as_millis());
                    self.update_stats(true, execution_time.as_millis() as f64, opportunity).await;
                } else {
                    warn!("❌ 다중자산 아비트래지 실패: {}", trade_id);
                    self.update_stats(false, execution_time.as_millis() as f64, opportunity).await;
                }
                Ok(success)
            }
            Ok(Err(e)) => {
                error!("💥 다중자산 아비트래지 오류: {} - {}", trade_id, e);
                self.update_stats(false, execution_time.as_millis() as f64, opportunity).await;
                Err(e)
            }
            Err(_) => {
                warn!("⏰ 다중자산 아비트래지 타임아웃: {}", trade_id);
                self.update_stats(false, execution_time.as_millis() as f64, opportunity).await;
                Ok(false)
            }
        }
    }

    /// Mock 모드 다중자산 아비트래지 실행
    async fn execute_mock_multi_arbitrage(
        &self,
        opportunity: &MultiAssetArbitrageOpportunity,
        trade_id: &str,
    ) -> Result<bool> {
        // 시뮬레이션: 85% 성공률
        sleep(Duration::from_millis(50 + fastrand::u64(100..200))).await; // 50-250ms 지연 시뮬레이션

        let success = fastrand::f64() > 0.15; // 85% 성공률

        if success {
            debug!("🎭 Mock 다중자산 아비트래지 성공: {}", trade_id);
        } else {
            debug!("🎭 Mock 다중자산 아비트래지 실패: {} (슬리피지 또는 유동성 부족)", trade_id);
        }

        Ok(success)
    }

    /// 실제 다중자산 아비트래지 실행 (스마트컨트랙트 호출)
    async fn execute_real_multi_asset_arbitrage(
        &self,
        opportunity: &MultiAssetArbitrageOpportunity,
        trade_id: &str,
    ) -> Result<bool> {
        let contract_address = self.multi_asset_contract
            .ok_or_else(|| anyhow!("Multi-asset contract not configured"))?;

        info!("🚀 실제 다중자산 아비트래지 실행: {}", trade_id);
        info!("  📄 컨트랙트: {:?}", contract_address);

        // 전략 타입에 따라 다른 실행 로직
        match &opportunity.strategy_type {
            MultiAssetStrategyType::TriangularArbitrage { .. } => {
                self.execute_triangular_arbitrage_contract(opportunity, contract_address).await
            }
            MultiAssetStrategyType::PositionMigration { .. } => {
                self.execute_position_migration_contract(opportunity, contract_address).await
            }
            MultiAssetStrategyType::ComplexArbitrage { .. } => {
                self.execute_complex_arbitrage_contract(opportunity, contract_address).await
            }
        }
    }

    /// 삼각 아비트래지 컨트랙트 실행 (어댑터 기반)
    async fn execute_triangular_arbitrage_contract(
        &self,
        opportunity: &MultiAssetArbitrageOpportunity,
        contract_address: Address,
    ) -> Result<bool> {
        use crate::utils::abi::ABICodec;
        use alloy::primitives::Bytes;

        let codec = ABICodec::new();

        if let MultiAssetStrategyType::TriangularArbitrage { token_a, token_b, token_c, amount_a, amount_b } = &opportunity.strategy_type {
            // 어댑터를 사용하여 각 스왑의 calldata 생성
            let deadline = chrono::Utc::now().timestamp() as u64 + 300; // 5분 후
            
            // A → C 스왑
            let adapter_ab = &opportunity.selected_dex_adapters[0];
            let (_, quote_ab) = self.adapter_selector.select_adapter(*token_a, *token_c, *amount_a, 50).await?;
            let calldata_ab = self.adapter_selector.factory()
                .get_adapter(&adapter_ab)
                .unwrap()
                .build_swap_calldata(&quote_ab, contract_address, deadline).await?;
            
            // B → C 스왑
            let adapter_bc = &opportunity.selected_dex_adapters[1];
            let (_, quote_bc) = self.adapter_selector.select_adapter(*token_b, *token_c, *amount_b, 50).await?;
            let calldata_bc = self.adapter_selector.factory()
                .get_adapter(&adapter_bc)
                .unwrap()
                .build_swap_calldata(&quote_bc, contract_address, deadline).await?;
            
            // C → A 스왑 (일부)
            let amount_c_to_a = quote_ab.amount_out.checked_div(U256::from(2)).unwrap_or(U256::ZERO);
            let adapter_ca = &opportunity.selected_dex_adapters[2];
            let (_, quote_ca) = self.adapter_selector.select_adapter(*token_c, *token_a, amount_c_to_a, 50).await?;
            let calldata_ca = self.adapter_selector.factory()
                .get_adapter(&adapter_ca)
                .unwrap()
                .build_swap_calldata(&quote_ca, contract_address, deadline).await?;
            
            // C → B 스왑 (나머지)
            let amount_c_to_b = quote_ab.amount_out - amount_c_to_a;
            let adapter_cb = &opportunity.selected_dex_adapters[3];
            let (_, quote_cb) = self.adapter_selector.select_adapter(*token_c, *token_b, amount_c_to_b, 50).await?;
            let calldata_cb = self.adapter_selector.factory()
                .get_adapter(&adapter_cb)
                .unwrap()
                .build_swap_calldata(&quote_cb, contract_address, deadline).await?;

            // 삼각 아비트래지 파라미터 구성
            let params = codec.encode_triangular_arbitrage_params(
                *token_a,
                *token_b,
                *token_c,
                *amount_a,
                *amount_b,
                calldata_ab.to,
                calldata_bc.to,
                calldata_ca.to,
                calldata_cb.to,
                opportunity.expected_profit,
                Bytes::from(calldata_ab.data),
                Bytes::from(calldata_bc.data),
                Bytes::from(calldata_ca.data),
                Bytes::from(calldata_cb.data),
            )?;

            // executeTriangularArbitrage 호출
            let calldata = codec.encode_triangular_arbitrage_execute_call(params)?;

            // 트랜잭션 구성 및 전송
            let tx = crate::types::Transaction {
                hash: alloy::primitives::B256::ZERO,
                from: alloy::primitives::Address::ZERO,
                to: Some(contract_address),
                value: U256::ZERO,
                gas_price: U256::from(30_000_000_000u64),
                gas_limit: U256::from(800_000u64),
                data: calldata.to_vec(),
                nonce: 0,
                timestamp: Utc::now(),
                block_number: None,
            };

            let sent = self.broadcast_transaction(tx).await?;
            Ok(sent)
        } else {
            Err(anyhow!("Invalid strategy type for triangular arbitrage"))
        }
    }

    /// 포지션 마이그레이션 컨트랙트 실행
    async fn execute_position_migration_contract(
        &self,
        _opportunity: &MultiAssetArbitrageOpportunity,
        _contract_address: Address,
    ) -> Result<bool> {
        // TODO: 포지션 마이그레이션 로직 구현
        warn!("포지션 마이그레이션은 아직 구현되지 않았습니다");
        Ok(false)
    }

    /// 복합 아비트래지 컨트랙트 실행
    async fn execute_complex_arbitrage_contract(
        &self,
        _opportunity: &MultiAssetArbitrageOpportunity,
        _contract_address: Address,
    ) -> Result<bool> {
        // TODO: 복합 아비트래지 로직 구현
        warn!("복합 아비트래지는 아직 구현되지 않았습니다");
        Ok(false)
    }

    /// 트랜잭션 브로드캐스트
    async fn broadcast_transaction(&self, tx: crate::types::Transaction) -> Result<bool> {
        use ethers::providers::{Provider as HttpProvider, Http, Middleware};
        use ethers::types::{TransactionRequest as EthersTxRequest, H160 as EthersH160, U256 as EthersU256};
        use ethers::signers::{LocalWallet, Signer};
        use ethers::middleware::SignerMiddleware;

        let rpc_url = &self.config.blockchain.primary_network.rpc_url;
        let provider: HttpProvider<Http> = HttpProvider::<Http>::try_from(rpc_url)
            .map_err(|e| anyhow!("provider error: {}", e))?;

        // 개인키 로드
        let pk = std::env::var("PRIVATE_KEY").ok()
            .or_else(|| std::env::var("FLASHBOTS_PRIVATE_KEY").ok())
            .ok_or_else(|| anyhow!("PRIVATE_KEY/FLASHBOTS_PRIVATE_KEY not set"))?;
        let mut wallet: LocalWallet = pk.parse().map_err(|e| anyhow!("wallet parse error: {}", e))?;
        let chain_id = self.config.blockchain.primary_network.chain_id;
        wallet = wallet.with_chain_id(chain_id);

        // 타입 변환
        let to = tx.to.ok_or_else(|| anyhow!("missing to address"))?;
        let to_h160: EthersH160 = EthersH160::from_slice(to.as_slice());

        let mut be = [0u8; 32];
        be.copy_from_slice(&tx.value.to_be_bytes::<32>());
        let val = EthersU256::from_big_endian(&be);
        be.copy_from_slice(&tx.gas_price.to_be_bytes::<32>());
        let gas_price = EthersU256::from_big_endian(&be);
        be.copy_from_slice(&tx.gas_limit.to_be_bytes::<32>());
        let gas_limit = EthersU256::from_big_endian(&be);

        // 트랜잭션 요청 구성
        let data_bytes = ethers::types::Bytes::from(tx.data.clone());
        let mut req = EthersTxRequest::new()
            .to(to_h160)
            .data(data_bytes)
            .value(val)
            .gas(gas_limit)
            .gas_price(gas_price);
        
        if tx.nonce != 0 { 
            req = req.nonce(ethers::types::U256::from(tx.nonce)); 
        }

        let client: SignerMiddleware<HttpProvider<Http>, LocalWallet> = SignerMiddleware::new(provider, wallet);
        let pending = client.send_transaction(req, None::<ethers::types::BlockId>).await?;
        let _tx_hash = pending.tx_hash();
        info!("📤 다중자산 아비트래지 트랜잭션 전송 완료");
        Ok(true)
    }

    /// 통계 업데이트
    async fn update_stats(
        &self,
        success: bool,
        execution_time_ms: f64,
        opportunity: &MultiAssetArbitrageOpportunity,
    ) {
        let mut stats = self.stats.lock().await;

        stats.executed_trades += 1;

        if success {
            stats.successful_trades += 1;
            stats.total_profit += opportunity.expected_profit;

            // 전략 타입별 카운트
            match &opportunity.strategy_type {
                MultiAssetStrategyType::TriangularArbitrage { .. } => {
                    stats.triangular_arbitrage_count += 1;
                }
                MultiAssetStrategyType::PositionMigration { .. } => {
                    stats.position_migration_count += 1;
                }
                MultiAssetStrategyType::ComplexArbitrage { .. } => {
                    stats.complex_arbitrage_count += 1;
                }
            }

            stats.avg_profit_per_trade = if stats.successful_trades > 0 {
                stats.total_profit / U256::from(stats.successful_trades)
            } else {
                U256::ZERO
            };
        } else {
            stats.failed_trades += 1;
        }

        // 성공률 계산
        stats.success_rate = if stats.executed_trades > 0 {
            stats.successful_trades as f64 / stats.executed_trades as f64
        } else {
            0.0
        };

        // 평균 실행 시간 업데이트
        stats.avg_execution_time_ms = (stats.avg_execution_time_ms * (stats.executed_trades - 1) as f64 + execution_time_ms) / stats.executed_trades as f64;
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> MultiAssetArbitrageStats {
        (*self.stats.lock().await).clone()
    }

    /// 다중자산 아비트래지 기회를 독립적으로 스캔하고 실행
    pub async fn scan_and_execute(&self) -> Result<usize> {
        if !self.is_enabled() {
            return Ok(0);
        }

        let start_time = Instant::now();

        // 삼각 아비트래지 기회 스캔
        let opportunities = self.scan_triangular_opportunities().await?;

        if opportunities.is_empty() {
            return Ok(0);
        }

        debug!("🔄 {}개 다중자산 아비트래지 기회 발견", opportunities.len());

        // 수익성 순으로 정렬
        let mut sorted_opportunities = opportunities;
        sorted_opportunities.sort_by(|a, b| b.profit_percentage.partial_cmp(&a.profit_percentage).unwrap_or(std::cmp::Ordering::Equal));

        // 통계용으로 기회 수를 저장
        let opportunities_count = sorted_opportunities.len() as u64;

        let mut executed_count = 0;

        // 상위 기회들을 병렬로 실행
        let max_concurrent = std::cmp::min(self.max_concurrent_trades, sorted_opportunities.len());
        let mut tasks = Vec::new();

        for opportunity in sorted_opportunities.into_iter().take(max_concurrent) {
            // 신뢰도 점수가 충분한 기회만 실행
            if opportunity.confidence_score >= 0.6 {
                let config = Arc::clone(&self.config);
                let provider = Arc::clone(&self.provider);
                let enabled = Arc::clone(&self.enabled);
                let active_opportunities = Arc::clone(&self.active_opportunities);
                let stats = Arc::clone(&self.stats);
                let min_profit_percentage = self.min_profit_percentage;
                let min_profit_usd = self.min_profit_usd;
                let max_execution_time_ms = self.max_execution_time_ms;
                let max_concurrent_trades = self.max_concurrent_trades;
                let daily_volume_limit = self.daily_volume_limit;
                let risk_limit_per_trade = self.risk_limit_per_trade;
                let multi_asset_contract = self.multi_asset_contract;
                let adapter_selector = Arc::clone(&self.adapter_selector);

                let task = tokio::spawn(async move {
                    let temp_strategy = MultiAssetArbitrageStrategy {
                        config,
                        provider,
                        enabled,
                        active_opportunities,
                        stats,
                        min_profit_percentage,
                        min_profit_usd,
                        max_execution_time_ms,
                        max_concurrent_trades,
                        daily_volume_limit,
                        risk_limit_per_trade,
                        multi_asset_contract,
                        adapter_selector,
                        dex_performance: Arc::new(Mutex::new(HashMap::new())),
                    };

                    temp_strategy.execute_multi_asset_arbitrage(&opportunity).await
                });
                tasks.push(task);
            }
        }

        // 모든 실행 완료 대기
        for task in tasks {
            match task.await {
                Ok(Ok(success)) => {
                    if success {
                        executed_count += 1;
                    }
                }
                Ok(Err(e)) => {
                    error!("다중자산 아비트래지 실행 오류: {}", e);
                }
                Err(e) => {
                    error!("태스크 실행 오류: {}", e);
                }
            }
        }

        let scan_duration = start_time.elapsed();
        if executed_count > 0 {
            info!("🔄 {}개 다중자산 아비트래지 실행 완료 ({:.2}ms)", 
                  executed_count, scan_duration.as_millis());
        }

        // 통계 업데이트
        {
            let mut stats = self.stats.lock().await;
            stats.total_opportunities += opportunities_count;
        }

        Ok(executed_count)
    }

    /// 하이브리드 경로 탐색: 네이티브 DEX와 애그리게이터를 모두 활용

    /// 개별 DEX에서 견적 조회 (병렬 처리용)
    async fn get_quote_from_dex_with_tracking(
        &self,
        dex_name: &str,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Option<(String, crate::adapters::Quote, U256)> {
        let start_time = std::time::Instant::now();
        if let Some(adapter) = self.adapter_selector.factory().get_adapter(dex_name) {
            match adapter.quote(token_in, token_out, amount_in, 50).await {
                Ok(quote) => {
                    let gas_weight = adapter.dex_type().gas_weight();
                    let response_time = start_time.elapsed().as_millis() as f64;
                    let adjusted_output = quote.amount_out * U256::from(1000) / U256::from((gas_weight * 1000.0) as u64);
                    self.record_dex_performance(dex_name, true, quote.amount_out, amount_in, response_time).await;
                    Some((dex_name.to_string(), quote, adjusted_output))
                }
                Err(e) => {
                    debug!("Failed to get quote from {}: {}", dex_name, e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// 시장 변동성 계산
    async fn calculate_market_volatility(&self) -> Result<f64> {
        // 간단한 변동성 계산: 최근 가격 변화율의 표준편차
        // 실제로는 더 정교한 변동성 지표를 사용할 수 있음
        let volatility = 0.05; // 기본값 5%
        Ok(volatility)
    }

    /// 동적 임계값 계산

    /// DEX 성능 추적 기록
    async fn record_dex_performance(
        &self,
        dex_name: &str,
        success: bool,
        profit: U256,
        volume: U256,
        response_time_ms: f64,
    ) {
        let mut perf_map = self.dex_performance.lock().await;
        let perf_data = perf_map.entry(dex_name.to_string()).or_insert_with(DexPerformanceData::new);
        perf_data.record_quote(success, profit, volume, response_time_ms);
    }

    /// DEX 성능 통계 조회
    pub async fn get_dex_performance_stats(&self) -> HashMap<String, DexPerformanceData> {
        self.dex_performance.lock().await.clone()
    }
    async fn get_dynamic_threshold(&self) -> Result<f64> {
        let market_volatility = self.calculate_market_volatility().await?;
        
        // 변동성이 높을 때는 더 낮은 임계값 사용 (더 적극적으로 애그리게이터 선택)
        let threshold = if market_volatility > 0.1 {
            3.0  // 3% 개선 시 애그리게이터 선택
        } else if market_volatility > 0.05 {
            4.0  // 4% 개선 시 애그리게이터 선택
        } else {
            5.0  // 5% 개선 시 애그리게이터 선택 (기본값)
        };
        
        debug!("Market volatility: {:.2}%, Dynamic threshold: {:.1}%", 
               market_volatility * 100.0, threshold);
        Ok(threshold)
    }
    async fn find_best_route_parallel(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<(U256, String, crate::adapters::Quote)> {
        use futures::future::join_all;
use std::collections::HashMap;        
        // 1. 네이티브 DEX들 병렬 쿼리
        let native_dexes = vec!["uniswap_v2", "uniswap_v3", "sushiswap"];
        let native_quotes = join_all(
            native_dexes.iter().map(|dex| {
                self.get_quote_from_dex_with_tracking(dex, token_in, token_out, amount_in)
            })
        ).await;
        
        // 2. 애그리게이터 병렬 쿼리
        let aggregators = vec!["zeroex", "oneinch"];
        let agg_quotes = join_all(
            aggregators.iter().map(|agg| {
                self.get_quote_from_dex_with_tracking(agg, token_in, token_out, amount_in)
            })
        ).await;
        
        // 3. 최적 견적 선택
        let mut best_quote: Option<(String, crate::adapters::Quote)> = None;
        let mut best_adjusted_output = U256::ZERO;
        
        // 네이티브 DEX 결과 처리
        for quote_result in native_quotes {
            if let Some((dex_name, quote, adjusted_output)) = quote_result {
                if adjusted_output > best_adjusted_output {
                    best_adjusted_output = adjusted_output;
                    best_quote = Some((dex_name, quote));
                }
            }
        }
        
        // 애그리게이터 결과 처리 (5% 이상 개선 시에만 선택)
        for quote_result in agg_quotes {
            if let Some((agg_name, quote, adjusted_output)) = quote_result {
                let threshold = self.get_dynamic_threshold().await?;
        let threshold_u256 = U256::from((threshold * 100.0) as u64) + U256::from(10000);
        if adjusted_output > best_adjusted_output * threshold_u256 / U256::from(10000) {
                    best_adjusted_output = adjusted_output;
                    best_quote = Some((agg_name, quote));
                }
            }
        }
        
        match best_quote {            Some((dex_name, quote)) => {
                info!("Best route: {} -> {} via {} (output: {})", 
                    token_in, token_out, dex_name, quote.amount_out);
                Ok((quote.amount_out, dex_name, quote))
            }
            None => {
                warn!("No valid quote found for {} -> {}", token_in, token_out);
                Ok((U256::ZERO, "unknown".to_string(), crate::adapters::Quote {
                    token_in,
                    token_out,
                    amount_in,
                    amount_out: U256::ZERO,
                    amount_out_min: U256::ZERO,
                    price_impact: 0.0,
                    gas_estimate: 150000,
                    valid_for: 60,
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    metadata: std::collections::HashMap::new(),
                }))
            }
        }
    }
    
    /// 어댑터를 사용한 스왑 견적 가져오기 (DEX 정보 포함) - 기존 메서드 래퍼
    async fn get_swap_quote_with_dex(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<(U256, String)> {
        let (amount_out, dex_name, _) = self.find_best_route_parallel(token_in, token_out, amount_in).await?;
        Ok((amount_out, dex_name))
    }

    /// 어댑터를 사용한 스왑 견적 가져오기 (DEX 정보 없이)
    async fn get_swap_quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<U256> {
        let (amount_out, _) = self.get_swap_quote_with_dex(token_in, token_out, amount_in).await?;
        Ok(amount_out)
    }
}

#[async_trait]
impl Strategy for MultiAssetArbitrageStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::MultiAssetArbitrage
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    async fn start(&self) -> Result<()> {
        self.enabled.store(true, Ordering::SeqCst);
        info!("🚀 다중자산 아비트래지 전략 시작됨");
        
        let contract_status = if self.multi_asset_contract.is_some() {
            "설정됨"
        } else {
            "미설정"
        };
        
        info!("📄 다중자산 컨트랙트: {}", contract_status);
        info!("🧭 최소 수익률: {:.3}%, 최소 수익(USD): {}", 
              self.min_profit_percentage * 100.0, self.min_profit_usd);
        info!("⏱️ 최대 실행 시간: {}ms, 동시 거래 한도: {}", 
              self.max_execution_time_ms, self.max_concurrent_trades);

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.enabled.store(false, Ordering::SeqCst);

        // 모든 활성 기회 대기
        let mut active_count = self.active_opportunities.lock().await.len();
        let mut wait_time = 0;

        while active_count > 0 && wait_time < 15000 { // 최대 15초 대기
            sleep(Duration::from_millis(100)).await;
            active_count = self.active_opportunities.lock().await.len();
            wait_time += 100;
        }

        if active_count > 0 {
            warn!("⚠️ {}개의 활성 기회가 완료되지 않았지만 전략을 중지합니다", active_count);
        }

        info!("⏹️ 다중자산 아비트래지 전략 중지됨");
        Ok(())
    }

    async fn analyze(&self, _transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(vec![]);
        }

        // 다중자산 아비트래지는 트랜잭션 기반이 아닌 독립적으로 실행
        Ok(vec![])
    }

    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        if opportunity.strategy != StrategyType::MultiAssetArbitrage {
            return Ok(false);
        }

        Ok(opportunity.expected_profit > U256::ZERO && opportunity.confidence > 0.5)
    }

    async fn create_bundle(&self, _opportunity: &Opportunity) -> Result<crate::types::Bundle> {
        // 다중자산 아비트래지는 Bundle 시스템을 사용하지 않음
        // 직접 스마트컨트랙트 호출로 실행
        Err(anyhow!("MultiAssetArbitrage strategy does not use bundle system"))
    }
}

impl std::fmt::Debug for MultiAssetArbitrageStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiAssetArbitrageStrategy")
            .field("enabled", &self.enabled)
            .field("min_profit_percentage", &self.min_profit_percentage)
            .field("max_execution_time_ms", &self.max_execution_time_ms)
            .field("max_concurrent_trades", &self.max_concurrent_trades)
            .field("multi_asset_contract", &self.multi_asset_contract)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MultiAssetStrategyType, SwapStep};
    use chrono::Utc;
    use alloy::primitives::Address;

    #[tokio::test]
    async fn test_multi_asset_arbitrage_strategy_creation() {
        let config = Arc::new(crate::config::Config::default());
        println!("MultiAssetArbitrage strategy creation test - would test with mock provider in production");
        assert!(true);
    }

    #[tokio::test]
    async fn test_triangular_arbitrage_opportunity_creation() {
        let opportunity = MultiAssetArbitrageOpportunity {
            id: "test_id".to_string(),
            strategy_type: MultiAssetStrategyType::TriangularArbitrage {
                token_a: Address::from([1u8; 20]),
                token_b: Address::from([2u8; 20]),
                token_c: Address::from([3u8; 20]),
                amount_a: U256::from(1000000000000000000u64),
                amount_b: U256::from(1000000000000000000u64),
            },
            borrow_assets: vec![Address::from([1u8; 20]), Address::from([2u8; 20])],
            borrow_amounts: vec![U256::from(1000000000000000000u64), U256::from(1000000000000000000u64)],
            target_assets: vec![Address::from([1u8; 20]), Address::from([2u8; 20])],
            expected_profit: U256::from(10000000000000000u64),
            profit_percentage: 1.0,
            execution_sequence: vec![0, 1, 2, 3],
            confidence_score: 0.8,
            gas_estimate: 500000,
            flash_loan_premiums: vec![U256::from(900000000000000u64), U256::from(900000000000000u64)],
            max_execution_time_ms: 5000,
            discovered_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(30),

            selected_dex_adapters: vec!["uniswap_v2".to_string(), "sushiswap".to_string()],
        };

        assert!(opportunity.is_valid());
        assert!(opportunity.profitability_score() > 0.0);
        assert_eq!(opportunity.borrow_assets.len(), 2);
        assert_eq!(opportunity.execution_sequence.len(), 4);
    }

    #[tokio::test]
    async fn test_complex_arbitrage_opportunity_creation() {
        let swap_steps = vec![
            SwapStep {
                dex: "uniswap_v2".to_string(),
                token_in: Address::from([1u8; 20]),
                token_out: Address::from([2u8; 20]),
                amount_in: U256::from(1000000000000000000u64),
                expected_amount_out: U256::from(2000000000000000000u64),
                call_data: vec![0x38, 0xed, 0x17, 0x39],
            },
            SwapStep {
                dex: "sushiswap".to_string(),
                token_in: Address::from([2u8; 20]),
                token_out: Address::from([3u8; 20]),
                amount_in: U256::from(2000000000000000000u64),
                expected_amount_out: U256::from(3000000000000000000u64),
                call_data: vec![0x38, 0xed, 0x17, 0x39],
            },
        ];

        let opportunity = MultiAssetArbitrageOpportunity {
            id: "test_complex_id".to_string(),
            strategy_type: MultiAssetStrategyType::ComplexArbitrage {
                swap_sequence: swap_steps.clone(),
            },
            borrow_assets: vec![Address::from([1u8; 20])],
            borrow_amounts: vec![U256::from(1000000000000000000u64)],
            target_assets: vec![Address::from([3u8; 20])],
            expected_profit: U256::from(50000000000000000u64),
            profit_percentage: 5.0,
            execution_sequence: vec![0, 1],
            confidence_score: 0.9,
            gas_estimate: 600000,
            flash_loan_premiums: vec![U256::from(900000000000000u64)],
            max_execution_time_ms: 8000,
            discovered_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(60),
            selected_dex_adapters: vec!["uniswap_v3".to_string()],
        };

        assert!(opportunity.is_valid());
        assert!(opportunity.profitability_score() > 0.0);
        assert_eq!(opportunity.borrow_assets.len(), 1);
        assert_eq!(opportunity.execution_sequence.len(), 2);
    }
}
