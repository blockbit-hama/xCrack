use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http},
    types::{Transaction, H256, U256, Address, Bytes, TransactionRequest},
    signers::{LocalWallet, Signer},
};
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, BTreeMap};

use crate::mev::simulation::{BundleSimulator, DetailedSimulationResult, SimulationOptions, SimulationMode};
use crate::blockchain::BlockchainClient;
use crate::types::OpportunityType;

/// MEV 번들
/// 
/// 여러 트랜잭션을 하나의 원자적 실행 단위로 묶어서 MEV를 추출하는 핵심 구조
#[derive(Debug, Clone)]
pub struct Bundle {
    pub id: String,
    pub transactions: Vec<Transaction>,
    pub target_block: u64,
    pub metadata: BundleMetadata,
    pub optimization_info: OptimizationInfo,
    pub validation_status: ValidationStatus,
    pub creation_time: SystemTime,
}

/// 번들 메타데이터
#[derive(Debug, Clone)]
pub struct BundleMetadata {
    pub bundle_type: BundleType,
    pub opportunity_type: OpportunityType,
    pub expected_profit: U256,
    pub max_gas_price: U256,
    pub min_timestamp: Option<u64>,
    pub max_timestamp: Option<u64>,
    pub priority_level: PriorityLevel,
    pub tags: Vec<String>,
    pub source_strategy: String,
}

/// 번들 타입
#[derive(Debug, Clone, PartialEq)]
pub enum BundleType {
    Sandwich,      // 샌드위치 어택
    Arbitrage,     // 차익거래
    Liquidation,   // 청산
    BackRun,       // 백런
    FrontRun,      // 프론트런
    Composite,     // 복합 전략
    Protection,    // MEV 보호
}

/// 우선순위 수준
#[derive(Debug, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub enum PriorityLevel {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
    Emergency = 5,
}

/// 최적화 정보
#[derive(Debug, Clone)]
pub struct OptimizationInfo {
    pub gas_optimized: bool,
    pub order_optimized: bool,
    pub profit_optimized: bool,
    pub risk_optimized: bool,
    pub optimization_score: f64,
    pub optimization_history: Vec<OptimizationStep>,
}

/// 최적화 단계
#[derive(Debug, Clone)]
pub struct OptimizationStep {
    pub step_type: OptimizationType,
    pub before_value: f64,
    pub after_value: f64,
    pub improvement: f64,
    pub timestamp: SystemTime,
}

/// 최적화 타입
#[derive(Debug, Clone)]
pub enum OptimizationType {
    GasReduction,
    ProfitIncrease,
    RiskReduction,
    OrderOptimization,
    TimingOptimization,
}

/// 검증 상태
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationStatus {
    Pending,
    Validating,
    Valid,
    Invalid(String),
    Expired,
}

/// 번들 빌더
/// 
/// MEV 기회를 번들로 변환하고 최적화하는 도구
pub struct BundleBuilder {
    blockchain_client: Arc<BlockchainClient>,
    wallet: LocalWallet,
    nonce_manager: NonceManager,
    gas_estimator: GasEstimator,
    optimization_engine: OptimizationEngine,
}

/// 논스 관리자
#[derive(Debug)]
struct NonceManager {
    current_nonces: HashMap<Address, u64>,
    pending_nonces: HashMap<Address, u64>,
}

/// 가스 추정기
#[derive(Debug)]
struct GasEstimator {
    base_fee_cache: Option<U256>,
    cache_timestamp: Option<SystemTime>,
    cache_ttl: Duration,
}

/// 최적화 엔진
#[derive(Debug)]
struct OptimizationEngine {
    optimization_strategies: Vec<OptimizationStrategy>,
    performance_metrics: PerformanceMetrics,
}

/// 최적화 전략
#[derive(Debug, Clone)]
struct OptimizationStrategy {
    name: String,
    strategy_type: OptimizationType,
    enabled: bool,
    weight: f64,
}

