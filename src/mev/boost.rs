use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http, Middleware},
    types::{Transaction, H256, U256, Address, Bytes, Block, Signature},
    signers::{LocalWallet, Signer},
    utils::hex,
};
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, BTreeMap};
use reqwest::Client as HttpClient;
use tokio::sync::{Mutex, RwLock};

use crate::mev::{Bundle, FlashbotsClient};
use crate::blockchain::BlockchainClient;

/// MEV-Boost 클라이언트
/// 
/// MEV-Boost 프로토콜을 통한 블록 빌더와의 통신 및 블록 제출 관리
pub struct MEVBoostClient {
    relay_endpoints: Vec<RelayEndpoint>,
    http_client: HttpClient,
    signer: LocalWallet,
    blockchain_client: Arc<BlockchainClient>,
    builder_registry: Arc<RwLock<HashMap<String, BlockBuilderInfo>>>,
    submission_history: Arc<Mutex<Vec<SubmissionRecord>>>,
    performance_metrics: Arc<Mutex<PerformanceMetrics>>,
    config: MEVBoostConfig,
}

/// 릴레이 엔드포인트
#[derive(Debug, Clone)]
pub struct RelayEndpoint {
    pub name: String,
    pub url: String,
    pub public_key: String,
    pub is_active: bool,
    pub priority: u8,
    pub commission_rate: f64,
    pub reputation_score: f64,
    pub avg_response_time_ms: u64,
}

/// 블록 빌더 정보
#[derive(Debug, Clone)]
pub struct BlockBuilderInfo {
    pub builder_id: String,
    pub builder_pubkey: String,
    pub relay_name: String,
    pub reputation: f64,
    pub total_blocks_built: u64,
    pub avg_block_value: U256,
    pub success_rate: f64,
    pub last_active: SystemTime,
}

/// 제출 기록
#[derive(Debug, Clone)]
pub struct SubmissionRecord {
    pub bundle_id: String,
    pub relay_name: String,
    pub builder_id: String,
    pub submission_time: SystemTime,
    pub block_number: u64,
    pub status: SubmissionStatus,
    pub bid_value: U256,
    pub gas_used: u64,
    pub profit: U256,
}

/// 제출 상태
#[derive(Debug, Clone, PartialEq)]
pub enum SubmissionStatus {
    Submitted,
    Accepted,
    Rejected(String),
    Included,
    Missed,
    Timeout,
}

/// 성능 메트릭
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub total_submissions: u64,
    pub successful_inclusions: u64,
    pub total_profit: U256,
    pub avg_submission_time_ms: f64,
    pub relay_performance: HashMap<String, RelayMetrics>,
}

/// 릴레이 메트릭
#[derive(Debug, Default, Clone)]
pub struct RelayMetrics {
    pub submissions: u64,
    pub inclusions: u64,
    pub total_value: U256,
    pub avg_response_time: f64,
    pub success_rate: f64,
}

/// MEV-Boost 설정
#[derive(Debug, Clone)]
pub struct MEVBoostConfig {
    pub max_bid_value: U256,
    pub min_bid_value: U256,
    pub submission_timeout: Duration,
    pub max_concurrent_submissions: usize,
    pub enable_multi_relay: bool,
    pub auto_relay_selection: bool,
    pub bid_strategy: BidStrategy,
}

/// 입찰 전략
#[derive(Debug, Clone)]
pub enum BidStrategy {
    Conservative,  // 안전한 입찰
    Aggressive,    // 공격적 입찰
    Adaptive,      // 적응형 입찰
    Fixed(U256),   // 고정 입찰
}

/// 블록 빌더
/// 
/// MEV 번들을 블록으로 구성하고 최적화하는 컴포넌트
pub struct BlockBuilder {
    mev_boost_client: Arc<MEVBoostClient>,
    blockchain_client: Arc<BlockchainClient>,
    current_block_template: Arc<Mutex<Option<BlockTemplate>>>,
    bundle_queue: Arc<Mutex<Vec<Bundle>>>,
    optimization_engine: OptimizationEngine,
    gas_limit_manager: GasLimitManager,
}

