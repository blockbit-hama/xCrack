use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use tracing::{info, debug, warn, error};
use alloy::primitives::{Address, B256, U256};
use ethers::providers::{Provider, Ws, Middleware};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::types::{Transaction, Opportunity, StrategyType, Bundle};
use crate::strategies::Strategy;
use crate::blockchain::{
    BlockchainClient, ContractFactory, DexRouterContract, AmmPoolContract, 
    TransactionDecoder, EventListener, LogParser
};

/// 온체인 데이터 기반 실시간 샌드위치 전략
/// 
/// 실제 블록체인 RPC를 사용하여 AMM 풀 상태를 실시간으로 모니터링하고,
/// 멤풀에서 대형 스왑 트랜잭션을 감지하여 샌드위치 공격을 실행합니다.
pub struct OnChainSandwichStrategy {
    config: Arc<Config>,
    blockchain_client: Arc<BlockchainClient>,
    contract_factory: Arc<ContractFactory>,
    tx_decoder: Arc<TransactionDecoder>,
    enabled: Arc<AtomicBool>,
    
    // AMM 풀 정보 캐시
    pool_cache: Arc<Mutex<HashMap<Address, PoolInfo>>>,
    
    // 실시간 가격 데이터
    price_cache: Arc<Mutex<HashMap<(Address, Address), PriceInfo>>>,
    
    // 수익성 임계값
    min_profit_eth: U256,
    min_profit_percentage: f64,
    
    // 가스 전략
    gas_multiplier: f64,
    max_gas_price: U256,
    
    // 통계
    stats: Arc<Mutex<OnChainSandwichStats>>,
}

#[derive(Debug, Clone)]
struct PoolInfo {
    /// 풀 주소
    address: Address,
    /// 토큰 0
    token0: Address,
    /// 토큰 1
    token1: Address,
    /// 리저브 0
    reserve0: U256,
    /// 리저브 1
    reserve1: U256,
    /// 수수료 (basis points)
    fee: u32,
    /// 마지막 업데이트 시간
    last_updated: Instant,
}

#[derive(Debug, Clone)]
struct PriceInfo {
    /// 현재 가격 (토큰1/토큰0)
    price: f64,
    /// 가격 임팩트 (예상)
    price_impact: f64,
    /// 마지막 업데이트
    timestamp: Instant,
}

#[derive(Debug, Clone)]
struct OnChainSandwichStats {
    pools_monitored: u64,
    transactions_analyzed: u64,
    opportunities_found: u64,
    successful_sandwiches: u64,
    total_profit: U256,
    avg_profit_per_sandwich: U256,
    avg_gas_used: U256,
    last_analysis_time: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct OnChainSandwichOpportunity {
    /// 타겟 트랜잭션
    pub target_tx: Transaction,
    /// 풀 정보
    pub pool: PoolInfo,
    /// 프론트런 트랜잭션
    pub front_run_tx: Transaction,
    /// 백런 트랜잭션
    pub back_run_tx: Transaction,
    /// 예상 수익
    pub expected_profit: U256,
    /// 가스 비용
    pub gas_cost: U256,
    /// 순수익
    pub net_profit: U256,
    /// 성공 확률
    pub success_probability: f64,
    /// 가격 영향
    pub price_impact: f64,
}

impl OnChainSandwichStrategy {
    /// 새로운 온체인 샌드위치 전략 생성
    pub async fn new(
        config: Arc<Config>, 
        blockchain_client: Arc<BlockchainClient>
    ) -> Result<Self> {
        info!("🥪🔗 온체인 샌드위치 전략 초기화 중...");
        
        let contract_factory = Arc::new(ContractFactory::new(blockchain_client.get_provider()));
        let tx_decoder = Arc::new(TransactionDecoder::new()?);
        
        let min_profit_eth = U256::from_str_radix(
            &config.strategies.sandwich.min_profit_eth,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("100000000000000000", 10).unwrap());
        
        let min_profit_percentage = config.strategies.sandwich.min_profit_percentage;
        let gas_multiplier = config.strategies.sandwich.gas_multiplier;
        let max_gas_price = U256::from_str_radix(
            &config.strategies.sandwich.max_gas_price_gwei,
            10
        ).unwrap_or_else(|_| U256::from(100_000_000_000u64)) * U256::from(1_000_000_000u64);
        
        info!("✅ 온체인 샌드위치 전략 초기화 완료");
        info!("  📊 최소 수익: {} ETH", format_eth_amount(min_profit_eth));
        info!("  📈 최소 수익률: {:.2}%", min_profit_percentage);
        info!("  ⛽ 가스 배수: {:.2}x", gas_multiplier);
        
        let strategy = Self {
            config,
            blockchain_client,
            contract_factory,
            tx_decoder,
            enabled: Arc::new(AtomicBool::new(true)),
            pool_cache: Arc::new(Mutex::new(HashMap::new())),
            price_cache: Arc::new(Mutex::new(HashMap::new())),
            min_profit_eth,
            min_profit_percentage,
            gas_multiplier,
            max_gas_price,
            stats: Arc::new(Mutex::new(OnChainSandwichStats {
                pools_monitored: 0,
                transactions_analyzed: 0,
                opportunities_found: 0,
                successful_sandwiches: 0,
                total_profit: U256::ZERO,
                avg_profit_per_sandwich: U256::ZERO,
                avg_gas_used: U256::ZERO,
                last_analysis_time: None,
            })),
        };
        
        // 초기 풀 데이터 로드
        strategy.initialize_pool_cache().await?;
        
        Ok(strategy)
    }
    