/// 성능 메트릭
#[derive(Debug, Default)]
struct PerformanceMetrics {
    total_optimizations: u64,
    successful_optimizations: u64,
    gas_savings: U256,
    profit_increases: U256,
    avg_optimization_time: Duration,
}

/// 번들 최적화기
/// 
/// 생성된 번들을 최적화하여 수익을 극대화하고 위험을 최소화
pub struct BundleOptimizer {
    simulator: BundleSimulator,
    optimization_config: OptimizationConfig,
    optimization_cache: HashMap<String, OptimizationResult>,
}

/// 최적화 설정
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    pub max_optimization_rounds: usize,
    pub gas_optimization_enabled: bool,
    pub order_optimization_enabled: bool,
    pub timing_optimization_enabled: bool,
    pub risk_optimization_enabled: bool,
    pub profit_threshold: U256,
    pub gas_threshold: u64,
    pub optimization_timeout: Duration,
}

/// 최적화 결과
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub original_bundle: Bundle,
    pub optimized_bundle: Bundle,
    pub improvements: Vec<Improvement>,
    pub optimization_time: Duration,
    pub success_rate: f64,
}

/// 개선 사항
#[derive(Debug, Clone)]
pub struct Improvement {
    pub improvement_type: OptimizationType,
    pub description: String,
    pub before_value: f64,
    pub after_value: f64,
    pub percentage_improvement: f64,
}

impl Bundle {
    /// 새로운 번들 생성
    pub fn new(
        transactions: Vec<Transaction>,
        target_block: u64,
        bundle_type: BundleType,
        opportunity_type: OpportunityType,
    ) -> Self {
        let id = Self::generate_bundle_id(&transactions, target_block);
        
        Self {
            id,
            transactions,
            target_block,
            metadata: BundleMetadata {
                bundle_type,
                opportunity_type,
                expected_profit: U256::zero(),
                max_gas_price: U256::from(100_000_000_000u64), // 100 gwei 기본값
                min_timestamp: None,
                max_timestamp: None,
                priority_level: PriorityLevel::Medium,
                tags: Vec::new(),
                source_strategy: "unknown".to_string(),
            },
            optimization_info: OptimizationInfo::default(),
            validation_status: ValidationStatus::Pending,
            creation_time: SystemTime::now(),
        }
    }

    /// 번들 ID 생성
    fn generate_bundle_id(transactions: &[Transaction], target_block: u64) -> String {
        let tx_hashes: String = transactions.iter()
            .map(|tx| format!("{:?}", tx.hash))
            .collect::<Vec<_>>()
            .join("");
        
        let combined = format!("{}{}", tx_hashes, target_block);
        format!("bundle_{:x}", ethers::utils::keccak256(combined.as_bytes()))
    }

    /// 번들 해시 계산
    pub fn calculate_hash(&self) -> H256 {
        let data = format!("{}{}", self.id, self.target_block);
        H256::from_slice(&ethers::utils::keccak256(data.as_bytes()))
    }

    /// 총 가스 한도 계산
    pub fn total_gas_limit(&self) -> U256 {
        self.transactions.iter()
            .map(|tx| tx.gas)
            .sum()
    }

    /// 번들 크기 (바이트)
    pub fn size_bytes(&self) -> usize {
        self.transactions.iter()
            .map(|tx| tx.rlp().len())
            .sum()
    }

    /// 번들 유효성 확인
    pub fn is_valid(&self) -> bool {
        !self.transactions.is_empty() && 
        self.validation_status == ValidationStatus::Valid
    }

    /// 만료 확인
    pub fn is_expired(&self, current_block: u64) -> bool {
        current_block > self.target_block + 2 // 2블록 여유
    }

    /// 우선순위 점수 계산
    pub fn priority_score(&self) -> f64 {
        let base_score = match self.metadata.priority_level {
            PriorityLevel::Low => 1.0,
            PriorityLevel::Medium => 2.0,
            PriorityLevel::High => 3.0,
            PriorityLevel::Critical => 4.0,
            PriorityLevel::Emergency => 5.0,
        };

        let profit_factor = (self.metadata.expected_profit.as_u128() as f64) / 1e18; // ETH 단위
        let age_factor = self.creation_time.elapsed()
            .unwrap_or_default()
            .as_secs() as f64 / 60.0; // 분 단위

        base_score + profit_factor * 0.1 - age_factor * 0.01
    }

