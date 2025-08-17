use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use tracing::{info, debug, warn, error};
use alloy::primitives::{Address, U256};
use ethers::{
    providers::{Provider, Ws, Middleware},
    types::{H160, U256 as EthersU256},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use rust_decimal::Decimal;

use crate::config::Config;
use crate::types::{Transaction, Opportunity, StrategyType, Bundle};
use crate::strategies::Strategy;
use crate::blockchain::{
    BlockchainClient, ContractFactory, LendingPoolContract, ERC20Contract,
    UserAccountData, ReserveData, TransactionDecoder
};

/// 온체인 데이터 기반 경쟁적 청산 전략
/// 
/// 실제 대출 프로토콜과 연결하여 사용자 건강도를 실시간으로 모니터링하고,
/// 청산 가능한 포지션을 감지하여 경쟁적으로 청산을 실행합니다.
pub struct OnChainLiquidationStrategy {
    config: Arc<Config>,
    blockchain_client: Arc<BlockchainClient>,
    contract_factory: Arc<ContractFactory>,
    tx_decoder: Arc<TransactionDecoder>,
    enabled: Arc<AtomicBool>,
    
    // 대출 프로토콜 정보
    lending_protocols: HashMap<Address, LendingProtocolInfo>,
    
    // 사용자 포지션 캐시
    user_positions: Arc<Mutex<HashMap<Address, Vec<UserPosition>>>>,
    
    // 자산 가격 캐시
    asset_prices: Arc<Mutex<HashMap<Address, AssetPrice>>>,
    
    // 최소 수익성 임계값
    min_profit_eth: U256,
    min_liquidation_amount: U256,
    
    // 가스 가격 전략
    gas_multiplier: f64,
    max_gas_price: U256,
    
    // 청산 조건
    health_factor_threshold: f64,
    max_liquidation_size: U256,
    
    // 통계
    stats: Arc<Mutex<OnChainLiquidationStats>>,
}

#[derive(Debug, Clone)]
struct LendingProtocolInfo {
    name: String,
    protocol_type: ProtocolType,
    lending_pool_address: Address,
    price_oracle_address: Option<Address>,
    liquidation_fee: u32, // basis points
    min_health_factor: f64,
    supported_assets: Vec<Address>,
}

#[derive(Debug, Clone)]
enum ProtocolType {
    Aave,
    Compound,
    MakerDAO,
}

#[derive(Debug, Clone)]
struct UserPosition {
    user: Address,
    protocol: Address,
    collateral_assets: Vec<CollateralPosition>,
    debt_assets: Vec<DebtPosition>,
    health_factor: f64,
    liquidation_threshold: f64,
    total_collateral_usd: f64,
    total_debt_usd: f64,
    last_updated: Instant,
}

#[derive(Debug, Clone)]
struct CollateralPosition {
    asset: Address,
    amount: U256,
    usd_value: f64,
    liquidation_threshold: f64,
}

#[derive(Debug, Clone)]
struct DebtPosition {
    asset: Address,
    amount: U256,
    usd_value: f64,
    borrow_rate: f64,
}

#[derive(Debug, Clone)]
struct AssetPrice {
    asset: Address,
    price_usd: f64,
    price_eth: f64,
    last_updated: Instant,
    source: PriceSource,
}

#[derive(Debug, Clone)]
enum PriceSource {
    Chainlink,
    Uniswap,
    Compound,
    Manual,
}

#[derive(Debug, Clone)]
struct OnChainLiquidationStats {
    protocols_monitored: u64,
    users_monitored: u64,
    transactions_analyzed: u64,
    opportunities_found: u64,
    successful_liquidations: u64,
    total_profit: U256,
    avg_profit_per_liquidation: U256,
    avg_gas_used: U256,
    last_scan_time: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct OnChainLiquidationOpportunity {
    /// 대상 사용자
    pub target_user: Address,
    /// 프로토콜
    pub protocol: LendingProtocolInfo,
    /// 사용자 포지션
    pub position: UserPosition,
    /// 청산할 담보 자산
    pub collateral_asset: Address,
    /// 상환할 부채 자산
    pub debt_asset: Address,
    /// 청산 가능 금액
    pub liquidation_amount: U256,
    /// 받을 담보 금액
    pub collateral_amount: U256,
    /// 청산 보상 (할인)
    pub liquidation_bonus: U256,
    /// 예상 수익
    pub expected_profit: U256,
    /// 가스 비용
    pub gas_cost: U256,
    /// 순수익
    pub net_profit: U256,
    /// 성공 확률
    pub success_probability: f64,
}

impl OnChainLiquidationStrategy {
    /// 새로운 온체인 청산 전략 생성
    pub async fn new(
        config: Arc<Config>, 
        blockchain_client: Arc<BlockchainClient>
    ) -> Result<Self> {
        info!("💸🔗 온체인 청산 전략 초기화 중...");
        
        let contract_factory = Arc::new(ContractFactory::new(blockchain_client.get_provider()));
        let tx_decoder = Arc::new(TransactionDecoder::new()?);
        
        let mut lending_protocols = HashMap::new();
        
        // Aave V2
        lending_protocols.insert(
            "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse()?,
            LendingProtocolInfo {
                name: "Aave V2".to_string(),
                protocol_type: ProtocolType::Aave,
                lending_pool_address: "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse()?,
                price_oracle_address: Some("0xA50ba011c48153De246E5192C8f9258A2ba79Ca9".parse()?),
                liquidation_fee: 500, // 5%
                min_health_factor: 1.0,
                supported_assets: vec![
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                    "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse()?, // USDC
                    "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse()?, // USDT
                    "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse()?, // DAI
                ],
            }
        );
        
        // Compound V3
        lending_protocols.insert(
            "0xc3d688B66703497DAA19211EEdff47fB25365b65".parse()?,
            LendingProtocolInfo {
                name: "Compound V3".to_string(),
                protocol_type: ProtocolType::Compound,
                lending_pool_address: "0xc3d688B66703497DAA19211EEdff47fB25365b65".parse()?,
                price_oracle_address: Some("0x50ce56A3239671Ab62f185704Caedf626352741e".parse()?),
                liquidation_fee: 750, // 7.5%
                min_health_factor: 1.0,
                supported_assets: vec![
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                    "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse()?, // USDC
                ],
            }
        );
        
        let min_profit_eth = U256::from_str_radix(
            &config.strategies.liquidation.min_profit_eth,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("50000000000000000", 10).unwrap());
        
        let min_liquidation_amount = U256::from_str_radix(
            &config.strategies.liquidation.min_liquidation_amount,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("1000000000000000000", 10).unwrap());
        
        info!("✅ 온체인 청산 전략 초기화 완료");
        info!("  📊 프로토콜 수: {}", lending_protocols.len());
        info!("  💰 최소 수익: {} ETH", format_eth_amount(min_profit_eth));
        info!("  💸 최소 청산 금액: {} ETH", format_eth_amount(min_liquidation_amount));
        
        let protocols_count = lending_protocols.len() as u64;
        
        let strategy = Self {
            config,
            blockchain_client,
            contract_factory,
            tx_decoder,
            enabled: Arc::new(AtomicBool::new(true)),
            lending_protocols,
            user_positions: Arc::new(Mutex::new(HashMap::new())),
            asset_prices: Arc::new(Mutex::new(HashMap::new())),
            min_profit_eth,
            min_liquidation_amount,
            gas_multiplier: 1.5,
            max_gas_price: U256::from(200_000_000_000u64) * U256::from(1_000_000_000u64),
            health_factor_threshold: 1.0,
            max_liquidation_size: U256::from_str_radix("10000000000000000000", 10).unwrap(),
            stats: Arc::new(Mutex::new(OnChainLiquidationStats {
                protocols_monitored: protocols_count,
                users_monitored: 0,
                transactions_analyzed: 0,
                opportunities_found: 0,
                successful_liquidations: 0,
                total_profit: U256::ZERO,
                avg_profit_per_liquidation: U256::ZERO,
                avg_gas_used: U256::ZERO,
                last_scan_time: None,
            })),
        };
        
        // 자산 가격 초기화
        strategy.initialize_asset_prices().await?;
        
        Ok(strategy)
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
    
    /// 자산 가격 초기화
    async fn initialize_asset_prices(&self) -> Result<()> {
        info!("💱 자산 가격 초기화 중...");
        
        let mut prices = self.asset_prices.lock().await;
        
        // 주요 자산들의 가격 설정 (실제로는 오라클에서 가져와야 함)
        let assets = vec![
            ("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, 2800.0), // WETH
            ("0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse()?, 1.0),    // USDC
            ("0xdAC17F958D2ee523a2206206994597C13D831ec7".parse()?, 1.0),    // USDT
            ("0x6B175474E89094C44Da98b954EedeAC495271d0F".parse()?, 1.0),    // DAI
        ];
        
        for (asset, price_usd) in assets {
            prices.insert(asset, AssetPrice {
                asset,
                price_usd,
                price_eth: price_usd / 2800.0,
                last_updated: Instant::now(),
                source: PriceSource::Manual,
            });
        }
        
        info!("✅ {} 개 자산 가격 초기화 완료", prices.len());
        Ok(())
    }
    
    /// 청산 가능한 포지션 스캔
    pub async fn scan_liquidatable_positions(&self) -> Result<Vec<OnChainLiquidationOpportunity>> {
        debug!("🔍 청산 가능 포지션 스캔 시작");
        
        let mut opportunities = Vec::new();
        
        // 각 프로토콜에서 청산 가능한 포지션 탐색
        for protocol in self.lending_protocols.values() {
            match self.scan_protocol_positions(protocol).await {
                Ok(mut protocol_opportunities) => {
                    opportunities.append(&mut protocol_opportunities);
                }
                Err(e) => {
                    warn!("프로토콜 {} 스캔 실패: {}", protocol.name, e);
                }
            }
        }
        
        // 수익성 순으로 정렬
        opportunities.sort_by(|a, b| b.net_profit.cmp(&a.net_profit));
        
        // 상위 10개만 반환
        opportunities.truncate(10);
        
        info!("🎯 청산 기회 발견: {} 개", opportunities.len());
        
        // 통계 업데이트
        let mut stats = self.stats.lock().await;
        stats.opportunities_found += opportunities.len() as u64;
        stats.last_scan_time = Some(Instant::now());
        
        Ok(opportunities)
    }
    
    /// 특정 프로토콜의 포지션 스캔
    async fn scan_protocol_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let mut opportunities = Vec::new();
        
        match protocol.protocol_type {
            ProtocolType::Aave => {
                opportunities.extend(self.scan_aave_positions(protocol).await?);
            }
            ProtocolType::Compound => {
                opportunities.extend(self.scan_compound_positions(protocol).await?);
            }
            ProtocolType::MakerDAO => {
                // MakerDAO 구현 필요
                debug!("MakerDAO 스캔은 아직 구현되지 않았습니다");
            }
        }
        
        Ok(opportunities)
    }
    
    /// Aave 포지션 스캔
    async fn scan_aave_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let h160_address = ethers::types::H160::from_slice(protocol.lending_pool_address.as_slice());
        let lending_pool = self.contract_factory.create_lending_pool(h160_address)?;
        let mut opportunities = Vec::new();
        
        // 알려진 고위험 사용자들 (실제로는 이벤트 로그나 서브그래프에서 가져와야 함)
        let high_risk_users = self.get_high_risk_users(protocol).await?;
        
        for user in high_risk_users {
            match self.analyze_user_position_aave(&lending_pool, user, protocol).await {
                Ok(Some(opportunity)) => {
                    opportunities.push(opportunity);
                }
                Ok(None) => {
                    debug!("사용자 {} - 청산 기회 없음", user);
                }
                Err(e) => {
                    warn!("사용자 {} 분석 실패: {}", user, e);
                }
            }
        }
        
        Ok(opportunities)
    }
    
    /// Compound 포지션 스캔
    async fn scan_compound_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        // Compound 구현 (Aave와 유사하지만 다른 API)
        debug!("Compound 포지션 스캔 구현 필요");
        Ok(vec![])
    }
    
    /// 고위험 사용자 목록 가져오기
    async fn get_high_risk_users(&self, protocol: &LendingProtocolInfo) -> Result<Vec<Address>> {
        // 실제로는 다음 방법으로 가져와야 함:
        // 1. 이벤트 로그에서 최근 거래한 사용자들
        // 2. 서브그래프 API
        // 3. 오프체인 모니터링 시스템
        
        // 임시로 알려진 테스트 주소들 반환
        Ok(vec![
            "0x742d35Cc6570000000000000000000000000001".parse()?,
            "0x742d35Cc6570000000000000000000000000002".parse()?,
            "0x742d35Cc6570000000000000000000000000003".parse()?,
        ])
    }
    
    /// Aave 사용자 포지션 분석
    async fn analyze_user_position_aave(
        &self,
        lending_pool: &Arc<LendingPoolContract>,
        user: Address,
        protocol: &LendingProtocolInfo
    ) -> Result<Option<OnChainLiquidationOpportunity>> {
        // 사용자 계정 데이터 가져오기
        let user_h160 = H160::from_slice(user.as_slice());
        let account_data = lending_pool.get_user_account_data(user_h160).await?;
        
        // 건강도 계산
        let health_factor = if account_data.health_factor == EthersU256::MAX {
            f64::INFINITY
        } else {
            account_data.health_factor.as_u128() as f64 / 1e18
        };
        
        // 청산 가능한지 확인
        if health_factor >= protocol.min_health_factor {
            return Ok(None); // 건강한 포지션
        }
        
        // 청산 가능한 자산 쌍 찾기
        let liquidation_details = self.find_best_liquidation_pair(user, &account_data, protocol).await?;
        
        if let Some((collateral_asset, debt_asset, liquidation_amount)) = liquidation_details {
            // 수익성 계산
            let (expected_profit, gas_cost, net_profit) = self.calculate_liquidation_profit_onchain(
                liquidation_amount,
                collateral_asset,
                debt_asset,
                protocol
            ).await?;
            
            // 최소 수익성 검증
            if net_profit < self.min_profit_eth {
                return Ok(None);
            }
            
            // 성공 확률 계산
            let success_probability = self.calculate_liquidation_success_probability_onchain(
                user,
                health_factor,
                net_profit
            ).await?;
            
            if success_probability < 0.3 {
                return Ok(None);
            }
            
            // 담보 받을 수량 계산
            let collateral_amount = self.calculate_collateral_amount(
                liquidation_amount,
                collateral_asset,
                debt_asset,
                protocol
            ).await?;
            
            // 청산 보상 계산
            let liquidation_bonus = collateral_amount * U256::from(protocol.liquidation_fee) / U256::from(10000);
            
            info!("💸 청산 기회 발견!");
            info!("  👤 사용자: {}", user);
            info!("  🏥 건강도: {:.3}", health_factor);
            info!("  💰 청산 금액: {} ETH", format_eth_amount(liquidation_amount));
            info!("  📊 예상 수익: {} ETH", format_eth_amount(net_profit));
            info!("  🎲 성공 확률: {:.1}%", success_probability * 100.0);
            
            // 더미 포지션 생성 (실제로는 온체인에서 가져와야 함)
            let position = UserPosition {
                user,
                protocol: protocol.lending_pool_address,
                collateral_assets: vec![],
                debt_assets: vec![],
                health_factor,
                liquidation_threshold: 0.8,
                total_collateral_usd: account_data.total_collateral_eth.as_u128() as f64 / 1e18 * 2800.0,
                total_debt_usd: account_data.total_debt_eth.as_u128() as f64 / 1e18 * 2800.0,
                last_updated: Instant::now(),
            };
            
            return Ok(Some(OnChainLiquidationOpportunity {
                target_user: user,
                protocol: protocol.clone(),
                position,
                collateral_asset,
                debt_asset,
                liquidation_amount,
                collateral_amount,
                liquidation_bonus,
                expected_profit,
                gas_cost,
                net_profit,
                success_probability,
            }));
        }
        
        Ok(None)
    }
    
    /// 최적 청산 자산 쌍 찾기
    async fn find_best_liquidation_pair(
        &self,
        user: Address,
        account_data: &UserAccountData,
        protocol: &LendingProtocolInfo
    ) -> Result<Option<(Address, Address, U256)>> {
        // 간단한 구현 - 실제로는 모든 담보/부채 자산을 분석해야 함
        
        let weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?;
        let usdc_address = "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse()?;
        
        // 최대 50% 청산 가능
        let max_liquidation = account_data.total_debt_eth / EthersU256::from(2);
        let liquidation_amount = U256::from_limbs_slice(&max_liquidation.0);
        
        Ok(Some((weth_address, usdc_address, liquidation_amount)))
    }
    
    /// 온체인 청산 수익 계산
    async fn calculate_liquidation_profit_onchain(
        &self,
        liquidation_amount: U256,
        collateral_asset: Address,
        debt_asset: Address,
        protocol: &LendingProtocolInfo
    ) -> Result<(U256, U256, U256)> {
        // 현재 가스 가격 가져오기
        let (base_fee, priority_fee) = self.blockchain_client.get_gas_price().await?;
        let gas_price_ethers = base_fee + priority_fee * ethers::types::U256::from(2);
        let gas_price = U256::from_limbs_slice(&gas_price_ethers.0);
        
        // 청산 가스 사용량 (복잡한 작업)
        let gas_limit = U256::from(800_000);
        let gas_cost = gas_limit * gas_price;
        
        // 청산 보상 계산
        let liquidation_fee_bps = protocol.liquidation_fee as f64 / 10000.0;
        let expected_profit = liquidation_amount * U256::from((liquidation_fee_bps * 10000.0) as u64) / U256::from(10000);
        
        let net_profit = if expected_profit > gas_cost {
            expected_profit - gas_cost
        } else {
            U256::ZERO
        };
        
        Ok((expected_profit, gas_cost, net_profit))
    }
    
    /// 온체인 청산 성공 확률 계산
    async fn calculate_liquidation_success_probability_onchain(
        &self,
        user: Address,
        health_factor: f64,
        net_profit: U256
    ) -> Result<f64> {
        let mut score = 0.6; // 기본 점수
        
        // 건강도 기반 점수 (낮을수록 긴급)
        if health_factor < 0.9 {
            score += 0.3; // 매우 위험
        } else if health_factor < 0.95 {
            score += 0.2; // 위험
        } else {
            score += 0.1; // 경계선
        }
        
        // 수익성 기반 점수
        if net_profit > U256::from_str_radix("1000000000000000000", 10).unwrap() {
            score += 0.2; // 1 ETH 이상
        } else if net_profit > U256::from_str_radix("500000000000000000", 10).unwrap() {
            score += 0.1; // 0.5 ETH 이상
        }
        
        // 현재 가스 가격 (낮을수록 경쟁 낮음)
        let (base_fee, _) = self.blockchain_client.get_gas_price().await?;
        let base_fee_alloy = U256::from_limbs_slice(&base_fee.0);
        if base_fee_alloy < U256::from(50_000_000_000u64) {
            score += 0.1;
        }
        
        // 네트워크 혼잡도
        let network_factor = 0.8; // 실제로는 멤풀 상태 확인
        score *= network_factor;
        
        Ok((score as f64).clamp(0.0, 1.0))
    }
    
    /// 담보 수량 계산
    async fn calculate_collateral_amount(
        &self,
        liquidation_amount: U256,
        collateral_asset: Address,
        debt_asset: Address,
        protocol: &LendingProtocolInfo
    ) -> Result<U256> {
        let prices = self.asset_prices.lock().await;
        
        let debt_price = prices.get(&debt_asset)
            .map(|p| p.price_usd)
            .unwrap_or(1.0);
        
        let collateral_price = prices.get(&collateral_asset)
            .map(|p| p.price_usd)
            .unwrap_or(2800.0);
        
        // 청산 보너스 포함
        let bonus_multiplier = 1.0 + (protocol.liquidation_fee as f64 / 10000.0);
        let collateral_amount_usd = liquidation_amount.to::<u128>() as f64 / 1e18 * debt_price * bonus_multiplier;
        let collateral_amount = (collateral_amount_usd / collateral_price * 1e18) as u128;
        
        Ok(U256::from(collateral_amount))
    }
    
    /// 통계 업데이트
    async fn update_stats_onchain(&self, opportunities_found: usize, profit: Option<U256>) {
        let mut stats = self.stats.lock().await;
        stats.transactions_analyzed += 1;
        stats.opportunities_found += opportunities_found as u64;
        
        if let Some(profit) = profit {
            stats.successful_liquidations += 1;
            stats.total_profit += profit;
            stats.avg_profit_per_liquidation = stats.total_profit / U256::from(stats.successful_liquidations);
        }
    }
}

#[async_trait]
impl Strategy for OnChainLiquidationStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Liquidation
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
    
    async fn start(&self) -> Result<()> {
        self.enabled.store(true, Ordering::SeqCst);
        info!("🚀 온체인 청산 전략 시작됨");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        self.enabled.store(false, Ordering::SeqCst);
        info!("⏹️ 온체인 청산 전략 중지됨");
        Ok(())
    }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(vec![]);
        }
        
        let start_time = Instant::now();
        let mut opportunities = Vec::new();
        
        // 트랜잭션이 청산 관련인지 확인
        let ethers_tx = self.convert_to_ethers_transaction(transaction)?;
        let decoded = self.tx_decoder.decode_transaction(&ethers_tx)?;
        
        if decoded.is_liquidation_opportunity() {
            // 실시간 청산 기회 스캔
            let liquidation_opportunities = self.scan_liquidatable_positions().await?;
            
            // Opportunity 타입으로 변환
            for liq_opp in liquidation_opportunities.into_iter().take(3) { // 최대 3개
                let opportunity = Opportunity::new(
                    crate::types::OpportunityType::Liquidation,
                    StrategyType::Liquidation,
                    liq_opp.net_profit,
                    liq_opp.success_probability,
                    800_000, // 청산 가스 추정값
                    0,
                    crate::types::OpportunityDetails::Liquidation(crate::types::LiquidationDetails {
                        protocol: liq_opp.protocol.name.clone(),
                        user: liq_opp.target_user,
                        collateral_asset: liq_opp.collateral_asset,
                        debt_asset: liq_opp.debt_asset,
                        collateral_amount: liq_opp.collateral_amount,
                        debt_amount: liq_opp.liquidation_amount,
                        health_factor: Decimal::from_f64_retain(liq_opp.position.health_factor).unwrap_or_default(),
                    }),
                );
                
                opportunities.push(opportunity);
            }
        }
        
        // 통계 업데이트
        self.update_stats_onchain(opportunities.len(), None).await;
        
        let duration = start_time.elapsed();
        debug!("💸🔗 온체인 청산 분석 완료: {:.2}ms, {}개 기회", duration.as_millis(), opportunities.len());
        
        Ok(opportunities)
    }
    
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        if opportunity.strategy != StrategyType::Liquidation {
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
        if opportunity.confidence < 0.3 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle> {
        let bundle = Bundle::new(
            vec![], // 실제 청산 트랜잭션들로 채워야 함
            0,
            opportunity.expected_profit,
            800_000,
            StrategyType::Liquidation,
        );
        
        Ok(bundle)
    }

}

/// ETH 금액 포맷팅 헬퍼
fn format_eth_amount(wei: U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6} ETH", eth)
}