/// 블록 템플릿
#[derive(Debug, Clone)]
pub struct BlockTemplate {
    pub parent_hash: H256,
    pub block_number: u64,
    pub timestamp: u64,
    pub base_fee: U256,
    pub gas_limit: u64,
    pub coinbase: Address,
    pub transactions: Vec<Transaction>,
    pub total_gas_used: u64,
    pub total_fees: U256,
    pub mev_value: U256,
}

/// 최적화 엔진
#[derive(Debug)]
struct OptimizationEngine {
    optimization_strategies: Vec<BlockOptimizationStrategy>,
}

/// 블록 최적화 전략
#[derive(Debug, Clone)]
struct BlockOptimizationStrategy {
    name: String,
    strategy_type: OptimizationStrategyType,
    weight: f64,
    enabled: bool,
}

/// 최적화 전략 타입
#[derive(Debug, Clone)]
enum OptimizationStrategyType {
    MaxValue,           // 최대 가치 추구
    MaxGasEfficiency,   // 가스 효율성 최대화
    MinRisk,            // 위험 최소화
    Balanced,           // 균형 잡힌 접근
}

/// 가스 한도 관리자
#[derive(Debug)]
struct GasLimitManager {
    target_gas_limit: u64,
    max_gas_limit: u64,
    reserved_gas: u64,
    gas_price_oracle: GasPriceOracle,
}

/// 가스 가격 오라클
#[derive(Debug)]
struct GasPriceOracle {
    base_fee_cache: Option<U256>,
    priority_fee_cache: Option<U256>,
    cache_timestamp: Option<SystemTime>,
    cache_ttl: Duration,
}

impl MEVBoostClient {
    /// 새로운 MEV-Boost 클라이언트 생성
    pub fn new(
        signer: LocalWallet,
        blockchain_client: Arc<BlockchainClient>,
        config: MEVBoostConfig,
    ) -> Self {
        let relay_endpoints = Self::default_relay_endpoints();

        Self {
            relay_endpoints,
            http_client: HttpClient::new(),
            signer,
            blockchain_client,
            builder_registry: Arc::new(RwLock::new(HashMap::new())),
            submission_history: Arc::new(Mutex::new(Vec::new())),
            performance_metrics: Arc::new(Mutex::new(PerformanceMetrics::default())),
            config,
        }
    }

    /// 기본 릴레이 엔드포인트
    fn default_relay_endpoints() -> Vec<RelayEndpoint> {
        vec![
            RelayEndpoint {
                name: "Flashbots".to_string(),
                url: "https://boost-relay.flashbots.net".to_string(),
                public_key: "0x9000009807ed12c1f08bf4e81c6da3ba8e3fc3d953898ce0102433094e5f22f21102ec057841fcb81978ed1ea0fa8246".to_string(),
                is_active: true,
                priority: 1,
                commission_rate: 0.0,
                reputation_score: 0.95,
                avg_response_time_ms: 50,
            },
            RelayEndpoint {
                name: "BloXroute Max Profit".to_string(),
                url: "https://mev-boost.bloxroute.max-profit.blxrbdn.com".to_string(),
                public_key: "0xad0a8bb54565c2211cee576363f3a347089d2f07cf72679d16911d740262694cadb62d7fd7483f27afd714ca0f1b9118".to_string(),
                is_active: true,
                priority: 2,
                commission_rate: 0.03,
                reputation_score: 0.88,
                avg_response_time_ms: 75,
            },
            RelayEndpoint {
                name: "Eden Network".to_string(),
                url: "https://boost-relay.edennetwork.io".to_string(),
                public_key: "0xb3ee7afcf27f1f1259ac1787876318c6584ee353097a50ed84f51a1f21a323b3736f271a895c7ce918c038e4265918be".to_string(),
                is_active: true,
                priority: 3,
                commission_rate: 0.02,
                reputation_score: 0.82,
                avg_response_time_ms: 90,
            },
        ]
    }