    /// 번들에 트랜잭션 추가
    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
        self.validation_status = ValidationStatus::Pending;
    }

    /// 트랜잭션 순서 변경
    pub fn reorder_transactions(&mut self, new_order: Vec<usize>) -> Result<()> {
        if new_order.len() != self.transactions.len() {
            return Err(anyhow!("순서 배열의 길이가 트랜잭션 수와 일치하지 않습니다"));
        }

        let mut reordered = Vec::new();
        for &index in &new_order {
            if index >= self.transactions.len() {
                return Err(anyhow!("잘못된 트랜잭션 인덱스: {}", index));
            }
            reordered.push(self.transactions[index].clone());
        }

        self.transactions = reordered;
        self.validation_status = ValidationStatus::Pending;
        
        Ok(())
    }

    /// 메타데이터 업데이트
    pub fn update_metadata(&mut self, updates: BundleMetadataUpdate) {
        if let Some(expected_profit) = updates.expected_profit {
            self.metadata.expected_profit = expected_profit;
        }
        if let Some(max_gas_price) = updates.max_gas_price {
            self.metadata.max_gas_price = max_gas_price;
        }
        if let Some(priority_level) = updates.priority_level {
            self.metadata.priority_level = priority_level;
        }
        if let Some(mut tags) = updates.tags {
            self.metadata.tags.append(&mut tags);
        }
    }
}

/// 번들 메타데이터 업데이트
#[derive(Debug, Default)]
pub struct BundleMetadataUpdate {
    pub expected_profit: Option<U256>,
    pub max_gas_price: Option<U256>,
    pub priority_level: Option<PriorityLevel>,
    pub tags: Option<Vec<String>>,
}

impl BundleBuilder {
    /// 새로운 번들 빌더 생성
    pub fn new(
        blockchain_client: Arc<BlockchainClient>,
        wallet: LocalWallet,
    ) -> Self {
        Self {
            blockchain_client,
            wallet,
            nonce_manager: NonceManager::new(),
            gas_estimator: GasEstimator::new(),
            optimization_engine: OptimizationEngine::new(),
        }
    }

    /// 샌드위치 번들 생성
    pub async fn create_sandwich_bundle(
        &mut self,
        victim_tx: Transaction,
        frontrun_params: FrontRunParams,
        backrun_params: BackRunParams,
        target_block: u64,
    ) -> Result<Bundle> {
        info!("🥪 샌드위치 번들 생성 시작");

        // 프론트런 트랜잭션 생성
        let frontrun_tx = self.create_frontrun_transaction(
            &victim_tx,
            frontrun_params,
            target_block,
        ).await?;

        // 백런 트랜잭션 생성
        let backrun_tx = self.create_backrun_transaction(
            &victim_tx,
            &frontrun_tx,
            backrun_params,
            target_block,
        ).await?;

        // 번들 생성 (순서: 프론트런 -> 피해자 -> 백런)
        let transactions = vec![frontrun_tx, victim_tx, backrun_tx];
        
        let mut bundle = Bundle::new(
            transactions,
            target_block,
            BundleType::Sandwich,
            OpportunityType::Sandwich,
        );

        bundle.metadata.source_strategy = "sandwich_strategy".to_string();
        bundle.metadata.priority_level = PriorityLevel::High;
        bundle.metadata.tags.push("sandwich".to_string());
        bundle.metadata.tags.push("mev".to_string());

        info!("✅ 샌드위치 번들 생성 완료: {}", bundle.id);

        Ok(bundle)
    }