    /// 풀 캐시 초기화
    async fn initialize_pool_cache(&self) -> Result<()> {
        info!("🔄 AMM 풀 캐시 초기화 중...");
        
        let known_pools = vec![
            // USDC/WETH Uniswap V2
            ("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc".parse::<Address>()?, 30),
            // USDT/WETH Uniswap V2
            ("0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852".parse::<Address>()?, 30),
            // DAI/WETH Uniswap V2
            ("0xA478c2975Ab1Ea89e8196811F51A7B7Ade33eB11".parse::<Address>()?, 30),
        ];
        
        let mut pool_cache = self.pool_cache.lock().await;
        
        for (pool_address, fee) in known_pools {
            match self.load_pool_info(pool_address, fee).await {
                Ok(pool_info) => {
                    pool_cache.insert(pool_address, pool_info);
                    debug!("✅ 풀 로드: {}", pool_address);
                }
                Err(e) => {
                    warn!("⚠️ 풀 로드 실패 {}: {}", pool_address, e);
                }
            }
        }
        
        let mut stats = self.stats.lock().await;
        stats.pools_monitored = pool_cache.len() as u64;
        
        info!("✅ {} 개 풀 캐시 초기화 완료", pool_cache.len());
        Ok(())
    }
    
    /// 풀 정보 로드
    async fn load_pool_info(&self, pool_address: Address, fee: u32) -> Result<PoolInfo> {
        // Address를 H160으로 변환
        let h160_address = ethers::types::H160::from_slice(pool_address.as_slice());
        let pool_contract = self.contract_factory.create_amm_pool(h160_address)?;
        
        let token0 = pool_contract.token0().await?;
        let token1 = pool_contract.token1().await?;
        let (reserve0, reserve1, _) = pool_contract.get_reserves().await?;
        
        Ok(PoolInfo {
            address: pool_address,
            token0: Address::from_slice(token0.as_bytes()),
            token1: Address::from_slice(token1.as_bytes()),
            reserve0: U256::from_limbs_slice(&reserve0.0),
            reserve1: U256::from_limbs_slice(&reserve1.0),
            fee,
            last_updated: Instant::now(),
        })
    }
    
