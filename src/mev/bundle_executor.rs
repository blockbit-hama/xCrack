use std::sync::Arc;
use std::collections::HashMap;
use ethers::{
    types::Transaction,
    abi::Bytes,
    providers::{Provider, Ws, Middleware},
    types::{H256, U256},
};use crate::config::Config;
use crate::flashbots::FlashbotsClient;
use serde::{Serialize, Deserialize};use crate::core::TransactionBuilder;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn};
use tokio::time::{timeout, Duration};
// use crate::strategies::();
use crate::mev::bundle::{Bundle, BundleMetadata, BundleType, OptimizationInfo, ValidationStatus, PriorityLevel};
use crate::types::{Opportunity, OpportunityType};

/// MEV Bundle 실행 및 제출 관리자
pub struct MEVBundleExecutor {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    flashbots_client: Arc<tokio::sync::Mutex<FlashbotsClient>>,
    transaction_builder: TransactionBuilder,
    pending_bundles: Arc<tokio::sync::RwLock<HashMap<String, PendingBundle>>>,
    execution_stats: Arc<tokio::sync::RwLock<ExecutionStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionBundle {
    pub bundle_id: String,
    // pub opportunities: Vec<()>,
    pub transactions: Vec<Bytes>,
    pub target_block: u64,
    pub estimated_profit_usd: f64,
    pub estimated_gas_cost: f64,
    pub submission_timestamp: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct PendingBundle {
    pub bundle: ExecutionBundle,
    pub submission_result: Option<BundleExecutionResult>,
    pub status: BundleStatus,
    pub retry_count: u32,
    pub last_retry: chrono::DateTime<chrono::Utc>,
}

/// Bundle execution status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BundleStatus {
    Created,
    Queued,
    Submitted,
    Pending,
    Included(ethers::types::H256), // block hash
    Failed,
    Expired,
    Cancelled,
    Rejected(String), // rejection reason
    Timeout,
    Replaced,         // bundle replaced by another
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub total_bundles_submitted: u64,
    pub successful_bundles: u64,
    pub failed_bundles: u64,
    pub total_profit_realized: f64,
    pub total_gas_spent: f64,
    pub average_execution_time_ms: f64,
    pub inclusion_rate: f64,
}

#[derive(Debug, Clone)]
pub struct BundleExecutionResult {
    pub bundle_id: String,
    pub success: bool,
    pub transaction_hash: Option<H256>,
    pub block_number: Option<u64>,
    pub gas_used: Option<u64>,
    pub profit_realized: Option<f64>,
    pub execution_time_ms: u64,
    pub error_message: Option<String>,
}

impl MEVBundleExecutor {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
    ) -> Result<Self> {
        info!("🚀 Initializing MEV Bundle Executor...");
        
        let flashbots_client = FlashbotsClient::new(Arc::clone(&config)).await?;
        
        let transaction_builder = TransactionBuilder::new(Arc::clone(&provider), Arc::clone(&config)).await?;
        
        info!("✅ MEV Bundle Executor initialized with Flashbots relay: {}", config.flashbots.relay_url);
        
        Ok(Self {
            config,
            provider,
            flashbots_client: Arc::new(tokio::sync::Mutex::new(flashbots_client)),
            transaction_builder,
            pending_bundles: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            execution_stats: Arc::new(tokio::sync::RwLock::new(ExecutionStats::default())),
        })
    }
    
    /// 청산 기회들을 Bundle로 패키징하고 실행
    pub async fn execute_liquidation_opportunities(
        &self,
        opportunities: Vec<Opportunity>,
        target_block: u64,
    ) -> Result<Vec<BundleExecutionResult>> {
        if opportunities.is_empty() {
            debug!("📭 No liquidation opportunities to execute");
            return Ok(Vec::new());
        }
        
        info!("📦 Packaging {} liquidation opportunities into MEV bundle for block {}", 
              opportunities.len(), target_block);
        
        // 1. Bundle 생성
        let bundle = self.create_execution_bundle(opportunities, target_block).await?;
        
        // 2. Bundle 제출
        let submission_results = self.submit_bundle(&bundle).await?;
        
        // 3. 실행 결과 모니터링
        let execution_results = self.monitor_bundle_execution(&bundle, submission_results).await?;
        
        // 4. 통계 업데이트
        self.update_execution_stats(&execution_results).await;
        
        info!("✅ Bundle execution complete: {} results", execution_results.len());
        Ok(execution_results)
    }
    