    /// 차익거래 번들 생성
    pub async fn create_arbitrage_bundle(
        &mut self,
        arbitrage_params: ArbitrageParams,
        target_block: u64,
    ) -> Result<Bundle> {
        info!("🔄 차익거래 번들 생성 시작");

        let mut transactions = Vec::new();

        // DEX 간 차익거래 트랜잭션들 생성
        for (i, trade) in arbitrage_params.trades.iter().enumerate() {
            let tx = self.create_arbitrage_transaction(
                trade,
                i,
                target_block,
            ).await?;
            transactions.push(tx);
        }

        let mut bundle = Bundle::new(
            transactions,
            target_block,
            BundleType::Arbitrage,
            OpportunityType::MicroArbitrage,
        );

        bundle.metadata.expected_profit = arbitrage_params.expected_profit;
        bundle.metadata.source_strategy = "arbitrage_strategy".to_string();
        bundle.metadata.priority_level = PriorityLevel::Medium;
        bundle.metadata.tags.push("arbitrage".to_string());
        bundle.metadata.tags.push("dex".to_string());

        info!("✅ 차익거래 번들 생성 완료: {}", bundle.id);

        Ok(bundle)
    }

    /// 청산 번들 생성
    pub async fn create_liquidation_bundle(
        &mut self,
        liquidation_params: LiquidationParams,
        target_block: u64,
    ) -> Result<Bundle> {
        info!("💸 청산 번들 생성 시작");

        // 청산 트랜잭션 생성
        let liquidation_tx = self.create_liquidation_transaction(
            &liquidation_params,
            target_block,
        ).await?;

        // 선택적으로 청산된 자산 판매 트랜잭션 추가
        let mut transactions = vec![liquidation_tx];

        if liquidation_params.auto_sell {
            let sell_tx = self.create_asset_sell_transaction(
                &liquidation_params,
                target_block,
            ).await?;
            transactions.push(sell_tx);
        }

        let mut bundle = Bundle::new(
            transactions,
            target_block,
            BundleType::Liquidation,
            OpportunityType::Liquidation,
        );

        bundle.metadata.expected_profit = liquidation_params.expected_profit;
        bundle.metadata.source_strategy = "liquidation_strategy".to_string();
        bundle.metadata.priority_level = PriorityLevel::High;
        bundle.metadata.tags.push("liquidation".to_string());
        bundle.metadata.tags.push("defi".to_string());

        info!("✅ 청산 번들 생성 완료: {}", bundle.id);

        Ok(bundle)
    }

    /// 프론트런 트랜잭션 생성
    async fn create_frontrun_transaction(
        &mut self,
        victim_tx: &Transaction,
        params: FrontRunParams,
        target_block: u64,
    ) -> Result<Transaction> {
        // 피해자 트랜잭션보다 높은 가스 가격 설정
        let gas_price = victim_tx.gas_price
            .unwrap_or_default()
            .saturating_mul(U256::from(params.gas_multiplier as u64))
            .max(params.min_gas_price);

        // 프론트런 트랜잭션 생성 (실제 구현에서는 더 복잡)
        let tx_request = TransactionRequest::new()
            .to(params.target_contract)
            .value(params.value)
            .data(params.calldata)
            .gas(params.gas_limit)
            .gas_price(gas_price)
            .nonce(self.get_next_nonce().await?);

        // 트랜잭션 서명
        let signed_tx = self.wallet.sign_transaction(&tx_request.into()).await?;
        
        Ok(signed_tx)
    }

    /// 백런 트랜잭션 생성
    async fn create_backrun_transaction(
        &mut self,
        _victim_tx: &Transaction,
        _frontrun_tx: &Transaction,
        params: BackRunParams,
        _target_block: u64,
    ) -> Result<Transaction> {
        // 백런 트랜잭션 생성 (프론트런의 결과를 활용)
        let tx_request = TransactionRequest::new()
            .to(params.target_contract)
            .value(params.value)
            .data(params.calldata)
            .gas(params.gas_limit)
            .gas_price(params.gas_price)
            .nonce(self.get_next_nonce().await?);

        let signed_tx = self.wallet.sign_transaction(&tx_request.into()).await?;
        
        Ok(signed_tx)
    }

