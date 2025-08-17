use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::{RwLock, Mutex};
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use tracing::{info, debug, warn, error};
use serde::{Serialize, Deserialize};
use alloy::primitives::{U256, Address, TxHash};
use uuid::Uuid;

use crate::types::{ChainId, BridgeProtocol};
use super::transaction_monitor::BridgeTransactionMonitor;
use super::profit_verifier::CrossChainProfitVerifier;

/// 타겟 체인 실행 워크플로우 관리자
/// 
/// 브리지 완료 후 목적지 체인에서의 후속 거래들을 관리합니다.
/// - 브리지 완료 대기 및 확인
/// - 타겟 체인 거래 실행 계획
/// - 실행 순서 최적화 및 배치 처리
/// - 실패 처리 및 재시도 메커니즘
/// - 전체 워크플로우 모니터링
// Debug 파생은 dyn ChainExecutor 필드로 인해 불가
pub struct TargetChainExecutionManager {
    /// 브리지 트랜잭션 모니터
    bridge_monitor: Arc<BridgeTransactionMonitor>,
    
    /// 수익 검증기
    profit_verifier: Arc<CrossChainProfitVerifier>,
    
    /// 실행 대기 중인 워크플로우들
    pending_workflows: Arc<RwLock<HashMap<String, ExecutionWorkflow>>>,
    
    /// 활성 실행 중인 워크플로우들
    active_executions: Arc<RwLock<HashMap<String, ActiveExecution>>>,
    
    /// 완료된 워크플로우 히스토리
    completed_workflows: Arc<RwLock<Vec<CompletedWorkflow>>>,
    
    /// 실행 설정
    execution_config: ExecutionConfig,
    
    /// 체인별 실행 클라이언트
    chain_executors: Arc<RwLock<HashMap<ChainId, Arc<dyn ChainExecutor>>>>,
    
    /// 실행 통계
    execution_stats: Arc<RwLock<ExecutionStatistics>>,
}

/// 실행 워크플로우
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionWorkflow {
    /// 워크플로우 ID
    pub workflow_id: String,
    
    /// 연결된 브리지 실행 ID
    pub bridge_execution_id: String,
    
    /// 소스 체인 정보
    pub source_chain: ChainId,
    
    /// 타겟 체인 정보
    pub target_chain: ChainId,
    
    /// 브리지 프로토콜
    pub bridge_protocol: BridgeProtocol,
    
    /// 토큰 정보
    pub token_symbol: String,
    
    /// 브리지된 금액
    pub bridged_amount: U256,
    
    /// 브리지된 금액 (USD)
    pub bridged_amount_usd: f64,
    
    /// 실행 계획
    pub execution_plan: ExecutionPlan,
    
    /// 워크플로우 상태
    pub status: WorkflowStatus,
    
    /// 생성 시간
    pub created_at: DateTime<Utc>,
    
    /// 브리지 완료 예상 시간
    pub expected_bridge_completion: DateTime<Utc>,
    
    /// 실행 시작 시간
    pub execution_started_at: Option<DateTime<Utc>>,
    
    /// 완료 시간
    pub completed_at: Option<DateTime<Utc>>,
    
    /// 우선순위 (0-10, 높을수록 우선)
    pub priority: u8,
    
    /// 실행 조건들
    pub execution_conditions: Vec<ExecutionCondition>,
    
    /// 실패 횟수
    pub failure_count: u32,
    
    /// 마지막 오류 메시지
    pub last_error: Option<String>,
}

/// 실행 계획
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// 실행할 단계들
    pub steps: Vec<ExecutionStep>,
    
    /// 전체 예상 소요 시간 (초)
    pub estimated_duration: u64,
    
    /// 예상 가스 비용
    pub estimated_gas_cost: U256,
    
    /// 예상 가스 비용 (USD)
    pub estimated_gas_cost_usd: f64,
    
    /// 실행 전략
    pub execution_strategy: ExecutionStrategy,
    
    /// 배치 처리 설정
    pub batch_config: Option<BatchConfig>,
    
    /// 타임아웃 설정 (초)
    pub timeout_seconds: u64,
    
    /// 재시도 설정
    pub retry_config: RetryConfig,
}

/// 실행 단계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// 단계 ID
    pub step_id: String,
    
    /// 단계 유형
    pub step_type: ExecutionStepType,
    
    /// 실행 순서
    pub order: u32,
    
    /// 의존성 (이전에 완료되어야 할 단계들)
    pub dependencies: Vec<String>,
    
    /// 거래 데이터
    pub transaction_data: TransactionData,
    
    /// 예상 가스 사용량
    pub estimated_gas: U256,
    
    /// 실행 조건들
    pub conditions: Vec<StepCondition>,
    
    /// 실행 상태
    pub status: StepStatus,
    
    /// 실행 결과
    pub result: Option<StepResult>,
}

/// 거래 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    /// 대상 컨트랙트 주소
    pub to: Address,
    
    /// 거래 데이터
    pub data: Vec<u8>,
    
    /// 전송할 이더 양
    pub value: U256,
    
    /// 가스 한도
    pub gas_limit: U256,
    
    /// 가스 가격
    pub gas_price: Option<U256>,
    
    /// EIP-1559 fee 설정
    pub eip1559_fees: Option<Eip1559Fees>,
}

/// EIP-1559 수수료 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip1559Fees {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
}

/// 활성 실행 정보
#[derive(Debug, Clone)]
pub struct ActiveExecution {
    /// 워크플로우 정보
    pub workflow: ExecutionWorkflow,
    