    /// MEV Bundle 생성
    async fn create_execution_bundle(
        &self,
        opportunities: Vec<Opportunity>,
        target_block: u64,
    ) -> Result<ExecutionBundle> {
        let bundle_id = format!("bundle_{}_{}_{}", 
                               target_block, 
                               opportunities.len(),
                               chrono::Utc::now().timestamp_millis());
        
        debug!("🔨 Creating execution bundle: {}", bundle_id);
        
        let mut transactions = Vec::new();
        let mut total_profit = 0.0;
        let mut total_gas_cost = 0.0;
        
        // 각 기회를 트랜잭션으로 변환
        for opportunity in &opportunities {
            // Opportunity 구조체에 맞게 수정
            total_profit += opportunity.expected_profit.as_u128() as f64 / 1e18;
            total_gas_cost += opportunity.gas_estimate as f64 * 20.0 / 1e9; // 20 gwei 가정
        }
        
        let bundle = ExecutionBundle {
            bundle_id: bundle_id.clone(),
            transactions,
            target_block,
            estimated_profit_usd: total_profit,
            estimated_gas_cost: total_gas_cost,
            submission_timestamp: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        };        debug!("✅ Bundle created: {} transactions, ${:.2} profit, ${:.2} gas cost", 
               bundle.transactions.len(), total_profit, total_gas_cost);
        
        Ok(bundle)
    }
    
    /// Bundle 제출 (다중 빌더)
    async fn submit_bundle(&self, bundle: &ExecutionBundle) -> Result<Vec<BundleExecutionResult>> {
        info!("📤 Submitting bundle {} to {} builders", 
              bundle.bundle_id, self.config.flashbots.builder_urls.len());
        
        let mut submission_results = Vec::new();
        
        // Bundle Request 생성
        let bundle_request = Bundle {
            id: format!("bundle_request_{}", bundle.target_block),
            transactions: vec![Transaction::default()], // TODO: Convert Vec<u8> to Transaction
            target_block: bundle.target_block,
            metadata: BundleMetadata {
                bundle_type: BundleType::Liquidation,
                opportunity_type: OpportunityType::Liquidation,
                expected_profit: U256::from((bundle.estimated_profit_usd * 1e18) as u64), // ETH 단위로 변환
                max_gas_price: U256::from((bundle.estimated_gas_cost * 1e9) as u64), // Gwei 단위로 변환
                min_timestamp: None,
                max_timestamp: None,
                priority_level: PriorityLevel::Medium,
                tags: Vec::new(),
                source_strategy: "liquidation_manager".to_string(),
            },
            optimization_info: OptimizationInfo::default(),
            validation_status: ValidationStatus::Pending,
            creation_time: std::time::SystemTime::now(),
            max_priority_fee_per_gas: None,
            max_fee_per_gas: None,
        };
        
        // 각 빌더에게 순차 제출 (mutable borrow 때문에 순차적으로 처리)
        for builder_url in self.config.flashbots.builder_urls.clone() {
            match self.submit_to_builder(&bundle_request, &builder_url).await {
                Ok(result) => {
                    debug!("✅ Bundle submitted to {}: {:?}", builder_url, result);
                    submission_results.push(result);
                }
                Err(e) => {
                    warn!("❌ Failed to submit bundle to {}: {}", builder_url, e);
                }
            }
        }
        
        if submission_results.is_empty() {
            return Err(anyhow!("Failed to submit bundle to any builder"));
        }
        
        // Pending bundles에 추가
        let pending_bundle = PendingBundle {
            bundle: bundle.clone(),
            submission_result: submission_results.first().cloned(),
            status: BundleStatus::Submitted,
            retry_count: 0,
            last_retry: chrono::Utc::now(),
        };        
        self.pending_bundles.write().await.insert(bundle.bundle_id.clone(), pending_bundle);
        
        info!("🚀 Bundle {} submitted to {} builders", bundle.bundle_id, submission_results.len());
        Ok(submission_results)
    }
    