    /// 차익거래 트랜잭션 생성
    async fn create_arbitrage_transaction(
        &mut self,
        trade: &ArbTrade,
        _index: usize,
        _target_block: u64,
    ) -> Result<Transaction> {
        let tx_request = TransactionRequest::new()
            .to(trade.target_contract)
            .value(trade.value)
            .data(trade.calldata.clone())
            .gas(trade.gas_limit)
            .gas_price(trade.gas_price)
            .nonce(self.get_next_nonce().await?);

        let signed_tx = self.wallet.sign_transaction(&tx_request.into()).await?;
        
        Ok(signed_tx)
    }

    /// 청산 트랜잭션 생성
    async fn create_liquidation_transaction(
        &mut self,
        params: &LiquidationParams,
        _target_block: u64,
    ) -> Result<Transaction> {
        let tx_request = TransactionRequest::new()
            .to(params.protocol_contract)
            .value(U256::zero())
            .data(params.liquidation_calldata.clone())
            .gas(params.gas_limit)
            .gas_price(params.gas_price)
            .nonce(self.get_next_nonce().await?);

        let signed_tx = self.wallet.sign_transaction(&tx_request.into()).await?;
        
        Ok(signed_tx)
    }

    /// 플래시론 + 청산 + 상환을 하나의 번들로 구성 (간단 구성)
    pub async fn create_flashloan_liquidation_bundle(
        &mut self,
        flashloan_tx: Transaction,
        liquidation_tx: Transaction,
        repay_tx: Transaction,
        target_block: u64,
        expected_profit: U256,
    ) -> Result<Bundle> {
        let transactions = vec![flashloan_tx, liquidation_tx, repay_tx];
        let mut bundle = Bundle::new(
            transactions,
            target_block,
            BundleType::Liquidation,
            OpportunityType::Liquidation,
        );
        bundle.metadata.expected_profit = expected_profit;
        bundle.metadata.priority_level = PriorityLevel::Critical;
        bundle.metadata.tags.push("flashloan".to_string());
        bundle.metadata.tags.push("liquidation".to_string());
        Ok(bundle)
    }

    /// 자산 판매 트랜잭션 생성
    async fn create_asset_sell_transaction(
        &mut self,
        params: &LiquidationParams,
        _target_block: u64,
    ) -> Result<Transaction> {
        let tx_request = TransactionRequest::new()
            .to(params.sell_contract.unwrap_or_default())
            .value(U256::zero())
            .data(params.sell_calldata.as_ref().unwrap_or(&Bytes::new()).clone())
            .gas(200000) // 기본 가스 한도
            .gas_price(params.gas_price)
            .nonce(self.get_next_nonce().await?);

        let signed_tx = self.wallet.sign_transaction(&tx_request.into()).await?;
        
        Ok(signed_tx)
    }

    /// 다음 논스 가져오기
    async fn get_next_nonce(&mut self) -> Result<u64> {
        let address = self.wallet.address();
        
        if let Some(nonce) = self.nonce_manager.get_pending_nonce(address) {
            Ok(nonce)
        } else {
            // 블록체인에서 현재 논스 조회
            let current_nonce = self.blockchain_client.get_transaction_count(address).await?;
            self.nonce_manager.set_base_nonce(address, current_nonce);
            Ok(current_nonce)
        }
    }
}

/// 프론트런 파라미터
#[derive(Debug, Clone)]
pub struct FrontRunParams {
    pub target_contract: Address,
    pub value: U256,
    pub calldata: Bytes,
    pub gas_limit: U256,
    pub gas_multiplier: f64,
    pub min_gas_price: U256,
}

/// 백런 파라미터
#[derive(Debug, Clone)]
pub struct BackRunParams {
    pub target_contract: Address,
    pub value: U256,
    pub calldata: Bytes,
    pub gas_limit: U256,
    pub gas_price: U256,
}

/// 차익거래 파라미터
#[derive(Debug, Clone)]
pub struct ArbitrageParams {
    pub trades: Vec<ArbTrade>,
    pub expected_profit: U256,
    pub max_slippage: f64,
}