    /// 현재 실행 중인 단계
    pub current_step: String,
    
    /// 실행된 거래 해시들
    pub transaction_hashes: Vec<TxHash>,
    
    /// 실행 시작 시간
    pub started_at: DateTime<Utc>,
    
    /// 예상 완료 시간
    pub estimated_completion: DateTime<Utc>,
    
    /// 실시간 가스 추적
    pub gas_tracker: GasTracker,
}

/// 완료된 워크플로우
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedWorkflow {
    /// 워크플로우 정보
    pub workflow: ExecutionWorkflow,
    
    /// 실행 결과
    pub execution_result: ExecutionResult,
    
    /// 총 소요 시간 (초)
    pub total_duration: f64,
    
    /// 실제 가스 비용
    pub actual_gas_cost: U256,
    
    /// 실제 가스 비용 (USD)
    pub actual_gas_cost_usd: f64,
    
    /// 모든 거래 해시들
    pub transaction_hashes: Vec<TxHash>,
    
    /// 수익 분석 결과
    pub profit_analysis: Option<ProfitAnalysisResult>,
}

/// 가스 추적기
#[derive(Debug, Clone)]
pub struct GasTracker {
    /// 누적 가스 사용량
    pub cumulative_gas_used: U256,
    
    /// 누적 가스 비용
    pub cumulative_gas_cost: U256,
    
    /// 현재 가스 가격
    pub current_gas_price: U256,
    
    /// 가스 가격 변화 추이
    pub gas_price_history: Vec<(DateTime<Utc>, U256)>,
}

/// 수익 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitAnalysisResult {
    /// 총 수익 (USD)
    pub total_profit_usd: f64,
    
    /// 가스 비용 (USD)
    pub gas_cost_usd: f64,
    
    /// 순수익 (USD)
    pub net_profit_usd: f64,
    
    /// 수익률 (%)
    pub profit_margin_percent: f64,
    
    /// ROI (%)
    pub roi_percent: f64,
    
    /// 분석 타임스탬프
    pub analyzed_at: DateTime<Utc>,
}

/// 워크플로우 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    /// 브리지 완료 대기 중
    WaitingForBridge,
    
    /// 실행 조건 확인 중
    CheckingConditions,
    
    /// 실행 대기 중 (큐에서 대기)
    Queued,
    
    /// 실행 중
    Executing,
    
    /// 일시 중지됨
    Paused,
    
    /// 성공 완료
    Completed,
    
    /// 실패로 종료
    Failed,
    
    /// 취소됨
    Cancelled,
    
    /// 타임아웃
    TimedOut,
}

/// 실행 단계 유형
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStepType {
    /// 토큰 스왑
    TokenSwap,
    
    /// 유동성 풀 참여
    AddLiquidity,
    
    /// 유동성 제거
    RemoveLiquidity,
    
    /// 대출 상환
    RepayLoan,
    
    /// 스테이킹
    Stake,
    
    /// 언스테이킹
    Unstake,
    
    /// NFT 구매
    BuyNFT,
    
    /// NFT 판매
    SellNFT,
    
    /// 거버넌스 투표
    Vote,
    
    /// 커스텀 컨트랙트 호출
    CustomCall,
    
    /// 토큰 전송
    Transfer,
    
    /// 멀티 호출
    Multicall,
}

/// 실행 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    /// 순차 실행 (하나씩 순서대로)
    Sequential,
    
    /// 병렬 실행 (동시에 여러 개)
    Parallel,
    
    /// 조건부 실행 (조건에 따라 선택)
    Conditional,
    
    /// 배치 실행 (여러 개를 하나의 거래로)
    Batch,
    
    /// 최적화된 실행 (가스 비용 최적화)
    Optimized,
}

/// 배치 처리 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// 최대 배치 크기
    pub max_batch_size: u32,
    
    /// 배치 타임아웃 (초)
    pub batch_timeout_seconds: u64,
    
    /// 배치 전략
    pub batch_strategy: BatchStrategy,
}

/// 배치 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchStrategy {
    /// 고정 크기 배치
    FixedSize,
    
    /// 동적 크기 배치 (가스 한도 기준)
    DynamicByGas,
    
    /// 시간 기반 배치
    TimeBased,
    
    /// 우선순위 기반 배치
    PriorityBased,
}

/// 재시도 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 최대 재시도 횟수
    pub max_retries: u32,
    
    /// 재시도 간격 (초)
    pub retry_interval_seconds: u64,
    
    /// 백오프 전략
    pub backoff_strategy: BackoffStrategy,
    
    /// 재시도 가능한 오류 유형들
    pub retryable_errors: Vec<RetryableErrorType>,
}

/// 백오프 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// 고정 간격
    Fixed,
    
    /// 지수적 증가
    Exponential { multiplier: f64 },
    
    /// 선형 증가
    Linear { increment: u64 },
    
    /// 랜덤 지터
    RandomJitter { base: u64, max_jitter: u64 },
}

/// 재시도 가능한 오류 유형
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryableErrorType {
    /// 네트워크 오류
    NetworkError,
    
    /// 가스 부족
    OutOfGas,
    
    /// 논스 오류
    NonceError,
    
    /// 일시적 실패
    TemporaryFailure,
    
    /// RPC 오류
    RpcError,
    
    /// 멤풀 거부
    MempoolRejection,
}