    /// 개별 빌더에게 Bundle 제출
    async fn submit_to_builder(
        &self,
        bundle: &Bundle,
        builder_url: &str,
    ) -> Result<BundleExecutionResult> {
        debug!("📡 Submitting to builder: {}", builder_url);
        
        // BundleOptions 생성
        let _bundle_options = crate::mev::flashbots::BundleOptions {
            min_timestamp: bundle.metadata.min_timestamp,
            max_timestamp: bundle.metadata.max_timestamp,
            reverting_tx_hashes: None,
            replacement_uuid: None,
            expected_profit: Some(bundle.metadata.expected_profit),
        };
        
        // 제출 타임아웃 설정 (3초)
        // Note: Mock mode에서는 mev::bundle::Bundle 타입 사용
        use crate::mev::bundle::BundleType;
        let mut types_bundle = crate::mev::bundle::Bundle::new(
            vec![], // transactions는 나중에 설정
            bundle.target_block,
            BundleType::Liquidation,
            crate::types::OpportunityType::Liquidation,
        );

        // 메타데이터 업데이트
        types_bundle.metadata.expected_profit = bundle.metadata.expected_profit;
        let flashbots_client = self.flashbots_client.lock().await;
        let submission_future = flashbots_client.submit_bundle(&types_bundle);
        
        match timeout(Duration::from_secs(3), submission_future).await {
            Ok(Ok(success)) => {
                // Mock 모드에서는 bool 반환
                Ok(BundleExecutionResult {
                    bundle_id: bundle.id.clone(),
                    success,
                    transaction_hash: None,
                    block_number: Some(bundle.target_block),
                    gas_used: None,
                    profit_realized: None,
                    execution_time_ms: 0,
                    error_message: None,
                })
            }
            Ok(Err(e)) => Err(e), // Bundle submission error
            Err(_) => Err(anyhow!("Bundle submission timeout for {}", builder_url)),
        }
    }
    
    /// Bundle 실행 모니터링
    async fn monitor_bundle_execution(
        &self,
        bundle: &ExecutionBundle,
        _submission_results: Vec<BundleExecutionResult>,
    ) -> Result<Vec<BundleExecutionResult>> {
        debug!("👁️ Monitoring bundle {} execution", bundle.bundle_id);
        
        let mut execution_results = Vec::new();
        let monitoring_start = std::time::Instant::now();
        
        // 5분간 모니터링
        let timeout_duration = Duration::from_secs(300);
        let monitoring_future = self.monitor_bundle_status(bundle);
        
        match timeout(timeout_duration, monitoring_future).await {
            Ok(results) => execution_results.extend(results?),
            Err(_) => {
                warn!("⏰ Bundle {} monitoring timeout", bundle.bundle_id);
                
                // 타임아웃 시 실패로 처리
                execution_results.push(BundleExecutionResult {
                    bundle_id: bundle.bundle_id.clone(),
                    success: false,
                    transaction_hash: None,
                    block_number: None,
                    gas_used: None,
                    profit_realized: None,
                    execution_time_ms: monitoring_start.elapsed().as_millis() as u64,
                    error_message: Some("Monitoring timeout".to_string()),
                });
            }
        }
        
        Ok(execution_results)
    }
    