/// 차익거래 트레이드
#[derive(Debug, Clone)]
pub struct ArbTrade {
    pub target_contract: Address,
    pub value: U256,
    pub calldata: Bytes,
    pub gas_limit: U256,
    pub gas_price: U256,
    pub expected_output: U256,
}

/// 청산 파라미터
#[derive(Debug, Clone)]
pub struct LiquidationParams {
    pub protocol_contract: Address,
    pub liquidation_calldata: Bytes,
    pub gas_limit: U256,
    pub gas_price: U256,
    pub expected_profit: U256,
    pub auto_sell: bool,
    pub sell_contract: Option<Address>,
    pub sell_calldata: Option<Bytes>,
        pub use_flash_loan: bool,
        pub flash_loan_amount: Option<U256>,
}

impl NonceManager {
    fn new() -> Self {
        Self {
            current_nonces: HashMap::new(),
            pending_nonces: HashMap::new(),
        }
    }

    fn get_pending_nonce(&mut self, address: Address) -> Option<u64> {
        if let Some(&nonce) = self.pending_nonces.get(&address) {
            self.pending_nonces.insert(address, nonce + 1);
            Some(nonce)
        } else {
            None
        }
    }

    fn set_base_nonce(&mut self, address: Address, nonce: u64) {
        self.current_nonces.insert(address, nonce);
        self.pending_nonces.insert(address, nonce + 1);
    }
}

impl GasEstimator {
    fn new() -> Self {
        Self {
            base_fee_cache: None,
            cache_timestamp: None,
            cache_ttl: Duration::from_secs(12), // 1 블록
        }
    }
}

impl OptimizationEngine {
    fn new() -> Self {
        let optimization_strategies = vec![
            OptimizationStrategy {
                name: "gas_optimization".to_string(),
                strategy_type: OptimizationType::GasReduction,
                enabled: true,
                weight: 1.0,
            },
            OptimizationStrategy {
                name: "profit_maximization".to_string(),
                strategy_type: OptimizationType::ProfitIncrease,
                enabled: true,
                weight: 1.2,
            },
            OptimizationStrategy {
                name: "order_optimization".to_string(),
                strategy_type: OptimizationType::OrderOptimization,
                enabled: true,
                weight: 0.8,
            },
        ];

        Self {
            optimization_strategies,
            performance_metrics: PerformanceMetrics::default(),
        }
    }
}

impl BundleOptimizer {
    /// 새로운 번들 최적화기 생성
    pub fn new(simulator: BundleSimulator) -> Self {
        Self {
            simulator,
            optimization_config: OptimizationConfig::default(),
            optimization_cache: HashMap::new(),
        }
    }