    /// 트랜잭션이 샌드위치 대상인지 확인 (온체인 검증 포함)
    async fn is_sandwich_target_onchain(&self, tx: &Transaction) -> Result<bool> {
        // 기본 필터링
        if let Some(to) = tx.to {
            // 알려진 DEX 라우터인지 확인
            let known_routers = vec![
                "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse::<Address>()?, // Uniswap V2
                "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse::<Address>()?, // SushiSwap
                "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse::<Address>()?, // Uniswap V3
            ];
            
            if !known_routers.contains(&to) {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
        
        // 트랜잭션 디코딩 - ethers Transaction으로 변환
        let ethers_tx = self.convert_to_ethers_transaction(tx)?;
        let decoded = self.tx_decoder.decode_transaction(&ethers_tx)?;
        
        // 스왑 트랜잭션인지 확인
        if !decoded.is_sandwich_target() {
            return Ok(false);
        }
        
        // 최소 거래 크기 확인 (실제 USD 값 계산)
        let transaction_value = self.calculate_transaction_usd_value(&decoded).await?;
        if transaction_value < 10000.0 { // $10,000 미만
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// 트랜잭션의 USD 가치 계산
    async fn calculate_transaction_usd_value(&self, decoded: &crate::blockchain::decoder::DecodedTransaction) -> Result<f64> {
        // 임시 구현 - 실제로는 오라클이나 실시간 가격 피드 사용
        let eth_usd_price = 2800.0; // $2800/ETH
        
        let mut total_value = decoded.value.as_u128() as f64 / 1e18 * eth_usd_price;
        
        // 스왑 금액 추가
        if let Some(ethers::abi::Token::Uint(amount)) = decoded.parameters.get("amountIn") {
            let amount_eth = amount.as_u128() as f64 / 1e18;
            total_value += amount_eth * eth_usd_price;
        }
        
        Ok(total_value)
    }
    
    /// 샌드위치 기회 분석 (온체인 데이터 활용)
    async fn analyze_sandwich_opportunity_onchain(&self, tx: &Transaction) -> Result<Option<OnChainSandwichOpportunity>> {
        let ethers_tx = self.convert_to_ethers_transaction(tx)?;
        let decoded = self.tx_decoder.decode_transaction(&ethers_tx)?;
        
        // 관련 풀 찾기
        let pool = self.find_affected_pool(&decoded).await?;
        if pool.is_none() {
            return Ok(None);
        }
        let pool = pool.unwrap();
        
        // 현재 풀 상태 업데이트
        let updated_pool = self.update_pool_state(&pool).await?;
        
        // 가격 영향 계산
        let price_impact = self.calculate_price_impact_onchain(&decoded, &updated_pool).await?;
        
        if price_impact < 0.005 { // 0.5% 미만이면 스킵
            return Ok(None);
        }
        
        // 최적 샌드위치 크기 계산
        let optimal_size = self.calculate_optimal_sandwich_size_onchain(&decoded, &updated_pool, price_impact).await?;
        
        // 수익성 계산
        let (expected_profit, gas_cost, net_profit) = self.calculate_sandwich_profit_onchain(
            &optimal_size, 
            &updated_pool,
            price_impact
        ).await?;
        
        // 최소 수익성 검증
        if net_profit < self.min_profit_eth {
            return Ok(None);
        }
        
        let profit_percentage = (net_profit.to::<u128>() as f64 / optimal_size.to::<u128>() as f64) * 100.0;
        if profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }
        
        // 성공 확률 계산
        let success_probability = self.calculate_success_probability_onchain(tx, &net_profit, &updated_pool).await?;
        
        if success_probability < 0.4 {
            return Ok(None);
        }
        
        // 프론트런/백런 트랜잭션 생성
        let front_run_tx = self.create_front_run_transaction_onchain(&optimal_size, &updated_pool, tx.gas_price).await?;
        let back_run_tx = self.create_back_run_transaction_onchain(&optimal_size, &updated_pool, tx.gas_price).await?;
        
        info!("🎯 온체인 샌드위치 기회 발견!");
        info!("  💰 예상 수익: {} ETH", format_eth_amount(net_profit));
        info!("  📈 수익률: {:.2}%", profit_percentage);
        info!("  🎲 성공 확률: {:.2}%", success_probability * 100.0);
        info!("  💥 가격 영향: {:.2}%", price_impact * 100.0);
        
        Ok(Some(OnChainSandwichOpportunity {
            target_tx: tx.clone(),
            pool: updated_pool,
            front_run_tx,
            back_run_tx,
            expected_profit,
            gas_cost,
            net_profit,
            success_probability,
            price_impact,
        }))
    }
    
    /// 영향받는 풀 찾기
    async fn find_affected_pool(&self, decoded: &crate::blockchain::decoder::DecodedTransaction) -> Result<Option<PoolInfo>> {
        let pool_cache = self.pool_cache.lock().await;
        
        // path에서 토큰 페어 추출
        if let Some(ethers::abi::Token::Array(path_tokens)) = decoded.parameters.get("path") {
            if path_tokens.len() >= 2 {
                if let (ethers::abi::Token::Address(token0), ethers::abi::Token::Address(token1)) = 
                    (&path_tokens[0], &path_tokens[1]) {
                    
                    // 해당 토큰 페어의 풀 찾기
                    for pool in pool_cache.values() {
                        let token0_addr = Address::from_slice(token0.as_bytes());
                        let token1_addr = Address::from_slice(token1.as_bytes());
                        
                        if (pool.token0 == token0_addr && pool.token1 == token1_addr) ||
                           (pool.token0 == token1_addr && pool.token1 == token0_addr) {
                            return Ok(Some(pool.clone()));
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    /// 풀 상태 업데이트
    async fn update_pool_state(&self, pool: &PoolInfo) -> Result<PoolInfo> {
        let h160_address = ethers::types::H160::from_slice(pool.address.as_slice());
        let pool_contract = self.contract_factory.create_amm_pool(h160_address)?;
        let (reserve0, reserve1, _) = pool_contract.get_reserves().await?;
        
        let mut updated_pool = pool.clone();
        updated_pool.reserve0 = U256::from_limbs_slice(&reserve0.0);
        updated_pool.reserve1 = U256::from_limbs_slice(&reserve1.0);
        updated_pool.last_updated = Instant::now();
        
        Ok(updated_pool)
    }
    
    /// 온체인 가격 영향 계산
    async fn calculate_price_impact_onchain(
        &self, 
        decoded: &crate::blockchain::decoder::DecodedTransaction,
        pool: &PoolInfo
    ) -> Result<f64> {
        if let Some(ethers::abi::Token::Uint(amount_in)) = decoded.parameters.get("amountIn") {
            // x * y = k 공식으로 가격 영향 계산
            let amount_in_u256 = U256::from_limbs_slice(&amount_in.0);
            
            // 수수료 적용 (0.3%)
            let amount_in_with_fee = amount_in_u256 * U256::from(997) / U256::from(1000);
            
            let price_before = pool.reserve1.to::<u128>() as f64 / pool.reserve0.to::<u128>() as f64;
            
            // 새로운 리저브 계산
            let new_reserve0 = pool.reserve0 + amount_in_with_fee;
            let new_reserve1 = pool.reserve0 * pool.reserve1 / new_reserve0;
            
            let price_after = new_reserve1.to::<u128>() as f64 / new_reserve0.to::<u128>() as f64;
            
            let price_impact = ((price_before - price_after) / price_before).abs();
            
            return Ok(price_impact);
        }
        
        Ok(0.0)
    }
    
    /// 온체인 최적 샌드위치 크기 계산
    async fn calculate_optimal_sandwich_size_onchain(
        &self,
        decoded: &crate::blockchain::decoder::DecodedTransaction,
        pool: &PoolInfo,
        price_impact: f64
    ) -> Result<U256> {
        if let Some(ethers::abi::Token::Uint(victim_amount)) = decoded.parameters.get("amountIn") {
            let victim_amount_u256 = U256::from_limbs_slice(&victim_amount.0);
            
            // Kelly Criterion 기반 최적 크기 계산
            let optimal_fraction = if price_impact > 0.02 {
                0.3 // 높은 가격 영향시 보수적
            } else {
                0.5 // 낮은 가격 영향시 공격적
            };
            
            let optimal_size = victim_amount_u256 * U256::from((optimal_fraction * 100.0) as u64) / U256::from(100);
            
            // 풀 크기 대비 제한 (5% 이하)
            let pool_limit = pool.reserve0 / U256::from(20);
            
            Ok(std::cmp::min(optimal_size, pool_limit))
        } else {
            Err(anyhow!("스왑 금액을 찾을 수 없습니다"))
        }
    }
    
    /// 온체인 수익 계산
    async fn calculate_sandwich_profit_onchain(
        &self,
        sandwich_size: &U256,
        pool: &PoolInfo,
        price_impact: f64
    ) -> Result<(U256, U256, U256)> {
        // 현재 가스 가격 가져오기
        let (base_fee, priority_fee) = self.blockchain_client.get_gas_price().await?;
        let gas_price = base_fee + priority_fee * ethers::types::U256::from(2); // 2배 priority fee
        
        // 예상 가스 사용량
        let gas_limit = U256::from(300_000 * 2); // 프론트런 + 백런
        let gas_cost = gas_limit * U256::from_limbs_slice(&gas_price.0);
        
        // 예상 수익 계산 (가격 영향 기반)
        let profit_rate = price_impact * 0.7; // 70% 효율
        let expected_profit = *sandwich_size * U256::from((profit_rate * 10000.0) as u64) / U256::from(10000);
        
        let net_profit = if expected_profit > gas_cost {
            expected_profit - gas_cost
        } else {
            U256::ZERO
        };
        
        Ok((expected_profit, gas_cost, net_profit))
    }
    
    /// 온체인 성공 확률 계산
    async fn calculate_success_probability_onchain(
        &self,
        tx: &Transaction,
        net_profit: &U256,
        pool: &PoolInfo
    ) -> Result<f64> {
        let mut score = 0.5;
        
        // 가스 가격 경쟁 요소
        let current_gas = self.blockchain_client.get_gas_price().await?;
        let competition_factor = if tx.gas_price < U256::from_limbs_slice(&current_gas.0.0) * U256::from(2) {
            0.8
        } else {
            0.4
        };
        score *= competition_factor;
        
        // 수익성 요소
        let profitability_factor = if *net_profit > U256::from_str_radix("500000000000000000", 10).unwrap() {
            0.9
        } else {
            0.6
        };
        score *= profitability_factor;
        
        // 풀 유동성 요소
        let total_liquidity = pool.reserve0 + pool.reserve1;
        let liquidity_factor = if total_liquidity > U256::from_str_radix("10000000000000000000000", 10).unwrap() {
            0.9
        } else {
            0.7
        };
        score *= liquidity_factor;
        
        // 네트워크 혼잡도 (현재 블록의 가스 사용률 기반)
        let current_block = self.blockchain_client.get_current_block().await?;
        let network_factor = 0.8; // 실제로는 블록 가스 사용률로 계산
        score *= network_factor;
        
        Ok((score as f64).clamp(0.0, 1.0))
    }
    
    /// 온체인 프론트런 트랜잭션 생성
    async fn create_front_run_transaction_onchain(
        &self,
        amount: &U256,
        pool: &PoolInfo,
        target_gas_price: U256
    ) -> Result<Transaction> {
        let competitive_gas = self.blockchain_client.calculate_competitive_gas_price(0.8).await?;
        let competitive_gas_alloy = U256::from_limbs_slice(&competitive_gas.0);
        let gas_price = std::cmp::min(competitive_gas_alloy, self.max_gas_price);
        
        // Uniswap V2 Router swapExactTokensForTokens 호출
        let mut data = vec![0x38, 0xed, 0x17, 0x39]; // swapExactTokensForTokens selector
        
        // 실제 ABI 인코딩 구현 필요
        data.extend_from_slice(&amount.to_be_bytes::<32>());
        data.extend_from_slice(&U256::ZERO.to_be_bytes::<32>()); // amountOutMin
        // path, to, deadline 등 추가 인코딩 필요
        
        Ok(Transaction {
            hash: B256::ZERO,
            from: alloy::primitives::Address::ZERO, // 실제 지갑 주소
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse()?), // Uniswap V2 Router
            value: *amount,
            gas_price: U256::from_be_bytes(gas_price.to_be_bytes::<32>()),
            gas_limit: U256::from(300_000u64),
            data,
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: Some(self.blockchain_client.get_current_block().await?),
        })
    }
    
    /// 온체인 백런 트랜잭션 생성
    async fn create_back_run_transaction_onchain(
        &self,
        amount: &U256,
        pool: &PoolInfo,
        target_gas_price: U256
    ) -> Result<Transaction> {
        let competitive_gas = self.blockchain_client.calculate_competitive_gas_price(0.7).await?;
        let competitive_gas_alloy = U256::from_limbs_slice(&competitive_gas.0);
        let gas_price = std::cmp::min(competitive_gas_alloy, self.max_gas_price);
        
        let mut data = vec![0x18, 0xcb, 0xaf, 0x05]; // swapExactTokensForETH selector
        
        // 실제 ABI 인코딩 구현 필요
        data.extend_from_slice(&amount.to_be_bytes::<32>());
        data.extend_from_slice(&U256::ZERO.to_be_bytes::<32>()); // amountOutMin
        
        Ok(Transaction {
            hash: B256::ZERO,
            from: alloy::primitives::Address::ZERO,
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse()?),
            value: U256::ZERO,
            gas_price: U256::from_be_bytes(gas_price.to_be_bytes::<32>()),
            gas_limit: U256::from(300_000u64),
            data,
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: Some(self.blockchain_client.get_current_block().await?),
        })
    }
    
    /// 통계 업데이트
    async fn update_stats_onchain(&self, opportunities_found: usize, profit: Option<U256>) {
        let mut stats = self.stats.lock().await;
        stats.transactions_analyzed += 1;
        stats.opportunities_found += opportunities_found as u64;
        stats.last_analysis_time = Some(Instant::now());
        
        if let Some(profit) = profit {
            stats.successful_sandwiches += 1;
            stats.total_profit += profit;
            stats.avg_profit_per_sandwich = stats.total_profit / U256::from(stats.successful_sandwiches);
        }
    }

    /// alloy Transaction을 ethers Transaction으로 변환
    fn convert_to_ethers_transaction(&self, tx: &Transaction) -> Result<ethers::types::Transaction> {
        Ok(ethers::types::Transaction {
            hash: ethers::types::H256::from_slice(tx.hash.as_slice()),
            nonce: ethers::types::U256::from(tx.nonce as u64),
            block_hash: tx.block_number.map(|_| ethers::types::H256::zero()),
            block_number: tx.block_number.map(|n| ethers::types::U64::from(n as u64)),
            transaction_index: None,
            from: ethers::types::H160::from_slice(tx.from.as_slice()),
            to: tx.to.map(|addr| ethers::types::H160::from_slice(addr.as_slice())),
            value: ethers::types::U256::from_little_endian(&tx.value.to_le_bytes::<32>()),
            gas_price: Some(ethers::types::U256::from_little_endian(&tx.gas_price.to_le_bytes::<32>())),
            gas: ethers::types::U256::from_little_endian(&tx.gas_limit.to_le_bytes::<32>()),
            input: ethers::types::Bytes::from(tx.data.clone()),
            v: ethers::types::U64::zero(),
            r: ethers::types::U256::zero(),
            s: ethers::types::U256::zero(),
            chain_id: Some(ethers::types::U256::from(1)),
            transaction_type: None,
            access_list: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            other: ethers::types::OtherFields::default(),
        })
    }
}

#[async_trait]
impl Strategy for OnChainSandwichStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Sandwich
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
    
    async fn start(&self) -> Result<()> {
        self.enabled.store(true, Ordering::SeqCst);
        info!("🚀 온체인 샌드위치 전략 시작됨");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        self.enabled.store(false, Ordering::SeqCst);
        info!("⏹️ 온체인 샌드위치 전략 중지됨");
        Ok(())
    }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(vec![]);
        }
        
        let start_time = Instant::now();
        let mut opportunities = Vec::new();
        
        // 온체인 검증 포함한 샌드위치 대상 확인
        if !self.is_sandwich_target_onchain(transaction).await? {
            return Ok(opportunities);
        }
        
        // 온체인 샌드위치 기회 분석
        if let Some(sandwich_opp) = self.analyze_sandwich_opportunity_onchain(transaction).await? {
            let opportunity = Opportunity::new(
                crate::types::OpportunityType::Sandwich,
                StrategyType::Sandwich,
                sandwich_opp.net_profit,
                sandwich_opp.success_probability,
                600_000, // 프론트런 + 백런 가스 추정값
                0,
                crate::types::OpportunityDetails::Sandwich(crate::types::SandwichDetails {
                    victim_transaction: sandwich_opp.target_tx.clone(),
                    frontrun_amount: sandwich_opp.front_run_tx.value,
                    backrun_amount: sandwich_opp.back_run_tx.value,
                    target_slippage: sandwich_opp.price_impact,
                    pool_address: sandwich_opp.pool.address,
                }),
            );
            
            opportunities.push(opportunity);
        }
        
        // 통계 업데이트
        self.update_stats_onchain(opportunities.len(), None).await;
        
        let duration = start_time.elapsed();
        debug!("🥪🔗 온체인 샌드위치 분석 완료: {:.2}ms, {}개 기회", duration.as_millis(), opportunities.len());
        
        Ok(opportunities)
    }
    
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        if opportunity.strategy != StrategyType::Sandwich {
            return Ok(false);
        }
        
        // 실시간 수익성 재검증
        if opportunity.expected_profit < self.min_profit_eth {
            return Ok(false);
        }
        
        // 현재 가스 가격 검증
        let (base_fee, _) = self.blockchain_client.get_gas_price().await?;
        let base_fee_alloy = U256::from_limbs_slice(&base_fee.0);
        if base_fee_alloy > self.max_gas_price {
            return Ok(false);
        }
        
        // 성공 확률 검증
        if opportunity.confidence < 0.4 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle> {
        let bundle = Bundle::new(
            vec![], // 실제 트랜잭션들로 채워야 함
            0,
            opportunity.expected_profit,
            600_000,
            StrategyType::Sandwich,
        );
        
        Ok(bundle)
    }
}

/// ETH 금액 포맷팅 헬퍼
fn format_eth_amount(wei: U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6} ETH", eth)
}