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
use std::str::FromStr;

use crate::config::Config;
use crate::types::{Transaction, Opportunity, StrategyType, Bundle};
use crate::utils::abi::ABICodec;
use serde::Deserialize;
use crate::storage::{Storage, UserPositionRecord, PriceHistoryRecord, LiquidationEvent};
use crate::strategies::Strategy;
use crate::flashbots::FlashbotsClient;
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
    storage: Arc<Storage>,
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

        // MakerDAO (Dog/Vat)
        // Use Dog as "lending_pool_address" for bark, but read via Vat for positions
        lending_protocols.insert(
            "0x135954d155898D42C90D2a57824C690e0c7BEf1B".parse()?, // Dog
            LendingProtocolInfo {
                name: "MakerDAO".to_string(),
                protocol_type: ProtocolType::MakerDAO,
                lending_pool_address: "0x135954d155898D42C90D2a57824C690e0c7BEf1B".parse()?, // Dog
                price_oracle_address: Some("0x35D1b3F3D7966A1DFe207aa4514C12a259A0492B".parse()?), // Vat (as data source)
                liquidation_fee: 1300, // illustrative bps, varies by ilk
                min_health_factor: 1.0,
                supported_assets: vec![
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                    "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse()?, // DAI
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
        
        let storage = Arc::new(Storage::new(std::env::var("REDIS_URL").ok().as_deref()).await?);

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
            storage,
        };
        
        // 자산 가격 초기화
        strategy.initialize_asset_prices().await?;
        
        Ok(strategy)
    }

    /// Create and submit a Flashbots bundle for a validated liquidation opportunity
    pub async fn submit_bundle_for_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        // 1) 번들 생성
        let bundle = self.create_bundle(opportunity).await?;
        // 빈 번들이면 제출 스킵
        if bundle.transactions.is_empty() {
            tracing::warn!("Liquidation bundle is empty; skipping submission");
            return Ok(false);
        }

        // 2) Flashbots 클라이언트 초기화 및 제출
        let client = FlashbotsClient::new(Arc::clone(&self.config)).await?;
        let result = client.submit_bundle(&bundle).await?;
        Ok(result)
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

        // 가격 히스토리 저장 (상위 기회 관련 자산만)
        if let Some(top) = opportunities.first() {
            let prices = self.asset_prices.lock().await;
            if let Some(p) = prices.get(&top.collateral_asset) {
                let _ = self.storage.save_price_history(&PriceHistoryRecord {
                    token: p.asset,
                    price_usd: p.price_usd,
                    price_eth: p.price_eth,
                    timestamp: chrono::Utc::now(),
                }).await;
            }
        }
        
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
                opportunities.extend(self.scan_maker_positions(protocol).await?);
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
        // 간단한 포지션 스캔: 알려진 고위험 주소 재사용
        let mut opportunities = Vec::new();
        let users = self.get_high_risk_users(protocol).await?;
        for user in users {
            if let Some(opp) = self.analyze_user_position_compound(user, protocol).await? {
                opportunities.push(opp);
            }
        }
        Ok(opportunities)
    }

    /// Compound 사용자 포지션 분석 (간단 버전)
    async fn analyze_user_position_compound(
        &self,
        user: Address,
        protocol: &LendingProtocolInfo
    ) -> Result<Option<OnChainLiquidationOpportunity>> {
        // 실제 Compound V3(Comet) 데이터를 조회하여 파라미터 산출
        let comet = self
            .contract_factory
            .create_comet(H160::from_slice(protocol.lending_pool_address.as_slice()))?;

        // 사용자 부채(기초자산) 조회
        let borrow_base_ethers = comet
            .borrow_balance_of(H160::from_slice(user.as_slice()))
            .await
            .unwrap_or_else(|_| ethers::types::U256::zero());
        if borrow_base_ethers.is_zero() { return Ok(None); }
        let borrow_base = U256::from_str_radix(&borrow_base_ethers.to_string(), 10)
            .unwrap_or(U256::ZERO);

        // 청산 가능한 금액 산출: min(부채, 최대/최소 한도)
        let mut liquidation_amount = borrow_base;
        if liquidation_amount > self.max_liquidation_size { liquidation_amount = self.max_liquidation_size; }
        if liquidation_amount < self.min_liquidation_amount { liquidation_amount = self.min_liquidation_amount; }

        // 기초자산(부채) 자산: 설정에서 우선 조회(USDC), 실패 시 기본값
        let debt_asset: Address = if let Some(h160) = self.config.get_token_address("USDC") {
            Address::from_slice(h160.as_bytes())
        } else {
            "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse()?
        };

        // 담보 자산 후보 선택: supported_assets 중 동일 상환액 대비 가장 큰 담보 수령량을 주는 자산 선택
        let mut best_collateral: Option<(Address, U256)> = None;
        for asset in protocol.supported_assets.iter() {
            let quoted_e = comet
                .quote_collateral(
                    H160::from_slice(asset.as_slice()),
                    EthersU256::from_dec_str(&liquidation_amount.to_string()).unwrap_or_else(|_| EthersU256::zero()),
                )
                .await
                .unwrap_or_default();
            let amt = U256::from_str_radix(&quoted_e.to_string(), 10).unwrap_or(U256::ZERO);
            match best_collateral {
                Some((_a, best_amt)) if best_amt >= amt => {},
                _ => { best_collateral = Some((*asset, amt)); }
            }
        }
        let (collateral_asset, collateral_amount_est) = best_collateral
            .unwrap_or_else(|| (protocol.supported_assets.get(0).copied().unwrap_or_else(|| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap()), U256::ZERO));

        // 담보로 받게 될 수량 견적 (선택된 담보 기준)
        let collateral_amount = collateral_amount_est;

        // 수익/가스/순익 계산
        let (expected_profit, gas_cost, net_profit) = self
            .calculate_liquidation_profit_onchain(liquidation_amount, collateral_asset, debt_asset, protocol)
            .await?;
        if net_profit < self.min_profit_eth { return Ok(None); }

        // 성공 확률(간단) — 부채비중과 네트워크 상태를 반영하여 조정할 수 있음
        let success_probability = self
            .calculate_liquidation_success_probability_onchain(user, 0.93, net_profit)
            .await
            .unwrap_or(0.5);

        let liquidation_bonus = collateral_amount * U256::from(protocol.liquidation_fee) / U256::from(10000);

        let position = UserPosition {
            user,
            protocol: protocol.lending_pool_address,
            collateral_assets: vec![],
            debt_assets: vec![],
            health_factor: 0.93,
            liquidation_threshold: 0.85,
            total_collateral_usd: 0.0,
            total_debt_usd: 0.0,
            last_updated: Instant::now(),
        };

        Ok(Some(OnChainLiquidationOpportunity {
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
        }))
    }

    /// MakerDAO 포지션 스캔 (간단 버전)
    async fn scan_maker_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let mut opportunities = Vec::new();
        let users = self.get_high_risk_users(protocol).await?;
        for user in users {
            if let Some(opp) = self.analyze_user_position_maker(user, protocol).await? {
                opportunities.push(opp);
            }
        }
        Ok(opportunities)
    }

    /// Maker 사용자 포지션 분석 (실데이터 기반, 다중 ilk 스캔 간단 버전)
    async fn analyze_user_position_maker(
        &self,
        user: Address,
        protocol: &LendingProtocolInfo
    ) -> Result<Option<OnChainLiquidationOpportunity>> {
        // Vat 주소를 price_oracle_address에 저장해두었음
        let vat_addr = protocol.price_oracle_address.unwrap_or_else(|| "0x35D1b3F3D7966A1DFe207aa4514C12a259A0492B".parse().unwrap());
        let vat = self.contract_factory.create_vat(H160::from_slice(vat_addr.as_slice()))?;

        // 스캔할 대표 ilk 목록과 담보 토큰 주소 매핑 (설정 우선, 기본값 폴백)
        let weth_address: Address = if let Some(h) = self.config.get_token_address("WETH") { Address::from_slice(h.as_bytes()) } else { "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()? };
        let wbtc_address: Address = if let Some(h) = self.config.get_token_address("WBTC") { Address::from_slice(h.as_bytes()) } else { "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse()? };
        let dai_address: Address = if let Some(h) = self.config.get_token_address("DAI") { Address::from_slice(h.as_bytes()) } else { "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse()? };
        let candidates: Vec<(&[u8], Address)> = vec![
            (b"ETH-A" as &[u8], weth_address),
            (b"ETH-B" as &[u8], weth_address),
            (b"ETH-C" as &[u8], weth_address),
            (b"WBTC-A" as &[u8], wbtc_address),
        ];

        let mut selected: Option<([u8;32], Address, U256, U256, U256)> = None; // (ilk, collToken, debt_wad, collateral_value_wad, health_factor_scaled_1e18)

        for (tag, coll_token) in candidates.iter() {
            let mut ilk_bytes = [0u8; 32];
            ilk_bytes[..tag.len()].copy_from_slice(tag);

            // urns(ilk, urn): (ink collateral, art normalized debt)
            let (ink_e, art_e) = vat.urns(ilk_bytes, H160::from_slice(user.as_slice())).await.unwrap_or((ethers::types::U256::zero(), ethers::types::U256::zero()));
            if art_e.is_zero() { continue; }
            let ink = U256::from_str_radix(&ink_e.to_string(), 10).unwrap_or(U256::ZERO);
            let art = U256::from_str_radix(&art_e.to_string(), 10).unwrap_or(U256::ZERO);
            if art.is_zero() { continue; }

            // ilks(ilk): (..., rate, spot, ...)
            let (_Art_e, rate_e, spot_e, _line_e, _dust_e) = vat.ilks(ilk_bytes).await.unwrap_or((ethers::types::U256::zero(), ethers::types::U256::from(1u64), ethers::types::U256::zero(), ethers::types::U256::zero(), ethers::types::U256::zero()));
            let rate = U256::from_str_radix(&rate_e.to_string(), 10).unwrap_or(U256::from(1u64));
            let spot = U256::from_str_radix(&spot_e.to_string(), 10).unwrap_or(U256::ZERO);

            // 실제 부채 = art * rate / RAY, 담보 한도 = ink * spot
            let ray = U256::from_str_radix("1000000000000000000000000000", 10).unwrap(); // 1e27
            let debt_rad = art * rate;
            let debt_wad = debt_rad / ray;
            let collateral_value_wad = (ink * spot) / ray;

            // 건강도 = collateral_value / debt (wad 단위 비율)
            let health_factor = if debt_wad.is_zero() { f64::INFINITY } else {
                let coll = f64::from_str(&collateral_value_wad.to_string()).unwrap_or(0.0);
                let deb = f64::from_str(&debt_wad.to_string()).unwrap_or(1.0);
                coll / deb
            };

            if health_factor < protocol.min_health_factor {
                selected = Some((ilk_bytes, *coll_token, debt_wad, collateral_value_wad, U256::from( (health_factor * 1e18) as u128 )));
                break;
            }
        }

        let (selected_ilk, collateral_token, debt_wad, _collateral_value_wad, _health_factor_scaled) = match selected {
            Some(v) => v,
            None => return Ok(None),
        };

        // 청산 금액은 최소/최대 범위 내에서 부채의 일부로 설정
        let mut liquidation_amount = debt_wad;
        if liquidation_amount > self.max_liquidation_size { liquidation_amount = self.max_liquidation_size; }
        if liquidation_amount < self.min_liquidation_amount { liquidation_amount = self.min_liquidation_amount; }

        // 건강도 재계산 (간단 비율)
        let health_factor = if debt_wad.is_zero() { f64::INFINITY } else {
            // 대략적 비율로 충분 (이미 위에서 계산됨)
            // 안전하게 0.9로 클램프
            0.9f64
        };

        // 수익/가스/순익 계산 (담보는 collateral_token, 부채는 DAI)
        let (expected_profit, gas_cost, net_profit) = self
            .calculate_liquidation_profit_onchain(liquidation_amount, collateral_token, dai_address, protocol)
            .await?;
        if net_profit < self.min_profit_eth { return Ok(None); }

        let success_probability = self
            .calculate_liquidation_success_probability_onchain(user, health_factor, net_profit)
            .await
            .unwrap_or(0.4);

        // 담보 수령량 근사
        let collateral_amount = self
            .calculate_collateral_amount(liquidation_amount, collateral_token, dai_address, protocol)
            .await?;
        let liquidation_bonus = collateral_amount * U256::from(protocol.liquidation_fee) / U256::from(10000);

        let position = UserPosition {
            user,
            protocol: protocol.lending_pool_address,
            collateral_assets: vec![],
            debt_assets: vec![],
            health_factor,
            liquidation_threshold: 0.8,
            total_collateral_usd: 0.0,
            total_debt_usd: 0.0,
            last_updated: Instant::now(),
        };

        Ok(Some(OnChainLiquidationOpportunity {
            target_user: user,
            protocol: protocol.clone(),
            position,
            collateral_asset: collateral_token,
            debt_asset: dai_address,
            liquidation_amount,
            collateral_amount,
            liquidation_bonus,
            expected_profit,
            gas_cost,
            net_profit,
            success_probability,
        }))
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

            // 포지션 스냅샷 저장 (비차단)
            let _ = self.storage.save_user_position(&UserPositionRecord {
                user,
                protocol: protocol.lending_pool_address,
                health_factor,
                total_collateral_usd: position.total_collateral_usd,
                total_debt_usd: position.total_debt_usd,
                timestamp: chrono::Utc::now(),
            }).await;
            
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
        // 현재 가스 가격 + 경쟁도/타이밍 기반 우선수수료 가중
        let (base_fee, priority_fee) = self.blockchain_client.get_gas_price().await?;
        let urgency = self.predict_liquidation_urgency(collateral_asset, debt_asset).await.unwrap_or(0.2);
        let competition = self.estimate_competition_intensity().await.unwrap_or(0.5);
        let aggressiveness = (urgency * 0.6 + competition * 0.4).clamp(0.0, 1.0);
        let bump_gwei = ((1.0 + aggressiveness) * 2.0).round() as u64; // 2~4 gwei 가산
        let adj_priority = priority_fee + ethers::types::U256::from(bump_gwei);
        let gas_price_ethers = base_fee + adj_priority * ethers::types::U256::from(2);
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

    /// 멤풀 기반 경쟁 강도 추정 (0~1)
    async fn estimate_competition_intensity(&self) -> Result<f64> {
        // 간단히 대기중 트랜잭션 수로 근사
        let pending = self.blockchain_client.get_pending_transactions().await.unwrap_or_default();
        let n = pending.len() as f64;
        let intensity = (n / 200_000.0).clamp(0.0, 1.0); // 대략적 스케일링
        Ok(intensity)
    }

    /// 가격 히스토리로 타이밍 긴급도 추정 (0~1)
    async fn predict_liquidation_urgency(&self, collateral: Address, _debt: Address) -> Result<f64> {
        let history = self.storage.get_recent_price_history(collateral, 60).await.unwrap_or_default();
        if history.len() < 5 { return Ok(0.2); }
        let mut returns = Vec::new();
        for w in history.windows(2) {
            let p0 = w[0].price_usd.max(1e-9);
            let p1 = w[1].price_usd.max(1e-9);
            returns.push((p1 / p0 - 1.0).abs());
        }
        let vol = returns.iter().copied().sum::<f64>() / returns.len().max(1) as f64;
        Ok(vol.min(0.5) * 2.0) // normalize to 0~1
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

    /// 결과 기록 및 라우팅: 이벤트 저장, 실패 사유 기록, 자산 처리 등
    async fn record_liquidation_outcome(
        &self,
        protocol_name: &str,
        user: Address,
        collateral_asset: Address,
        debt_asset: Address,
        debt_repaid: U256,
        collateral_received: U256,
        expected_profit: U256,
        gas_cost: U256,
        net_profit: U256,
        success: bool,
        reason: Option<String>,
        block_number: Option<u64>,
    ) {
        let _ = self.storage.save_liquidation_event(&LiquidationEvent {
            protocol: protocol_name.to_string(),
            user,
            collateral_asset,
            debt_asset,
            debt_repaid,
            collateral_received,
            expected_profit,
            gas_cost,
            net_profit,
            success,
            reason,
            block_number,
            timestamp: chrono::Utc::now(),
        }).await;
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
        // LiquidationDetails에서 필요한 정보 추출
        let (protocol_name, user, collateral_asset, debt_asset, debt_amount) = match &opportunity.details {
            crate::types::OpportunityDetails::Liquidation(d) => (
                d.protocol.clone(), d.user, d.collateral_asset, d.debt_asset, d.debt_amount
            ),
            _ => {
                return Ok(Bundle::new(vec![], 0, opportunity.expected_profit, 800_000, StrategyType::Liquidation));
            }
        };

        // 프로토콜 주소 탐색
        let protocol_info = self.find_protocol_by_name(&protocol_name);
        if protocol_info.is_none() {
            return Ok(Bundle::new(vec![], 0, opportunity.expected_profit, 800_000, StrategyType::Liquidation));
        }
        let protocol_info = protocol_info.unwrap();

        // 프로토콜별 liquidation calldata 생성
        let abi = ABICodec::new();
        let (to_addr, data_bytes) = match protocol_info.protocol_type {
            ProtocolType::Aave => {
                let calldata = abi.encode_aave_liquidation(
                    collateral_asset,
                    debt_asset,
                    user,
                    debt_amount,
                    false,
                )?;
                (protocol_info.lending_pool_address, calldata)
            }
            ProtocolType::Compound => {
                let calldata = abi.encode_compound_liquidation(
                    user,
                    collateral_asset,
                    debt_amount,
                )?;
                (protocol_info.lending_pool_address, calldata)
            }
            ProtocolType::MakerDAO => {
                // 간단화: ETH-A ilk 고정 (예시용)
                let ilk_eth_a: [u8; 32] = {
                    let mut b = [0u8; 32];
                    let tag = b"ETH-A";
                    b[..tag.len()].copy_from_slice(tag);
                    b
                };
                let keeper = "0x0000000000000000000000000000000000000001".parse()?;
                let calldata = abi.encode_maker_bark(ilk_eth_a, user, keeper)?;
                (protocol_info.lending_pool_address, calldata)
            }
        };

        // 플래시론 사용 여부에 따라 3-스텝 번들 구성 (flashLoan -> liquidation -> repay)
        // 기본 트랜잭션 구성 (표시 목적)
        let tx = Transaction {
            hash: alloy::primitives::B256::ZERO,
            from: alloy::primitives::Address::ZERO,
            to: Some(to_addr),
            value: U256::ZERO,
            gas_price: U256::from(30_000_000_000u64),
            gas_limit: U256::from(800_000u64),
            data: data_bytes.to_vec(),
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: None,
        };

        // 판매 경로 견적 준비 (0x 우선, 실패 시 1inch)
        let abi = ABICodec::new();
        let mut sell_target: Option<Address> = None;
        let mut sell_calldata: Option<alloy::primitives::Bytes> = None;
        let mut sell_spender: Option<Address> = None;
        if let Ok(Some(quote)) = self.try_get_0x_quote(collateral_asset, debt_asset, debt_amount).await {
            sell_target = Some(quote.to);
            sell_calldata = Some(quote.data.clone());
            sell_spender = quote.allowance_target;
        } else if let Ok(Some(quote)) = self.try_get_1inch_quote(collateral_asset, debt_asset, debt_amount).await {
            sell_target = Some(quote.to);
            sell_calldata = Some(quote.data.clone());
            sell_spender = None; // 1inch는 allowanceTarget을 제공하지 않거나 라우터 자체가 spender
        }

        // 플래시론 수신자 설정 시: 3-스텝을 수신자 내부에서 처리하도록 단일 flashLoan 트랜잭션만 번들에 포함하고 조기 반환
        if let Some(receiver_h160) = self.config.blockchain.primary_network.flashloan_receiver {
            if receiver_h160 != H160::zero() {
                let receiver_addr = Address::from_slice(receiver_h160.as_bytes());
                // 플래시론 수수료(9bps)만큼 상환에 필요한 최소 아웃 계산
                let min_out = {
                    let fee = debt_amount * U256::from(9u64) / U256::from(10000u64);
                    debt_amount + fee
                };
                let params = abi.encode_flashloan_receiver_liquidation_params(
                    to_addr,
                    alloy::primitives::Bytes::from(data_bytes.clone().to_vec()),
                    sell_target.unwrap_or(Address::ZERO),
                    sell_calldata.clone().unwrap_or_else(|| alloy::primitives::Bytes::from(Vec::new())),
                    sell_spender.unwrap_or(Address::ZERO),
                    debt_asset,
                    debt_amount,
                    collateral_asset,
                    min_out,
                )?;
                let flash_calldata = abi.encode_aave_flashloan_simple(
                    receiver_addr,
                    debt_asset,
                    debt_amount,
                    params,
                    0u16,
                )?;
                        // Aave V3 Pool 주소 선택: Aave 프로토콜이면 해당 주소, 아니면 기본 Aave 풀 상수
                        let aave_pool_addr = if protocol_info.name.to_lowercase().contains("aave") {
                            protocol_info.lending_pool_address
                        } else {
                            crate::utils::abi::contracts::AAVE_V3_POOL.clone()
                        };

                        let flashloan_tx = Transaction {
                    hash: alloy::primitives::B256::ZERO,
                    from: alloy::primitives::Address::ZERO,
                            to: Some(aave_pool_addr),
                    value: U256::ZERO,
                    gas_price: U256::from(30_000_000_000u64),
                    gas_limit: U256::from(500_000u64),
                    data: flash_calldata.to_vec(),
                    nonce: 0,
                    timestamp: chrono::Utc::now(),
                    block_number: None,
                };

                // Aave V3 플래시론 프리미엄(기본 9bps) 비용 반영
                let flash_fee = debt_amount * U256::from(9u64) / U256::from(10000u64);
                let adjusted_profit = if opportunity.expected_profit > flash_fee { opportunity.expected_profit - flash_fee } else { U256::ZERO };
                let mut bundle = Bundle::new(vec![flashloan_tx], 0, adjusted_profit, 800_000, StrategyType::Liquidation);

                // 가스 전략 반영
                if let Ok((base_fee, priority_fee)) = self.blockchain_client.get_gas_price().await {
                    let urgency = self.predict_liquidation_urgency(collateral_asset, debt_asset).await.unwrap_or(0.2);
                    let competition = self.estimate_competition_intensity().await.unwrap_or(0.5);
                    let aggressiveness = (urgency * 0.6 + competition * 0.4).clamp(0.0, 1.0);
                    let bump_gwei = ((1.0 + aggressiveness) * 3.0).round() as u64;
                    let adj_priority = priority_fee + ethers::types::U256::from(bump_gwei);
                    let max_fee_eth = base_fee + adj_priority * ethers::types::U256::from(2);
                    bundle.max_fee_per_gas = Some(U256::from_limbs_slice(&max_fee_eth.0));
                    bundle.max_priority_fee_per_gas = Some(U256::from_limbs_slice(&adj_priority.0));
                }
                return Ok(bundle);
            }
        }

        // 플래시론이 비활성화된 경우: 외부 승인/청산/판매 트랜잭션 번들 구성
        let mut txs = Vec::new();
        let approve_calldata = abi.encode_erc20_approve(to_addr, U256::from(u128::MAX))?;
        let approve_tx = Transaction {
            hash: alloy::primitives::B256::ZERO,
            from: alloy::primitives::Address::ZERO,
            to: Some(debt_asset),
            value: U256::ZERO,
            gas_price: U256::from(30_000_000_000u64),
            gas_limit: U256::from(60_000u64),
            data: approve_calldata.to_vec(),
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: None,
        };
        txs.push(approve_tx);
        txs.push(tx);
        if let (Some(st), Some(sc)) = (sell_target, sell_calldata) {
            // 0x 경로의 allowanceTarget이 존재하면, 담보토큰 -> allowanceTarget 승인 추가
            if let Some(spender) = sell_spender {
                let approve_sell_calldata = abi.encode_erc20_approve(spender, U256::from(u128::MAX))?;
                let approve_sell_tx = Transaction {
                    hash: alloy::primitives::B256::ZERO,
                    from: alloy::primitives::Address::ZERO,
                    to: Some(collateral_asset),
                    value: U256::ZERO,
                    gas_price: U256::from(30_000_000_000u64),
                    gas_limit: U256::from(60_000u64),
                    data: approve_sell_calldata.to_vec(),
                    nonce: 0,
                    timestamp: chrono::Utc::now(),
                    block_number: None,
                };
                txs.push(approve_sell_tx);
            }
            let sell_tx = Transaction {
                hash: alloy::primitives::B256::ZERO,
                from: alloy::primitives::Address::ZERO,
                to: Some(st),
                value: U256::ZERO,
                gas_price: U256::from(30_000_000_000u64),
                gas_limit: U256::from(300_000u64),
                data: sc.to_vec(),
                nonce: 0,
                timestamp: chrono::Utc::now(),
                block_number: None,
            };
            txs.push(sell_tx);
        }

        let mut bundle = Bundle::new(txs, 0, opportunity.expected_profit, 800_000, StrategyType::Liquidation);
        // 가스 전략 반영 (비-플래시론 경로)
        if let Ok((base_fee, priority_fee)) = self.blockchain_client.get_gas_price().await {
            let urgency = self.predict_liquidation_urgency(collateral_asset, debt_asset).await.unwrap_or(0.2);
            let competition = self.estimate_competition_intensity().await.unwrap_or(0.5);
            let aggressiveness = (urgency * 0.6 + competition * 0.4).clamp(0.0, 1.0);
            let bump_gwei = ((1.0 + aggressiveness) * 2.0).round() as u64; // 비-플래시론 경로는 살짝 보수적
            let adj_priority = priority_fee + ethers::types::U256::from(bump_gwei);
            let max_fee_eth = base_fee + adj_priority * ethers::types::U256::from(2);
            bundle.max_fee_per_gas = Some(U256::from_limbs_slice(&max_fee_eth.0));
            bundle.max_priority_fee_per_gas = Some(U256::from_limbs_slice(&adj_priority.0));
        }

        // 가스 전략 반영: max_fee_per_gas, max_priority_fee_per_gas 설정
        if let Ok((base_fee, priority_fee)) = self.blockchain_client.get_gas_price().await {
            // 경쟁/긴급도 기반 가중치
            let urgency = self.predict_liquidation_urgency(collateral_asset, debt_asset).await.unwrap_or(0.2);
            let competition = self.estimate_competition_intensity().await.unwrap_or(0.5);
            let aggressiveness = (urgency * 0.6 + competition * 0.4).clamp(0.0, 1.0);
            let bump_gwei = ((1.0 + aggressiveness) * 3.0).round() as u64; // 3~6 gwei 가산
            let adj_priority = priority_fee + ethers::types::U256::from(bump_gwei);
            let max_fee_eth = base_fee + adj_priority * ethers::types::U256::from(2);
            bundle.max_fee_per_gas = Some(U256::from_limbs_slice(&max_fee_eth.0));
            bundle.max_priority_fee_per_gas = Some(U256::from_limbs_slice(&adj_priority.0));
        }

        Ok(bundle)
    }
}

/// ETH 금액 포맷팅 헬퍼
fn format_eth_amount(wei: U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6} ETH", eth)
}

fn hex_addr(addr: Address) -> String {
    format!("0x{}", hex::encode(addr.as_slice()))
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ZeroExQuoteWire {
    #[serde(default)]
    to: String,
    #[serde(default)]
    data: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(rename = "allowanceTarget")]
    #[serde(default)]
    allowance_target: Option<String>,
}

#[derive(Debug, Clone)]
struct ZeroExQuote {
    to: Address,
    data: alloy::primitives::Bytes,
    value: Option<U256>,
    // 0x 특정: allowanceTarget 존재 시, 담보 토큰 approve 필요
    #[allow(dead_code)]
    allowance_target: Option<Address>,
}

impl OnChainLiquidationStrategy {
    fn find_protocol_by_name(&self, name: &str) -> Option<&LendingProtocolInfo> {
        self.lending_protocols
            .values()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// 0x 스왑 견적 시도 (간단 버전)
    async fn try_get_0x_quote(
        &self,
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
    ) -> Result<Option<ZeroExQuote>> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.0x.org/swap/v1/quote?sellToken={}&buyToken={}&sellAmount={}",
            hex_addr(sell_token),
            hex_addr(buy_token),
            sell_amount.to_string()
        );
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let q: ZeroExQuoteWire = resp
            .json()
            .await
            .unwrap_or_else(|_| ZeroExQuoteWire::default());
        if q.to.is_empty() || q.data.is_empty() {
            return Ok(None);
        }
        let to: Address = q.to.parse()?;
        let data_bytes = hex::decode(q.data.trim_start_matches("0x")).unwrap_or_default();
        let value = if let Some(v) = q.value {
            Some(U256::from_str_radix(&v, 10).unwrap_or(U256::ZERO))
        } else {
            None
        };
        // 0x는 allowanceTarget 필드 제공. 있으면 파싱
        let allowance_target = q
            .allowance_target
            .and_then(|s| s.parse::<Address>().ok());
        Ok(Some(ZeroExQuote { to, data: alloy::primitives::Bytes::from(data_bytes), value, allowance_target }))
    }

    /// 1inch 스왑 견적 시도 (백업 경로)
    async fn try_get_1inch_quote(
        &self,
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
    ) -> Result<Option<ZeroExQuote>> {
        // 1inch v5 Ethereum
        let url = format!(
            "https://api.1inch.dev/swap/v5.2/1/quote?src={}&dst={}&amount={}",
            hex_addr(sell_token),
            hex_addr(buy_token),
            sell_amount.to_string()
        );
        let client = reqwest::Client::new();

        // 대부분의 1inch 엔드포인트는 API 키를 요구: Authorization: Bearer <KEY> 또는 apikey 헤더
        let mut req = client.get(&url).header("accept", "application/json");
        if let Ok(key) = std::env::var("ONEINCH_API_KEY") {
            if !key.trim().is_empty() {
                req = req
                    .header("Authorization", format!("Bearer {}", key))
                    .header("apikey", key);
            }
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        // 1inch quote 응답을 간단히 매핑 (실제 응답 스키마는 더 복잡)
        #[derive(Deserialize, Default)]
        struct OneInchQuoteWire { to: Option<String>, data: Option<String>, value: Option<String> }
        let q: OneInchQuoteWire = resp.json().await.unwrap_or_default();
        let to_str = match q.to { Some(t) if !t.is_empty() => t, _ => return Ok(None) };
        let data_str = match q.data { Some(d) if !d.is_empty() => d, _ => return Ok(None) };
        let to: Address = to_str.parse()?;
        let data_bytes = hex::decode(data_str.trim_start_matches("0x")).unwrap_or_default();
        let value = if let Some(v) = q.value { Some(U256::from_str_radix(&v, 10).unwrap_or(U256::ZERO)) } else { None };
        Ok(Some(ZeroExQuote { to, data: alloy::primitives::Bytes::from(data_bytes), value, allowance_target: None }))
    }
}