    /// 번들 최적화 실행
    pub async fn optimize_bundle(&mut self, bundle: Bundle) -> Result<OptimizationResult> {
        info!("⚡ 번들 최적화 시작: {}", bundle.id);
        let start_time = SystemTime::now();

        let original_bundle = bundle.clone();
        let mut current_bundle = bundle;
        let mut improvements = Vec::new();

        // 캐시 확인
        let cache_key = format!("{}_{}", current_bundle.id, current_bundle.target_block);
        if let Some(cached_result) = self.optimization_cache.get(&cache_key) {
            info!("📋 캐시된 최적화 결과 사용");
            return Ok(cached_result.clone());
        }

        // 초기 시뮬레이션
        let mut best_simulation = self.simulator.simulate_bundle(
            &current_bundle.transactions,
            SimulationOptions {
                simulation_mode: SimulationMode::Fast,
                ..Default::default()
            },
        ).await?;

        // 최적화 라운드 실행
        for round in 0..self.optimization_config.max_optimization_rounds {
            info!("🔄 최적화 라운드 {}/{}", round + 1, self.optimization_config.max_optimization_rounds);

            let mut round_improved = false;

            // 가스 최적화
            if self.optimization_config.gas_optimization_enabled {
                if let Ok(optimized) = self.optimize_gas(&current_bundle).await {
                    if optimized.optimization_info.optimization_score > current_bundle.optimization_info.optimization_score {
                        info!("⛽ 가스 최적화 적용");
                        current_bundle = optimized;
                        round_improved = true;
                    }
                }
            }

            // 순서 최적화
            if self.optimization_config.order_optimization_enabled {
                if let Ok(optimized) = self.optimize_order(&current_bundle).await {
                    if optimized.optimization_info.optimization_score > current_bundle.optimization_info.optimization_score {
                        info!("🔄 순서 최적화 적용");
                        current_bundle = optimized;
                        round_improved = true;
                    }
                }
            }

            // 타이밍 최적화
            if self.optimization_config.timing_optimization_enabled {
                if let Ok(optimized) = self.optimize_timing(&current_bundle).await {
                    if optimized.optimization_info.optimization_score > current_bundle.optimization_info.optimization_score {
                        info!("⏰ 타이밍 최적화 적용");
                        current_bundle = optimized;
                        round_improved = true;
                    }
                }
            }

            // 개선이 없으면 중단
            if !round_improved {
                info!("ℹ️ 더 이상 최적화 불가, 중단");
                break;
            }
        }

        // 최종 시뮬레이션
        let final_simulation = self.simulator.simulate_bundle(
            &current_bundle.transactions,
            SimulationOptions {
                simulation_mode: SimulationMode::Accurate,
                ..Default::default()
            },
        ).await?;

        // 개선 사항 계산
        if final_simulation.profit_after_gas > best_simulation.profit_after_gas {
            let profit_improvement = final_simulation.profit_after_gas - best_simulation.profit_after_gas;
            let percentage = (profit_improvement.as_u128() as f64) / (best_simulation.profit_after_gas.as_u128() as f64) * 100.0;
            
            improvements.push(Improvement {
                improvement_type: OptimizationType::ProfitIncrease,
                description: "수익 증가".to_string(),
                before_value: format_eth_amount(best_simulation.profit_after_gas).parse().unwrap_or(0.0),
                after_value: format_eth_amount(final_simulation.profit_after_gas).parse().unwrap_or(0.0),
                percentage_improvement: percentage,
            });
        }

        let optimization_time = start_time.elapsed().unwrap_or_default();
        let success_rate = if improvements.is_empty() { 0.5 } else { 1.0 };

        let result = OptimizationResult {
            original_bundle,
            optimized_bundle: current_bundle,
            improvements,
            optimization_time,
            success_rate,
        };

        // 결과 캐시
        self.optimization_cache.insert(cache_key, result.clone());

        info!("✅ 번들 최적화 완료");
        info!("  ⏱️ 소요 시간: {:?}", optimization_time);
        info!("  📈 개선 사항: {}", result.improvements.len());

        Ok(result)
    }

    /// 가스 최적화
    async fn optimize_gas(&mut self, bundle: &Bundle) -> Result<Bundle> {
        debug!("⛽ 가스 최적화 실행");
        
        let mut optimized = bundle.clone();
        
        // 가스 한도 최적화
        for tx in &mut optimized.transactions {
            let estimated_gas = self.estimate_optimal_gas(tx).await?;
            if estimated_gas < tx.gas {
                tx.gas = estimated_gas;
            }
        }

        // 최적화 점수 업데이트
        optimized.optimization_info.gas_optimized = true;
        optimized.optimization_info.optimization_score += 0.1;

        Ok(optimized)
    }

    /// 순서 최적화
    async fn optimize_order(&mut self, bundle: &Bundle) -> Result<Bundle> {
        debug!("🔄 순서 최적화 실행");
        
        let mut best_bundle = bundle.clone();
        let mut best_score = 0.0;

        // 다양한 순서 시도 (간단한 버전)
        let tx_count = bundle.transactions.len();
        if tx_count <= 6 { // 팩토리얼 복잡도 때문에 제한
            for permutation in self.generate_permutations(tx_count) {
                let mut test_bundle = bundle.clone();
                test_bundle.reorder_transactions(permutation)?;

                if let Ok(simulation) = self.simulator.simulate_bundle(
                    &test_bundle.transactions,
                    SimulationOptions {
                        simulation_mode: SimulationMode::Fast,
                        ..Default::default()
                    },
                ).await {
                    if simulation.validation_score > best_score {
                        best_score = simulation.validation_score;
                        best_bundle = test_bundle;
                    }
                }
            }
        }

        best_bundle.optimization_info.order_optimized = true;
        best_bundle.optimization_info.optimization_score += 0.1;

        Ok(best_bundle)
    }