/// 실행 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionCondition {
    /// 브리지 완료 확인
    BridgeCompleted,
    
    /// 최소 잔액 확인
    MinimumBalance { token: String, amount: U256 },
    
    /// 가스 가격 임계값
    MaxGasPrice { max_price_gwei: u64 },
    
    /// 시간 조건
    TimeCondition { after: DateTime<Utc>, before: Option<DateTime<Utc>> },
    
    /// 시장 조건
    MarketCondition { condition: String },
    
    /// 커스텀 조건
    CustomCondition { condition_id: String, parameters: HashMap<String, String> },
}

/// 단계 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepCondition {
    /// 이전 단계 성공
    PreviousStepSuccess { step_id: String },
    
    /// 토큰 승인 확인
    TokenApproval { token: Address, spender: Address, amount: U256 },
    
    /// 충분한 가스
    SufficientGas { required_gas: U256 },
    
    /// 슬리피지 허용 범위
    SlippageTolerance { max_slippage_percent: f64 },
}

/// 단계 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepStatus {
    /// 대기 중
    Pending,
    
    /// 조건 확인 중
    CheckingConditions,
    
    /// 실행 중
    Executing,
    
    /// 성공 완료
    Completed,
    
    /// 실패
    Failed,
    
    /// 건너뜀
    Skipped,
}

/// 단계 실행 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// 거래 해시
    pub transaction_hash: Option<TxHash>,
    
    /// 사용된 가스
    pub gas_used: Option<U256>,
    
    /// 실제 가스 가격
    pub gas_price: Option<U256>,
    
    /// 실행 시간 (초)
    pub execution_time: f64,
    
    /// 성공 여부
    pub success: bool,
    
    /// 오류 메시지
    pub error_message: Option<String>,
    
    /// 추가 메타데이터
    pub metadata: HashMap<String, String>,
}

/// 실행 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// 성공
    Success {
        steps_completed: u32,
        total_steps: u32,
    },
    
    /// 부분 성공
    PartialSuccess {
        steps_completed: u32,
        total_steps: u32,
        failed_steps: Vec<String>,
    },
    
    /// 실패
    Failure {
        failed_step: String,
        error_message: String,
    },
    
    /// 취소됨
    Cancelled {
        reason: String,
    },
}

/// 실행 설정
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// 최대 동시 실행 워크플로우 수
    pub max_concurrent_workflows: u32,
    
    /// 워크플로우 큐 크기
    pub max_queue_size: u32,
    
    /// 기본 가스 가격 전략
    pub default_gas_strategy: GasStrategy,
    
    /// 실행 타임아웃 (초)
    pub default_timeout_seconds: u64,
    
    /// 모니터링 간격 (초)
    pub monitoring_interval_seconds: u64,
    
    /// 자동 재시도 활성화
    pub auto_retry_enabled: bool,
    
    /// 통계 수집 활성화
    pub statistics_enabled: bool,
}

/// 가스 전략
#[derive(Debug, Clone)]
pub enum GasStrategy {
    /// 빠른 실행 (높은 가스 가격)
    Fast,
    
    /// 표준 실행 (중간 가스 가격)
    Standard,
    
    /// 경제적 실행 (낮은 가스 가격)
    Economy,
    
    /// 동적 가격 (시장 상황에 따라)
    Dynamic,
    
    /// 커스텀 가격
    Custom { gas_price: U256 },
}

/// 실행 통계
#[derive(Debug, Clone, Default)]
pub struct ExecutionStatistics {
    /// 총 워크플로우 수
    pub total_workflows: u64,
    
    /// 성공한 워크플로우 수
    pub successful_workflows: u64,
    
    /// 실패한 워크플로우 수
    pub failed_workflows: u64,
    
    /// 평균 실행 시간 (초)
    pub avg_execution_time: f64,
    
    /// 총 가스 사용량
    pub total_gas_used: U256,
    
    /// 총 가스 비용 (USD)
    pub total_gas_cost_usd: f64,
    
    /// 평균 가스 가격 (gwei)
    pub avg_gas_price_gwei: f64,
    
    /// 마지막 업데이트 시간
    pub last_updated: DateTime<Utc>,
}

/// 체인 실행 클라이언트 트레이트
#[async_trait::async_trait]
pub trait ChainExecutor: Send + Sync {
    /// 체인 이름
    fn chain_id(&self) -> ChainId;
    
    /// 거래 실행
    async fn execute_transaction(&self, tx_data: &TransactionData) -> Result<TxHash>;
    
    /// 거래 상태 확인
    async fn get_transaction_status(&self, tx_hash: TxHash) -> Result<TransactionStatus>;
    
    /// 계정 잔액 조회
    async fn get_balance(&self, address: Address, token: Option<Address>) -> Result<U256>;
    
    /// 가스 가격 조회
    async fn get_gas_price(&self) -> Result<U256>;
    
    /// 논스 조회
    async fn get_nonce(&self, address: Address) -> Result<U256>;
    
    /// 거래 시뮬레이션
    async fn simulate_transaction(&self, tx_data: &TransactionData) -> Result<SimulationResult>;
}

/// 거래 상태
#[derive(Debug, Clone)]
pub enum TransactionStatus {
    /// 펜딩 중
    Pending,
    
    /// 성공
    Success { gas_used: U256, block_number: u64 },
    
    /// 실패
    Failed { error: String, block_number: u64 },
    
    /// 대체됨 (다른 거래로)
    Replaced { replaced_by: TxHash },
}