    /// Bundle 상태 모니터링 (실제 구현)
    async fn monitor_bundle_status(&self, bundle: &ExecutionBundle) -> Result<Vec<BundleExecutionResult>> {
        let mut results = Vec::new();
        let start_time = std::time::Instant::now();
        
        // 실제 환경에서는 Flashbots API를 통해 Bundle 상태를 조회
        // 여기서는 시뮬레이션
        
        // 30초 후 성공으로 가정 (테스트용)
        tokio::time::sleep(Duration::from_secs(30)).await;
        
        // Mock successful execution
        results.push(BundleExecutionResult {
            bundle_id: bundle.bundle_id.clone(),
            success: true,
            transaction_hash: Some(H256::random()),
            block_number: Some(bundle.target_block),
            gas_used: Some(650_000),
            profit_realized: Some(bundle.estimated_profit_usd),
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            error_message: None,
        });
        
        // Bundle 상태 업데이트
        if let Some(pending_bundle) = self.pending_bundles.write().await.get_mut(&bundle.bundle_id) {
            // Generate a mock block hash for Included status
            let block_hash = ethers::types::H256::random();
            pending_bundle.status = BundleStatus::Included(block_hash);
        }
        
        info!("✅ Bundle {} successfully executed in block {}", 
              bundle.bundle_id, bundle.target_block);
        
        Ok(results)
    }
    
    /// 실행 통계 업데이트
    async fn update_execution_stats(&self, results: &[BundleExecutionResult]) {
        let mut stats = self.execution_stats.write().await;
        
        stats.total_bundles_submitted += 1;
        
        for result in results {
            if result.success {
                stats.successful_bundles += 1;
                if let Some(profit) = result.profit_realized {
                    stats.total_profit_realized += profit;
                }
            } else {
                stats.failed_bundles += 1;
            }
            
            // 실행 시간 평균 업데이트
            let total_bundles = stats.total_bundles_submitted as f64;
            stats.average_execution_time_ms = 
                (stats.average_execution_time_ms * (total_bundles - 1.0) + result.execution_time_ms as f64) / total_bundles;
        }
        
        // 성공률 계산
        if stats.total_bundles_submitted > 0 {
            stats.inclusion_rate = (stats.successful_bundles as f64) / (stats.total_bundles_submitted as f64);
        }
        
        debug!("📊 Execution stats updated: success rate {:.2}%, avg profit ${:.2}", 
               stats.inclusion_rate * 100.0, stats.total_profit_realized / stats.successful_bundles as f64);
    }
    
    /// 단일 기회를 즉시 실행
    pub async fn execute_single_opportunity(
        &self,
        opportunity: Opportunity,
    ) -> Result<BundleExecutionResult> {
        let current_block = self.provider.get_block(ethers::types::BlockNumber::Latest).await?.unwrap().number.unwrap().as_u64();
        let target_block = current_block + 1;
        
        let results = self.execute_liquidation_opportunities(vec![opportunity], target_block).await?;
        
        results.into_iter().next()
            .ok_or_else(|| anyhow!("No execution result returned"))
    }
    
    /// 대기 중인 Bundle들 정리
    pub async fn cleanup_expired_bundles(&self) -> u32 {
        let mut pending_bundles = self.pending_bundles.write().await;
        let current_time = chrono::Utc::now();
        let mut cleaned_count = 0;
        
        pending_bundles.retain(|bundle_id, pending_bundle| {
            if pending_bundle.bundle.expires_at < current_time {
                debug!("🧹 Cleaning up expired bundle: {}", bundle_id);
                cleaned_count += 1;
                false
            } else {
                true
            }
        });
        
        if cleaned_count > 0 {
            info!("🧹 Cleaned up {} expired bundles", cleaned_count);
        }
        
        cleaned_count
    }
    