    /// 번들을 MEV-Boost 릴레이에 제출
    pub async fn submit_bundle_to_boost(
        &self,
        bundle: Bundle,
        target_block: u64,
    ) -> Result<Vec<BoostSubmissionResult>> {
        info!("🚀 MEV-Boost에 번들 제출: {}", bundle.id);

        let mut results = Vec::new();
        let active_relays = self.get_active_relays();

        if self.config.enable_multi_relay {
            // 여러 릴레이에 동시 제출
            let mut handles = Vec::new();

            for relay in active_relays {
                let bundle_clone = bundle.clone();
                let self_clone = Arc::new(self.clone());
                let relay_clone = relay.clone();

                let handle = tokio::spawn(async move {
                    self_clone.submit_to_single_relay(bundle_clone, target_block, relay_clone).await
                });

                handles.push(handle);
            }

            // 모든 제출 결과 수집
            for handle in handles {
                match handle.await {
                    Ok(Ok(result)) => results.push(result),
                    Ok(Err(e)) => warn!("릴레이 제출 실패: {}", e),
                    Err(e) => warn!("제출 태스크 실패: {}", e),
                }
            }
        } else {
            // 단일 릴레이에 제출 (우선순위 기반)
            if let Some(best_relay) = self.select_best_relay(&active_relays) {
                match self.submit_to_single_relay(bundle, target_block, best_relay).await {
                    Ok(result) => results.push(result),
                    Err(e) => error!("릴레이 제출 실패: {}", e),
                }
            }
        }

        info!("📊 제출 완료: {} 릴레이", results.len());
        Ok(results)
    }