    /// 타이밍 최적화
    async fn optimize_timing(&mut self, bundle: &Bundle) -> Result<Bundle> {
        debug!("⏰ 타이밍 최적화 실행");
        
        let mut optimized = bundle.clone();
        
        // 타임스탬프 최적화 (예시)
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        optimized.metadata.min_timestamp = Some(current_time);
        optimized.metadata.max_timestamp = Some(current_time + 120); // 2분 여유

        optimized.optimization_info.optimization_score += 0.05;

        Ok(optimized)
    }

    /// 최적 가스 추정
    async fn estimate_optimal_gas(&self, _tx: &Transaction) -> Result<U256> {
        // 실제 구현에서는 더 정교한 가스 추정 필요
        Ok(U256::from(200000)) // 기본값
    }

    /// 순열 생성
    fn generate_permutations(&self, n: usize) -> Vec<Vec<usize>> {
        if n <= 1 {
            return vec![vec![0]];
        }

        let mut result = Vec::new();
        let indices: Vec<usize> = (0..n).collect();
        
        // 간단한 몇 가지 순열만 시도 (성능상 제한)
        result.push(indices.clone()); // 원본
        
        if n >= 2 {
            let mut swapped = indices.clone();
            swapped.swap(0, 1);
            result.push(swapped);
        }

        if n >= 3 {
            let mut swapped = indices.clone();
            swapped.swap(1, 2);
            result.push(swapped);
        }

        result
    }
}

impl Default for OptimizationInfo {
    fn default() -> Self {
        Self {
            gas_optimized: false,
            order_optimized: false,
            profit_optimized: false,
            risk_optimized: false,
            optimization_score: 0.0,
            optimization_history: Vec::new(),
        }
    }
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            max_optimization_rounds: 3,
            gas_optimization_enabled: true,
            order_optimization_enabled: true,
            timing_optimization_enabled: true,
            risk_optimization_enabled: true,
            profit_threshold: U256::from(10_000_000_000_000_000u64), // 0.01 ETH
            gas_threshold: 500_000,
            optimization_timeout: Duration::from_secs(30),
        }
    }
}

/// ETH 금액 포맷팅
fn format_eth_amount(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_creation() {
        let transactions = vec![]; // 빈 트랜잭션 리스트로 테스트
        let bundle = Bundle::new(
            transactions,
            100,
            BundleType::Sandwich,
            OpportunityType::Sandwich,
        );

        assert_eq!(bundle.target_block, 100);
        assert_eq!(bundle.metadata.bundle_type, BundleType::Sandwich);
        assert_eq!(bundle.validation_status, ValidationStatus::Pending);
    }

    #[test]
    fn test_priority_score() {
        let mut bundle = Bundle::new(
            vec![],
            100,
            BundleType::Arbitrage,
            OpportunityType::MicroArbitrage,
        );

        bundle.metadata.priority_level = PriorityLevel::High;
        bundle.metadata.expected_profit = U256::from(1_000_000_000_000_000_000u64); // 1 ETH

        let score = bundle.priority_score();
        assert!(score > 3.0); // High priority (3.0) + profit factor
    }

    #[test]
    fn test_bundle_reordering() {
        let mut bundle = Bundle::new(
            vec![
                Transaction::default(),
                Transaction::default(),
                Transaction::default(),
            ],
            100,
            BundleType::Sandwich,
            OpportunityType::Sandwich,
        );

        let result = bundle.reorder_transactions(vec![2, 0, 1]);
        assert!(result.is_ok());
        assert_eq!(bundle.validation_status, ValidationStatus::Pending);
    }
}