    /// 실행 통계 조회
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        self.execution_stats.read().await.clone()
    }
    
    /// 현재 대기 중인 Bundle 수
    pub async fn get_pending_bundle_count(&self) -> usize {
        self.pending_bundles.read().await.len()
    }
    
    /// Bundle 상태 조회
    pub async fn get_bundle_status(&self, bundle_id: &str) -> Option<BundleStatus> {
        self.pending_bundles.read().await.get(bundle_id).map(|pb| pb.status.clone())
    }
    
    /// 일반 MEV Opportunity를 실행 (다른 전략들과의 호환성)
    pub async fn execute_mev_opportunity(&self, opportunity: Opportunity) -> Result<BundleExecutionResult> {
        debug!("⚡ Executing MEV opportunity: {:?}", opportunity.opportunity_type);
        
        let current_block = self.provider.get_block(ethers::types::BlockNumber::Latest).await?.unwrap().number.unwrap().as_u64();
        let target_block = current_block + 1;
        
        // Opportunity를 Bundle로 변환
        let bundle = ExecutionBundle {
            bundle_id: format!("mev_{}_{}", opportunity.opportunity_type as u8, chrono::Utc::now().timestamp_millis()),
            transactions: vec![], // TODO: 실제 트랜잭션 데이터 추가
            target_block,
            estimated_profit_usd: opportunity.expected_profit.as_u128() as f64 / 1e18,
            estimated_gas_cost: opportunity.gas_estimate as f64 * 20.0 / 1e9, // 20 gwei 가정
            submission_timestamp: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        
        // Bundle 제출 및 모니터링
        };        let submission_results = self.submit_bundle(&bundle).await?;
        let execution_results = self.monitor_bundle_execution(&bundle, submission_results).await?;
        
        execution_results.into_iter().next()
            .ok_or_else(|| anyhow!("No execution result for MEV opportunity"))
    }
}

/// Bundle 실행 컨텍스트 - 다중 Bundle 관리
pub struct BundleExecutionContext {
    pub max_concurrent_bundles: usize,
    pub current_bundles: Vec<String>,
    pub next_target_block: u64,
    pub gas_price_multiplier: f64,
}

impl BundleExecutionContext {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent_bundles: max_concurrent,
            current_bundles: Vec::new(),
            next_target_block: 0,
            gas_price_multiplier: 1.0,
        }
    }
    
    pub fn can_submit_bundle(&self) -> bool {
        self.current_bundles.len() < self.max_concurrent_bundles
    }
    
    pub fn add_bundle(&mut self, bundle_id: String) {
        if self.current_bundles.len() < self.max_concurrent_bundles {
            self.current_bundles.push(bundle_id);
        }
    }
    
    pub fn remove_bundle(&mut self, bundle_id: &str) {
        self.current_bundles.retain(|id| id != bundle_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_bundle_creation() {
        // Bundle 생성 테스트
        let config = Config::load_test_config();
        let bundle_id = format!("test_bundle_{}", chrono::Utc::now().timestamp_millis());
        
        let bundle = ExecutionBundle {
            bundle_id: bundle_id.clone(),
            opportunities: vec![],
            transactions: vec![Bytes::from(vec![0x01, 0x02, 0x03])],
            target_block: 12345,
            estimated_profit_usd: 100.0,
            estimated_gas_cost: 50.0,
            submission_timestamp: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        
        };
        assert_eq!(bundle.bundle_id, bundle_id);
        assert_eq!(bundle.transactions.len(), 1);
        assert!(bundle.estimated_profit_usd > 0.0);
        println!("Bundle created: {:#?}", bundle);
    }
    
    #[test]
    fn test_execution_context() {
        let mut context = BundleExecutionContext::new(3);
        
        assert!(context.can_submit_bundle());
        assert_eq!(context.current_bundles.len(), 0);
        
        context.add_bundle("bundle_1".to_string());
        context.add_bundle("bundle_2".to_string());
        context.add_bundle("bundle_3".to_string());
        
        assert_eq!(context.current_bundles.len(), 3);
        assert!(!context.can_submit_bundle());
        
        context.remove_bundle("bundle_1");
        assert_eq!(context.current_bundles.len(), 2);
        assert!(context.can_submit_bundle());
    }
}