    /// 단일 릴레이에 제출
    async fn submit_to_single_relay(
        &self,
        bundle: Bundle,
        target_block: u64,
        relay: RelayEndpoint,
    ) -> Result<BoostSubmissionResult> {
        info!("📡 릴레이 제출: {} -> {}", bundle.id, relay.name);

        let start_time = SystemTime::now();

        // 입찰 가격 계산
        let bid_value = self.calculate_bid_value(&bundle, &relay).await?;

        // 블록 헤더 생성
        let block_header = self.create_block_header(target_block, bid_value).await?;

        // 제출 페이로드 구성
        let submission_payload = BoostSubmissionPayload {
            slot: target_block,
            parent_hash: block_header.parent_hash,
            block_hash: block_header.hash,
            builder_pubkey: self.signer.address().into(),
            proposer_pubkey: Address::zero(), // 실제로는 검증자 공개키
            proposer_fee_recipient: Address::zero(), // 수수료 수신자
            gas_limit: block_header.gas_limit,
            gas_used: bundle.total_gas_limit().as_u64(),
            value: bid_value,
            transactions: bundle.transactions.clone(),
        };

        // 서명 생성
        let signature = self.sign_submission(&submission_payload).await?;

        // HTTP 요청 구성
        let request_body = serde_json::json!({
            "message": submission_payload,
            "signature": format!("0x{}", hex::encode(signature.to_vec()))
        });

        // 릴레이에 제출
        let response = self.http_client
            .post(&format!("{}/eth/v1/builder/blocks", relay.url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(self.config.submission_timeout)
            .send()
            .await?;

        let submission_time = start_time.elapsed().unwrap_or_default();

        let result = if response.status().is_success() {
            let response_text = response.text().await?;
            info!("✅ 릴레이 제출 성공: {} ({}ms)", relay.name, submission_time.as_millis());
            
            BoostSubmissionResult {
                relay_name: relay.name.clone(),
                bundle_id: bundle.id.clone(),
                status: SubmissionStatus::Accepted,
                bid_value,
                submission_time,
                response_data: Some(response_text),
                error: None,
            }
        } else {
            let error_text = response.text().await.unwrap_or_default();
            warn!("❌ 릴레이 제출 실패: {} - {}", relay.name, error_text);
            
            BoostSubmissionResult {
                relay_name: relay.name.clone(),
                bundle_id: bundle.id.clone(),
                status: SubmissionStatus::Rejected(error_text.clone()),
                bid_value,
                submission_time,
                response_data: None,
                error: Some(error_text),
            }
        };

        // 제출 기록 저장
        self.record_submission(&bundle, &relay, &result).await;

        Ok(result)
    }

    /// 활성 릴레이 가져오기
    fn get_active_relays(&self) -> Vec<RelayEndpoint> {
        self.relay_endpoints.iter()
            .filter(|relay| relay.is_active)
            .cloned()
            .collect()
    }

    /// 최적 릴레이 선택
    fn select_best_relay(&self, relays: &[RelayEndpoint]) -> Option<RelayEndpoint> {
        if self.config.auto_relay_selection {
            // 성능 기반 자동 선택
            relays.iter()
                .max_by(|a, b| {
                    let score_a = a.reputation_score - (a.avg_response_time_ms as f64 / 1000.0) * 0.1;
                    let score_b = b.reputation_score - (b.avg_response_time_ms as f64 / 1000.0) * 0.1;
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned()
        } else {
            // 우선순위 기반 선택
            relays.iter()
                .min_by_key(|relay| relay.priority)
                .cloned()
        }
    }

    /// 입찰 가격 계산
    async fn calculate_bid_value(&self, bundle: &Bundle, relay: &RelayEndpoint) -> Result<U256> {
        let base_value = bundle.metadata.expected_profit;
        
        let bid_value = match &self.config.bid_strategy {
            BidStrategy::Conservative => base_value * U256::from(70) / U256::from(100), // 70%
            BidStrategy::Aggressive => base_value * U256::from(95) / U256::from(100),   // 95%
            BidStrategy::Adaptive => {
                // 릴레이 성능에 따른 적응형 입찰
                let performance_factor = relay.reputation_score;
                let percentage = (50.0 + performance_factor * 40.0) as u64; // 50-90%
                base_value * U256::from(percentage) / U256::from(100)
            }
            BidStrategy::Fixed(value) => *value,
        };

        // 한도 확인
        let final_bid = bid_value
            .max(self.config.min_bid_value)
            .min(self.config.max_bid_value);

        Ok(final_bid)
    }

    /// 블록 헤더 생성
    async fn create_block_header(&self, block_number: u64, value: U256) -> Result<BlockHeader> {
        let parent_block = self.blockchain_client.get_block(block_number - 1).await?;
        let parent_hash = parent_block.and_then(|b| b.hash).unwrap_or_default();

        Ok(BlockHeader {
            parent_hash,
            block_number,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            gas_limit: 30_000_000, // 30M gas 기본값
            hash: H256::zero(), // 실제로는 계산 필요
            value,
        })
    }

    /// 제출 서명
    async fn sign_submission(&self, payload: &BoostSubmissionPayload) -> Result<Signature> {
        let message = serde_json::to_string(payload)?;
        let signature = self.signer.sign_message(&message).await?;
        Ok(signature)
    }

    /// 제출 기록
    async fn record_submission(
        &self,
        bundle: &Bundle,
        relay: &RelayEndpoint,
        result: &BoostSubmissionResult,
    ) {
        let record = SubmissionRecord {
            bundle_id: bundle.id.clone(),
            relay_name: relay.name.clone(),
            builder_id: format!("{:?}", self.signer.address()),
            submission_time: SystemTime::now(),
            block_number: bundle.target_block,
            status: result.status.clone(),
            bid_value: result.bid_value,
            gas_used: bundle.total_gas_limit().as_u64(),
            profit: bundle.metadata.expected_profit,
        };

        let mut history = self.submission_history.lock().await;
        history.push(record);

        // 최대 1000개 기록 유지
        if history.len() > 1000 {
            history.remove(0);
        }
    }

    /// 성능 메트릭 업데이트
    pub async fn update_performance_metrics(&self, relay_name: &str, was_included: bool, profit: U256) {
        let mut metrics = self.performance_metrics.lock().await;
        metrics.total_submissions += 1;
        
        if was_included {
            metrics.successful_inclusions += 1;
            metrics.total_profit += profit;
        }

        let relay_metrics = metrics.relay_performance
            .entry(relay_name.to_string())
            .or_default();
        
        relay_metrics.submissions += 1;
        if was_included {
            relay_metrics.inclusions += 1;
            relay_metrics.total_value += profit;
        }
        relay_metrics.success_rate = relay_metrics.inclusions as f64 / relay_metrics.submissions as f64;
    }

    /// 제출 기록 조회
    pub async fn get_submission_history(&self, limit: Option<usize>) -> Vec<SubmissionRecord> {
        let history = self.submission_history.lock().await;
        let count = limit.unwrap_or(history.len());
        history.iter().rev().take(count).cloned().collect()
    }

    /// 성능 통계 조회
    pub async fn get_performance_stats(&self) -> PerformanceMetrics {
        let metrics = self.performance_metrics.lock().await;
        metrics.clone()
    }
}

/// MEV-Boost 제출 결과
#[derive(Debug, Clone)]
pub struct BoostSubmissionResult {
    pub relay_name: String,
    pub bundle_id: String,
    pub status: SubmissionStatus,
    pub bid_value: U256,
    pub submission_time: Duration,
    pub response_data: Option<String>,
    pub error: Option<String>,
}

/// MEV-Boost 제출 페이로드
#[derive(Debug, Serialize)]
struct BoostSubmissionPayload {
    slot: u64,
    parent_hash: H256,
    block_hash: H256,
    builder_pubkey: Address,
    proposer_pubkey: Address,
    proposer_fee_recipient: Address,
    gas_limit: u64,
    gas_used: u64,
    value: U256,
    transactions: Vec<Transaction>,
}

/// 블록 헤더
#[derive(Debug, Clone)]
struct BlockHeader {
    parent_hash: H256,
    block_number: u64,
    timestamp: u64,
    gas_limit: u64,
    hash: H256,
    value: U256,
}

impl Clone for MEVBoostClient {
    fn clone(&self) -> Self {
        Self {
            relay_endpoints: self.relay_endpoints.clone(),
            http_client: HttpClient::new(),
            signer: self.signer.clone(),
            blockchain_client: Arc::clone(&self.blockchain_client),
            builder_registry: Arc::clone(&self.builder_registry),
            submission_history: Arc::clone(&self.submission_history),
            performance_metrics: Arc::clone(&self.performance_metrics),
            config: self.config.clone(),
        }
    }
}

impl Clone for PerformanceMetrics {
    fn clone(&self) -> Self {
        Self {
            total_submissions: self.total_submissions,
            successful_inclusions: self.successful_inclusions,
            total_profit: self.total_profit,
            avg_submission_time_ms: self.avg_submission_time_ms,
            relay_performance: self.relay_performance.clone(),
        }
    }
}

impl Default for MEVBoostConfig {
    fn default() -> Self {
        Self {
            max_bid_value: U256::from(1_000_000_000_000_000_000u64), // 1 ETH
            min_bid_value: U256::from(1_000_000_000_000_000u64),     // 0.001 ETH
            submission_timeout: Duration::from_secs(2),
            max_concurrent_submissions: 5,
            enable_multi_relay: true,
            auto_relay_selection: true,
            bid_strategy: BidStrategy::Adaptive,
        }
    }
}

impl BlockBuilder {
    /// 새로운 블록 빌더 생성
    pub fn new(
        mev_boost_client: Arc<MEVBoostClient>,
        blockchain_client: Arc<BlockchainClient>,
    ) -> Self {
        Self {
            mev_boost_client,
            blockchain_client,
            current_block_template: Arc::new(Mutex::new(None)),
            bundle_queue: Arc::new(Mutex::new(Vec::new())),
            optimization_engine: OptimizationEngine::new(),
            gas_limit_manager: GasLimitManager::new(),
        }
    }

    /// 번들을 블록 빌더 큐에 추가
    pub async fn add_bundle_to_queue(&self, bundle: Bundle) -> Result<()> {
        let mut queue = self.bundle_queue.lock().await;
        queue.push(bundle);
        
        // 우선순위 정렬
        queue.sort_by(|a, b| b.priority_score().partial_cmp(&a.priority_score()).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(())
    }

    /// 블록 템플릿 생성
    pub async fn create_block_template(&self, target_block: u64) -> Result<BlockTemplate> {
        info!("🏗️ 블록 템플릿 생성: {}", target_block);

        let parent_block = self.blockchain_client.get_block(target_block - 1).await?;
        let base_fee = self.gas_limit_manager.get_base_fee().await?;

        let mut template = BlockTemplate {
            parent_hash: parent_block.and_then(|b| b.hash).unwrap_or_default(),
            block_number: target_block,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            base_fee,
            gas_limit: self.gas_limit_manager.target_gas_limit,
            coinbase: Address::zero(), // 빌더 주소
            transactions: Vec::new(),
            total_gas_used: 0,
            total_fees: U256::zero(),
            mev_value: U256::zero(),
        };

        // 번들 큐에서 트랜잭션 추가
        let mut queue = self.bundle_queue.lock().await;
        let mut remaining_gas = template.gas_limit;

        for bundle in queue.iter() {
            if bundle.target_block == target_block {
                let bundle_gas = bundle.total_gas_limit().as_u64();
                if remaining_gas >= bundle_gas {
                    template.transactions.extend(bundle.transactions.clone());
                    template.total_gas_used += bundle_gas;
                    template.mev_value += bundle.metadata.expected_profit;
                    remaining_gas -= bundle_gas;
                }
            }
        }

        // 최적화 적용
        template = self.optimization_engine.optimize_block_template(template).await?;

        // 현재 템플릿 저장
        let mut current_template = self.current_block_template.lock().await;
        *current_template = Some(template.clone());

        info!("✅ 블록 템플릿 생성 완료");
        info!("  📊 트랜잭션: {}", template.transactions.len());
        info!("  ⛽ 가스 사용: {}/{}", template.total_gas_used, template.gas_limit);
        info!("  💰 MEV 가치: {} ETH", format_eth_amount(template.mev_value));

        Ok(template)
    }

    /// 현재 블록 템플릿 조회
    pub async fn get_current_template(&self) -> Option<BlockTemplate> {
        let template = self.current_block_template.lock().await;
        template.clone()
    }
}

impl OptimizationEngine {
    fn new() -> Self {
        let optimization_strategies = vec![
            BlockOptimizationStrategy {
                name: "max_value".to_string(),
                strategy_type: OptimizationStrategyType::MaxValue,
                weight: 1.0,
                enabled: true,
            },
            BlockOptimizationStrategy {
                name: "gas_efficiency".to_string(),
                strategy_type: OptimizationStrategyType::MaxGasEfficiency,
                weight: 0.8,
                enabled: true,
            },
        ];

        Self {
            optimization_strategies,
        }
    }

    async fn optimize_block_template(&self, mut template: BlockTemplate) -> Result<BlockTemplate> {
        debug!("⚡ 블록 템플릿 최적화");

        // 트랜잭션 순서 최적화
        template.transactions.sort_by(|a, b| {
            let gas_price_a = a.gas_price.unwrap_or_default();
            let gas_price_b = b.gas_price.unwrap_or_default();
            gas_price_b.cmp(&gas_price_a) // 높은 가스 가격 우선
        });

        // 총 수수료 계산
        template.total_fees = template.transactions.iter()
            .map(|tx| {
                let gas_price = tx.gas_price.unwrap_or_default();
                gas_price * tx.gas
            })
            .fold(U256::zero(), |acc, x| acc + x);

        Ok(template)
    }
}

impl GasLimitManager {
    fn new() -> Self {
        Self {
            target_gas_limit: 30_000_000, // 30M gas
            max_gas_limit: 30_000_000,
            reserved_gas: 100_000, // 예약 가스
            gas_price_oracle: GasPriceOracle::new(),
        }
    }

    async fn get_base_fee(&self) -> Result<U256> {
        self.gas_price_oracle.get_current_base_fee().await
    }
}

impl GasPriceOracle {
    fn new() -> Self {
        Self {
            base_fee_cache: None,
            priority_fee_cache: None,
            cache_timestamp: None,
            cache_ttl: Duration::from_secs(12), // 1 블록
        }
    }

    async fn get_current_base_fee(&self) -> Result<U256> {
        // 캐시된 값이 유효하면 반환
        if let Some(cached_fee) = self.base_fee_cache {
            if let Some(timestamp) = self.cache_timestamp {
                if SystemTime::now().duration_since(timestamp).unwrap_or_default() < self.cache_ttl {
                    return Ok(cached_fee);
                }
            }
        }

        // 기본값 반환 (실제로는 네트워크에서 조회)
        Ok(U256::from(20_000_000_000u64)) // 20 gwei
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
    fn test_relay_endpoint_creation() {
        let relays = MEVBoostClient::default_relay_endpoints();
        assert!(!relays.is_empty());
        assert!(relays.iter().any(|r| r.name == "Flashbots"));
    }

    #[test]
    fn test_bid_strategy_conservative() {
        let config = MEVBoostConfig {
            bid_strategy: BidStrategy::Conservative,
            ..Default::default()
        };
        assert!(matches!(config.bid_strategy, BidStrategy::Conservative));
    }

    #[test]
    fn test_submission_status() {
        let status = SubmissionStatus::Accepted;
        assert_eq!(status, SubmissionStatus::Accepted);
        assert_ne!(status, SubmissionStatus::Submitted);
    }
}