/// 시뮬레이션 결과
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// 성공 여부
    pub success: bool,
    
    /// 예상 가스 사용량
    pub estimated_gas: U256,
    
    /// 오류 메시지 (실패시)
    pub error_message: Option<String>,
    
    /// 반환 데이터
    pub return_data: Vec<u8>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_workflows: 10,
            max_queue_size: 100,
            default_gas_strategy: GasStrategy::Standard,
            default_timeout_seconds: 1800, // 30분
            monitoring_interval_seconds: 30,
            auto_retry_enabled: true,
            statistics_enabled: true,
        }
    }
}

impl TargetChainExecutionManager {
    /// 새로운 실행 관리자 생성
    pub fn new(
        bridge_monitor: Arc<BridgeTransactionMonitor>,
        profit_verifier: Arc<CrossChainProfitVerifier>,
    ) -> Self {
        Self {
            bridge_monitor,
            profit_verifier,
            pending_workflows: Arc::new(RwLock::new(HashMap::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            completed_workflows: Arc::new(RwLock::new(Vec::new())),
            execution_config: ExecutionConfig::default(),
            chain_executors: Arc::new(RwLock::new(HashMap::new())),
            execution_stats: Arc::new(RwLock::new(ExecutionStatistics::default())),
        }
    }
    
    /// 커스텀 설정으로 생성
    pub fn with_config(
        bridge_monitor: Arc<BridgeTransactionMonitor>,
        profit_verifier: Arc<CrossChainProfitVerifier>,
        config: ExecutionConfig,
    ) -> Self {
        Self {
            bridge_monitor,
            profit_verifier,
            pending_workflows: Arc::new(RwLock::new(HashMap::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            completed_workflows: Arc::new(RwLock::new(Vec::new())),
            execution_config: config,
            chain_executors: Arc::new(RwLock::new(HashMap::new())),
            execution_stats: Arc::new(RwLock::new(ExecutionStatistics::default())),
        }
    }
    
    /// 워크플로우 등록
    pub async fn register_workflow(&self, workflow: ExecutionWorkflow) -> Result<()> {
        let workflow_id = workflow.workflow_id.clone();
        
        // 기본 검증
        self.validate_workflow(&workflow).await?;
        
        // 큐에 추가
        let mut pending = self.pending_workflows.write().await;
        
        if pending.len() >= self.execution_config.max_queue_size as usize {
            return Err(anyhow::anyhow!("워크플로우 큐가 가득 참"));
        }
        
        pending.insert(workflow_id.clone(), workflow);
        
        info!("📋 워크플로우 등록 완료: {} (큐 크기: {})", workflow_id, pending.len());
        
        // 자동 처리 시작
        self.process_pending_workflows().await?;
        
        Ok(())
    }
    
    /// 워크플로우 검증
    async fn validate_workflow(&self, workflow: &ExecutionWorkflow) -> Result<()> {
        // 기본 필드 검증
        if workflow.workflow_id.is_empty() {
            return Err(anyhow::anyhow!("워크플로우 ID가 비어있음"));
        }
        
        if workflow.execution_plan.steps.is_empty() {
            return Err(anyhow::anyhow!("실행 단계가 없음"));
        }
        
        // 체인 실행기 존재 확인
        let executors = self.chain_executors.read().await;
        if !executors.contains_key(&workflow.target_chain) {
            return Err(anyhow::anyhow!("타겟 체인 실행기가 등록되지 않음: {}", workflow.target_chain.name()));
        }
        
        // 실행 계획 검증
        self.validate_execution_plan(&workflow.execution_plan).await?;
        
        Ok(())
    }
    
    /// 실행 계획 검증
    async fn validate_execution_plan(&self, plan: &ExecutionPlan) -> Result<()> {
        // 단계 순서 검증
        let mut orders: Vec<u32> = plan.steps.iter().map(|s| s.order).collect();
        orders.sort();
        
        for (i, order) in orders.iter().enumerate() {
            if *order != i as u32 + 1 {
                return Err(anyhow::anyhow!("실행 단계 순서가 올바르지 않음"));
            }
        }
        
        // 의존성 검증
        for step in &plan.steps {
            for dep in &step.dependencies {
                if !plan.steps.iter().any(|s| s.step_id == *dep) {
                    return Err(anyhow::anyhow!("의존성 단계가 존재하지 않음: {}", dep));
                }
            }
        }
        
        Ok(())
    }
    
    /// 펜딩 워크플로우 처리
    async fn process_pending_workflows(&self) -> Result<()> {
        let mut pending = self.pending_workflows.write().await;
        let active = self.active_executions.read().await;
        
        // 동시 실행 한도 확인
        if active.len() >= self.execution_config.max_concurrent_workflows as usize {
            return Ok(());
        }
        
        // 실행 가능한 워크플로우 찾기
        let mut ready_workflows = Vec::new();
        
        for (workflow_id, workflow) in pending.iter() {
            if self.is_workflow_ready(workflow).await? {
                ready_workflows.push(workflow_id.clone());
            }
        }
        
        // 우선순위 순으로 정렬
        ready_workflows.sort_by(|a, b| {
            let priority_a = pending.get(a).map(|w| w.priority).unwrap_or(0);
            let priority_b = pending.get(b).map(|w| w.priority).unwrap_or(0);
            priority_b.cmp(&priority_a) // 높은 우선순위가 먼저
        });
        
        // 실행 시작
        let available_slots = self.execution_config.max_concurrent_workflows as usize - active.len();
        let workflows_to_start = ready_workflows.into_iter().take(available_slots);
        
        drop(active); // 락 해제
        
        for workflow_id in workflows_to_start {
            if let Some(workflow) = pending.remove(&workflow_id) {
                self.start_workflow_execution(workflow).await?;
            }
        }
        
        Ok(())
    }
    
    /// 워크플로우 실행 준비 확인
    async fn is_workflow_ready(&self, workflow: &ExecutionWorkflow) -> Result<bool> {
        // 브리지 완료 확인
        if workflow.status == WorkflowStatus::WaitingForBridge {
            let bridge_status = self.bridge_monitor
                .get_transaction_status(&workflow.bridge_execution_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("no status"))?;
                
            if !bridge_status.status.is_completed() {
                return Ok(false);
            }
        }
        
        // 실행 조건 확인
        for condition in &workflow.execution_conditions {
            if !self.check_execution_condition(condition, workflow).await? {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// 실행 조건 확인
    async fn check_execution_condition(
        &self,
        condition: &ExecutionCondition,
        workflow: &ExecutionWorkflow,
    ) -> Result<bool> {
        match condition {
            ExecutionCondition::BridgeCompleted => {
                let status = self.bridge_monitor
                    .get_transaction_status(&workflow.bridge_execution_id)
                    .await
                    .ok_or_else(|| anyhow::anyhow!("no status"))?;
                Ok(status.status.is_completed())
            }
            ExecutionCondition::MinimumBalance { token: _token, amount: _amount } => {
                // TODO: 실제 잔액 확인 로직 구현
                Ok(true)
            }
            ExecutionCondition::MaxGasPrice { max_price_gwei } => {
                let executors = self.chain_executors.read().await;
                if let Some(executor) = executors.get(&workflow.target_chain) {
                    let current_gas_price = executor.get_gas_price().await?;
                    let current_gwei = current_gas_price.to::<u64>() / 1_000_000_000;
                    Ok(current_gwei <= *max_price_gwei)
                } else {
                    Ok(false)
                }
            }
            ExecutionCondition::TimeCondition { after, before } => {
                let now = Utc::now();
                let after_check = now >= *after;
                let before_check = before.map(|b| now <= b).unwrap_or(true);
                Ok(after_check && before_check)
            }
            ExecutionCondition::MarketCondition { condition: _condition } => {
                // TODO: 시장 조건 확인 로직 구현
                Ok(true)
            }
            ExecutionCondition::CustomCondition { condition_id: _condition_id, parameters: _parameters } => {
                // TODO: 커스텀 조건 확인 로직 구현
                Ok(true)
            }
        }
    }
    
    /// 워크플로우 실행 시작
    async fn start_workflow_execution(&self, mut workflow: ExecutionWorkflow) -> Result<()> {
        let workflow_id = workflow.workflow_id.clone();
        
        workflow.status = WorkflowStatus::Executing;
        workflow.execution_started_at = Some(Utc::now());
        
        let active_execution = ActiveExecution {
            workflow: workflow.clone(),
            current_step: workflow.execution_plan.steps.first()
                .map(|s| s.step_id.clone())
                .unwrap_or_default(),
            transaction_hashes: Vec::new(),
            started_at: Utc::now(),
            estimated_completion: Utc::now() + ChronoDuration::seconds(workflow.execution_plan.estimated_duration as i64),
            gas_tracker: GasTracker {
                cumulative_gas_used: U256::ZERO,
                cumulative_gas_cost: U256::ZERO,
                current_gas_price: U256::ZERO,
                gas_price_history: Vec::new(),
            },
        };
        
        let mut active = self.active_executions.write().await;
        active.insert(workflow_id.clone(), active_execution);
        
        info!("🚀 워크플로우 실행 시작: {} ({}개 단계)", workflow_id, workflow.execution_plan.steps.len());
        
        // 비동기로 실행 (별도 태스크에서)
        let manager = Arc::new(self.clone());
        tokio::spawn(async move {
            if let Err(e) = manager.execute_workflow_steps(workflow_id.clone()).await {
                error!("❌ 워크플로우 실행 실패: {} - {}", workflow_id, e);
                let _ = manager.handle_workflow_failure(workflow_id, e.to_string()).await;
            }
        });
        
        Ok(())
    }
    
    /// 워크플로우 단계별 실행
    async fn execute_workflow_steps(&self, workflow_id: String) -> Result<()> {
        loop {
            let (current_step_id, execution_strategy) = {
                let active = self.active_executions.read().await;
                let execution = active.get(&workflow_id)
                    .ok_or_else(|| anyhow::anyhow!("활성 실행을 찾을 수 없음"))?;
                
                (execution.current_step.clone(), execution.workflow.execution_plan.execution_strategy.clone())
            };
            
            if current_step_id.is_empty() {
                // 모든 단계 완료
                self.complete_workflow(workflow_id).await?;
                break;
            }
            
            // 실행 전략에 따른 단계 실행
            match execution_strategy {
                ExecutionStrategy::Sequential => {
                    self.execute_step_sequential(&workflow_id, &current_step_id).await?;
                }
                ExecutionStrategy::Parallel => {
                    self.execute_step_parallel(&workflow_id).await?;
                }
                ExecutionStrategy::Batch => {
                    self.execute_step_batch(&workflow_id).await?;
                }
                _ => {
                    // 기본적으로 순차 실행
                    self.execute_step_sequential(&workflow_id, &current_step_id).await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// 순차 단계 실행
    async fn execute_step_sequential(&self, workflow_id: &str, step_id: &str) -> Result<()> {
        // 단계 정보 가져오기
        let (step, chain_id) = {
            let active = self.active_executions.read().await;
            let execution = active.get(workflow_id)
                .ok_or_else(|| anyhow::anyhow!("활성 실행을 찾을 수 없음"))?;
            
            let step = execution.workflow.execution_plan.steps.iter()
                .find(|s| s.step_id == step_id)
                .ok_or_else(|| anyhow::anyhow!("단계를 찾을 수 없음: {}", step_id))?
                .clone();
                
            (step, execution.workflow.target_chain)
        };
        
        // 단계 조건 확인
        for condition in &step.conditions {
            if !self.check_step_condition(condition, workflow_id).await? {
                warn!("⚠️ 단계 조건 미충족: {} - {:?}", step_id, condition);
                return Err(anyhow::anyhow!("단계 조건 미충족"));
            }
        }
        
        // 체인 실행기로 거래 실행
        let executors = self.chain_executors.read().await;
        let executor = executors.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("체인 실행기를 찾을 수 없음"))?;
        
        info!("⚡ 단계 실행 시작: {}", step_id);
        let start_time = std::time::Instant::now();
        
        // 거래 시뮬레이션 (선택적)
        if let Ok(sim_result) = executor.simulate_transaction(&step.transaction_data).await {
            if !sim_result.success {
                return Err(anyhow::anyhow!("거래 시뮬레이션 실패: {}", 
                    sim_result.error_message.unwrap_or_default()));
            }
        }
        
        // 실제 거래 실행
        let tx_hash = executor.execute_transaction(&step.transaction_data).await?;
        let execution_time = start_time.elapsed().as_secs_f64();
        
        // 거래 상태 모니터링
        let mut tx_status = TransactionStatus::Pending;
        let timeout = std::time::Instant::now() + std::time::Duration::from_secs(300); // 5분 타임아웃
        
        while std::time::Instant::now() < timeout {
            tx_status = executor.get_transaction_status(tx_hash).await?;
            
            match &tx_status {
                TransactionStatus::Success { gas_used, block_number: _ } => {
                    info!("✅ 단계 완료: {} (가스: {})", step_id, gas_used);
                    
                    // 실행 결과 업데이트
                    self.update_step_result(workflow_id, step_id, StepResult {
                        transaction_hash: Some(tx_hash),
                        gas_used: Some(*gas_used),
                        gas_price: Some(step.transaction_data.gas_price.unwrap_or_default()),
                        execution_time,
                        success: true,
                        error_message: None,
                        metadata: HashMap::new(),
                    }).await?;
                    
                    // 다음 단계로 이동
                    self.move_to_next_step(workflow_id, step_id).await?;
                    break;
                }
                TransactionStatus::Failed { error, block_number: _ } => {
                    error!("❌ 단계 실패: {} - {}", step_id, error);
                    
                    // 실행 결과 업데이트
                    self.update_step_result(workflow_id, step_id, StepResult {
                        transaction_hash: Some(tx_hash),
                        gas_used: None,
                        gas_price: Some(step.transaction_data.gas_price.unwrap_or_default()),
                        execution_time,
                        success: false,
                        error_message: Some(error.clone()),
                        metadata: HashMap::new(),
                    }).await?;
                    
                    return Err(anyhow::anyhow!("단계 실행 실패: {}", error));
                }
                TransactionStatus::Pending => {
                    // 계속 대기
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                TransactionStatus::Replaced { replaced_by } => {
                    warn!("🔄 거래 대체됨: {} -> {}", tx_hash, replaced_by);
                    // 대체된 거래로 계속 모니터링
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
        
        if matches!(tx_status, TransactionStatus::Pending) {
            return Err(anyhow::anyhow!("거래 타임아웃"));
        }
        
        Ok(())
    }
    
    /// 병렬 단계 실행 (구현 스텁)
    async fn execute_step_parallel(&self, _workflow_id: &str) -> Result<()> {
        // TODO: 병렬 실행 로직 구현
        Ok(())
    }
    
    /// 배치 단계 실행 (구현 스텁)
    async fn execute_step_batch(&self, _workflow_id: &str) -> Result<()> {
        // TODO: 배치 실행 로직 구현
        Ok(())
    }
    
    /// 단계 조건 확인
    async fn check_step_condition(&self, _condition: &StepCondition, _workflow_id: &str) -> Result<bool> {
        // TODO: 단계별 조건 확인 로직 구현
        Ok(true)
    }
    
    /// 단계 결과 업데이트
    async fn update_step_result(&self, workflow_id: &str, step_id: &str, result: StepResult) -> Result<()> {
        let mut active = self.active_executions.write().await;
        if let Some(execution) = active.get_mut(workflow_id) {
            // 해당 단계 찾아서 결과 업데이트
            let result_clone = result.clone();
            for step in &mut execution.workflow.execution_plan.steps {
                if step.step_id == step_id {
                    step.result = Some(result_clone.clone());
                    step.status = if result_clone.success { StepStatus::Completed } else { StepStatus::Failed };
                    break;
                }
            }
            
            // 거래 해시 추가
            if let Some(tx_hash) = result.transaction_hash.clone() {
                execution.transaction_hashes.push(tx_hash);
            }
            
            // 가스 추적 업데이트
            if let (Some(gas_used), Some(gas_price)) = (result.gas_used.clone(), result.gas_price.clone()) {
                execution.gas_tracker.cumulative_gas_used += gas_used;
                execution.gas_tracker.cumulative_gas_cost += gas_used * gas_price;
                execution.gas_tracker.current_gas_price = gas_price;
                execution.gas_tracker.gas_price_history.push((Utc::now(), gas_price));
            }
        }
        
        Ok(())
    }
    
    /// 다음 단계로 이동
    async fn move_to_next_step(&self, workflow_id: &str, current_step_id: &str) -> Result<()> {
        let mut active = self.active_executions.write().await;
        if let Some(execution) = active.get_mut(workflow_id) {
            // 현재 단계 순서 찾기
            let current_order = execution.workflow.execution_plan.steps.iter()
                .find(|s| s.step_id == current_step_id)
                .map(|s| s.order)
                .unwrap_or(0);
            
            // 다음 실행 가능한 단계 찾기
            let next_step = execution.workflow.execution_plan.steps.iter()
                .filter(|s| s.order > current_order && s.status == StepStatus::Pending)
                .min_by_key(|s| s.order);
            
            if let Some(next_step) = next_step {
                execution.current_step = next_step.step_id.clone();
                debug!("➡️ 다음 단계로 이동: {}", next_step.step_id);
            } else {
                execution.current_step = String::new(); // 모든 단계 완료
                debug!("🏁 모든 단계 완료");
            }
        }
        
        Ok(())
    }
    
    /// 워크플로우 완료
    async fn complete_workflow(&self, workflow_id: String) -> Result<()> {
        let (workflow, total_duration, total_gas_cost, transaction_hashes) = {
            let mut active = self.active_executions.write().await;
            let execution = active.remove(&workflow_id)
                .ok_or_else(|| anyhow::anyhow!("활성 실행을 찾을 수 없음"))?;
            
            let total_duration = (Utc::now() - execution.started_at).num_seconds() as f64;
            let total_gas_cost = execution.gas_tracker.cumulative_gas_cost;
            let transaction_hashes = execution.transaction_hashes.clone();
            
            (execution.workflow, total_duration, total_gas_cost, transaction_hashes)
        };
        
        // 수익 분석 실행
        let profit_analysis = self.analyze_workflow_profit(&workflow, &transaction_hashes).await.ok();
        
        // 완료된 워크플로우 기록
        let completed = CompletedWorkflow {
            workflow,
            execution_result: ExecutionResult::Success {
                steps_completed: transaction_hashes.len() as u32,
                total_steps: transaction_hashes.len() as u32,
            },
            total_duration,
            actual_gas_cost: total_gas_cost,
            actual_gas_cost_usd: 0.0, // TODO: USD 변환
            transaction_hashes,
            profit_analysis,
        };
        
        let mut completed_workflows = self.completed_workflows.write().await;
        completed_workflows.push(completed);
        
        // 통계 업데이트
        self.update_statistics(true, total_duration, total_gas_cost).await;
        
        info!("🎉 워크플로우 완료: {} ({:.1}초, {} gas)", workflow_id, total_duration, total_gas_cost);
        
        Ok(())
    }
    
    /// 워크플로우 실패 처리
    async fn handle_workflow_failure(&self, workflow_id: String, error_message: String) -> Result<()> {
        let mut active = self.active_executions.write().await;
        if let Some(mut execution) = active.remove(&workflow_id) {
            execution.workflow.status = WorkflowStatus::Failed;
            execution.workflow.last_error = Some(error_message.clone());
            execution.workflow.failure_count += 1;
            
            // 재시도 로직
            if self.execution_config.auto_retry_enabled && 
               execution.workflow.failure_count < execution.workflow.execution_plan.retry_config.max_retries {
                
                warn!("🔄 워크플로우 재시도: {} (시도 {}/{})", 
                      workflow_id, 
                      execution.workflow.failure_count,
                      execution.workflow.execution_plan.retry_config.max_retries);
                
                // 재시도 대기열에 추가
                let mut pending = self.pending_workflows.write().await;
                pending.insert(workflow_id.clone(), execution.workflow);
            } else {
                error!("💥 워크플로우 최종 실패: {} - {}", workflow_id, error_message);
                
                // 실패 통계 업데이트
                self.update_statistics(false, 0.0, U256::ZERO).await;
            }
        }
        
        Ok(())
    }
    
    /// 워크플로우 수익 분석
    async fn analyze_workflow_profit(
        &self,
        workflow: &ExecutionWorkflow,
        _transaction_hashes: &[TxHash],
    ) -> Result<ProfitAnalysisResult> {
        // TODO: 실제 수익 계산 로직 구현
        // 현재는 기본값 반환
        Ok(ProfitAnalysisResult {
            total_profit_usd: workflow.bridged_amount_usd,
            gas_cost_usd: 10.0, // 임시값
            net_profit_usd: workflow.bridged_amount_usd - 10.0,
            profit_margin_percent: 1.0,
            roi_percent: 1.0,
            analyzed_at: Utc::now(),
        })
    }
    
    /// 통계 업데이트
    async fn update_statistics(&self, success: bool, duration: f64, gas_cost: U256) {
        let mut stats = self.execution_stats.write().await;
        
        stats.total_workflows += 1;
        if success {
            stats.successful_workflows += 1;
        } else {
            stats.failed_workflows += 1;
        }
        
        // 평균 실행 시간 업데이트
        let total_successful = stats.successful_workflows as f64;
        if total_successful > 0.0 {
            stats.avg_execution_time = (stats.avg_execution_time * (total_successful - 1.0) + duration) / total_successful;
        }
        
        stats.total_gas_used += gas_cost;
        stats.last_updated = Utc::now();
    }
    
    /// 체인 실행기 등록
    pub async fn register_chain_executor(&self, chain_id: ChainId, executor: Arc<dyn ChainExecutor>) -> Result<()> {
        let mut executors = self.chain_executors.write().await;
        executors.insert(chain_id, executor);
        
        info!("🔧 체인 실행기 등록: {}", chain_id.name());
        
        Ok(())
    }
    
    /// 워크플로우 상태 조회
    pub async fn get_workflow_status(&self, workflow_id: &str) -> Option<WorkflowStatus> {
        // 펜딩 큐 확인
        {
            let pending = self.pending_workflows.read().await;
            if let Some(workflow) = pending.get(workflow_id) {
                return Some(workflow.status.clone());
            }
        }
        
        // 활성 실행 확인
        {
            let active = self.active_executions.read().await;
            if let Some(execution) = active.get(workflow_id) {
                return Some(execution.workflow.status.clone());
            }
        }
        
        // 완료된 워크플로우 확인
        {
            let completed = self.completed_workflows.read().await;
            if let Some(workflow) = completed.iter().find(|w| w.workflow.workflow_id == workflow_id) {
                return Some(workflow.workflow.status.clone());
            }
        }
        
        None
    }
    
    /// 실행 통계 조회
    pub async fn get_execution_statistics(&self) -> ExecutionStatistics {
        let stats = self.execution_stats.read().await;
        stats.clone()
    }
    
    /// 워크플로우 취소
    pub async fn cancel_workflow(&self, workflow_id: &str) -> Result<()> {
        // 펜딩 큐에서 제거
        {
            let mut pending = self.pending_workflows.write().await;
            if pending.remove(workflow_id).is_some() {
                info!("❌ 펜딩 워크플로우 취소: {}", workflow_id);
                return Ok(());
            }
        }
        
        // 활성 실행 취소
        {
            let mut active = self.active_executions.write().await;
            if let Some(mut execution) = active.remove(workflow_id) {
                execution.workflow.status = WorkflowStatus::Cancelled;
                info!("❌ 활성 워크플로우 취소: {}", workflow_id);
                return Ok(());
            }
        }
        
        Err(anyhow::anyhow!("워크플로우를 찾을 수 없음: {}", workflow_id))
    }
}

// Helper 트레이트들
impl Clone for TargetChainExecutionManager {
    fn clone(&self) -> Self {
        Self {
            bridge_monitor: Arc::clone(&self.bridge_monitor),
            profit_verifier: Arc::clone(&self.profit_verifier),
            pending_workflows: Arc::clone(&self.pending_workflows),
            active_executions: Arc::clone(&self.active_executions),
            completed_workflows: Arc::clone(&self.completed_workflows),
            execution_config: self.execution_config.clone(),
            chain_executors: Arc::clone(&self.chain_executors),
            execution_stats: Arc::clone(&self.execution_stats),
        }
    }
}

/// 브리지 트랜잭션 상태 확장 메서드
impl super::transaction_monitor::TransactionStatus {
    pub fn is_completed(&self) -> bool {
        matches!(self, 
            super::transaction_monitor::TransactionStatus::DestConfirmed |
            super::transaction_monitor::TransactionStatus::Failed)
    }
}

/// 간단한 실행 워크플로우 빌더
pub struct ExecutionWorkflowBuilder {
    workflow: ExecutionWorkflow,
}

impl ExecutionWorkflowBuilder {
    pub fn new(bridge_execution_id: String, target_chain: ChainId) -> Self {
        Self {
            workflow: ExecutionWorkflow {
                workflow_id: Uuid::new_v4().to_string(),
                bridge_execution_id,
                source_chain: ChainId::Ethereum, // 기본값
                target_chain,
                bridge_protocol: BridgeProtocol::Stargate, // 기본값
                token_symbol: String::new(),
                bridged_amount: U256::ZERO,
                bridged_amount_usd: 0.0,
                execution_plan: ExecutionPlan {
                    steps: Vec::new(),
                    estimated_duration: 300,
                    estimated_gas_cost: U256::from(21000u64),
                    estimated_gas_cost_usd: 5.0,
                    execution_strategy: ExecutionStrategy::Sequential,
                    batch_config: None,
                    timeout_seconds: 1800,
                    retry_config: RetryConfig {
                        max_retries: 3,
                        retry_interval_seconds: 60,
                        backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
                        retryable_errors: vec![
                            RetryableErrorType::NetworkError,
                            RetryableErrorType::RpcError,
                        ],
                    },
                },
                status: WorkflowStatus::WaitingForBridge,
                created_at: Utc::now(),
                expected_bridge_completion: Utc::now() + ChronoDuration::minutes(10),
                execution_started_at: None,
                completed_at: None,
                priority: 5,
                execution_conditions: vec![ExecutionCondition::BridgeCompleted],
                failure_count: 0,
                last_error: None,
            },
        }
    }
    
    pub fn add_step(mut self, step: ExecutionStep) -> Self {
        self.workflow.execution_plan.steps.push(step);
        self
    }
    
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.workflow.priority = priority;
        self
    }
    
    pub fn build(self) -> ExecutionWorkflow {
        self.workflow
    }
}