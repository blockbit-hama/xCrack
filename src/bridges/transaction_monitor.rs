use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, sleep, Duration, Instant};
use chrono::{DateTime, Utc};
use tracing::{info, debug, warn, error};
use serde::{Serialize, Deserialize};
use alloy::primitives::{B256, Address, U256};
use futures::StreamExt;

use crate::types::{ChainId, BridgeProtocol};
use super::performance_tracker::{BridgePerformanceTracker, ExecutionStatus};

/// 브리지 트랜잭션 상태 모니터링 시스템
/// 
/// 크로스체인 브리지 트랜잭션의 전체 라이프사이클을 추적합니다:
/// 1. 소스 체인에서의 초기 트랜잭션
/// 2. 브리지 프로토콜에서의 처리 상태
/// 3. 대상 체인에서의 최종 완료
#[derive(Debug)]
pub struct BridgeTransactionMonitor {
    /// 모니터링 중인 트랜잭션들
    active_transactions: Arc<RwLock<HashMap<String, MonitoredTransaction>>>,
    
    /// 완료된 트랜잭션 히스토리
    completed_transactions: Arc<RwLock<Vec<MonitoredTransaction>>>,
    
    /// 성능 추적기
    performance_tracker: Arc<BridgePerformanceTracker>,
    
    /// 체인별 RPC 엔드포인트
    rpc_endpoints: HashMap<ChainId, String>,
    
    /// 모니터링 설정
    config: MonitorConfig,
    
    /// 실행 중 상태
    is_running: Arc<RwLock<bool>>,
    
    /// 알림 채널
    notification_sender: Option<mpsc::UnboundedSender<TransactionEvent>>,
}

/// 모니터링되는 트랜잭션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredTransaction {
    /// 실행 ID (브리지 성능 추적기와 연동)
    pub execution_id: String,
    
    /// 브리지 프로토콜
    pub bridge_protocol: BridgeProtocol,
    
    /// 소스 체인 정보
    pub source_chain: ChainInfo,
    
    /// 대상 체인 정보
    pub dest_chain: ChainInfo,
    
    /// 토큰 정보
    pub token_symbol: String,
    pub amount: U256,
    pub amount_usd: f64,
    
    /// 트랜잭션 상태
    pub status: TransactionStatus,
    
    /// 소스 체인 트랜잭션
    pub source_tx: Option<TransactionDetails>,
    
    /// 브리지 트랜잭션들 (여러 개일 수 있음)
    pub bridge_txs: Vec<TransactionDetails>,
    
    /// 대상 체인 트랜잭션
    pub dest_tx: Option<TransactionDetails>,
    
    /// 진행 단계
    pub progress_stages: Vec<ProgressStage>,
    
    /// 모니터링 시작 시간
    pub monitoring_started: DateTime<Utc>,
    
    /// 마지막 업데이트 시간
    pub last_updated: DateTime<Utc>,
    
    /// 예상 완료 시간
    pub estimated_completion: DateTime<Utc>,
    
    /// 실제 완료 시간
    pub actual_completion: Option<DateTime<Utc>>,
    
    /// 타임아웃 시간
    pub timeout_at: DateTime<Utc>,
    
    /// 오류 정보
    pub error_info: Option<ErrorInfo>,
    
    /// 재시도 횟수
    pub retry_count: u32,
    
    /// 최대 재시도 횟수
    pub max_retries: u32,
}

/// 체인 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: ChainId,
    pub name: String,
    pub rpc_url: String,
    pub block_time: u64, // 평균 블록 시간 (초)
}

/// 트랜잭션 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    /// 초기화됨 (아직 트랜잭션 미제출)
    Initialized,
    /// 소스 체인 트랜잭션 제출됨
    SourceSubmitted,
    /// 소스 체인 트랜잭션 확인됨
    SourceConfirmed,
    /// 브리지 처리 중
    BridgeProcessing,
    /// 브리지 처리 완료
    BridgeCompleted,
    /// 대상 체인 트랜잭션 제출됨
    DestSubmitted,
    /// 대상 체인 트랜잭션 확인됨 (완료)
    DestConfirmed,
    /// 실패
    Failed,
    /// 타임아웃
    Timeout,
    /// 취소됨
    Cancelled,
}

// keep completion checks in target_execution extension only to avoid duplicates

/// 트랜잭션 상세 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    /// 트랜잭션 해시
    pub hash: B256,
    
    /// 체인 ID
    pub chain_id: ChainId,
    
    /// 보낸 주소
    pub from: Address,
    
    /// 받는 주소
    pub to: Option<Address>,
    
    /// 전송 값
    pub value: U256,
    
    /// 가스 가격
    pub gas_price: U256,
    
    /// 가스 한도
    pub gas_limit: U256,
    
    /// 실제 사용 가스
    pub gas_used: Option<U256>,
    
    /// 블록 번호
    pub block_number: Option<u64>,
    
    /// 블록 해시
    pub block_hash: Option<B256>,
    
    /// 트랜잭션 인덱스
    pub transaction_index: Option<u64>,
    
    /// 확인 횟수
    pub confirmations: u64,
    
    /// 필요 확인 횟수
    pub required_confirmations: u64,
    
    /// 제출 시간
    pub submitted_at: DateTime<Utc>,
    
    /// 확인 시간
    pub confirmed_at: Option<DateTime<Utc>>,
    
    /// 트랜잭션 상태
    pub tx_status: TxStatus,
    
    /// 실행 결과
    pub receipt: Option<TransactionReceipt>,
}

/// 트랜잭션 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TxStatus {
    Pending,      // 펜딩 중
    Confirmed,    // 확인됨
    Failed,       // 실패
    Reverted,     // 리버트됨
}

/// 트랜잭션 영수증
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub status: bool,
    pub logs: Vec<String>,
    pub gas_used: U256,
    pub effective_gas_price: U256,
}

/// 진행 단계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressStage {
    pub stage: String,
    pub description: String,
    pub timestamp: DateTime<Utc>,
    pub is_completed: bool,
    pub additional_data: HashMap<String, String>,
}

/// 오류 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub error_type: ErrorType,
    pub message: String,
    pub details: HashMap<String, String>,
    pub is_recoverable: bool,
    pub suggested_action: String,
    pub occurred_at: DateTime<Utc>,
}

/// 오류 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorType {
    NetworkError,       // 네트워크 연결 오류
    InsufficientGas,    // 가스 부족
    InsufficientFunds,  // 자금 부족
    SlippageExceeded,   // 슬리피지 초과
    BridgeError,        // 브리지 프로토콜 오류
    TimeoutError,       // 타임아웃
    ValidationError,    // 검증 오류
    UnknownError,       // 알 수 없는 오류
}

/// 트랜잭션 이벤트
#[derive(Debug, Clone)]
pub struct TransactionEvent {
    pub execution_id: String,
    pub event_type: EventType,
    pub data: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// 이벤트 타입
#[derive(Debug, Clone)]
pub enum EventType {
    StatusChanged,
    ProgressUpdate,
    ErrorOccurred,
    CompletionDetected,
    TimeoutWarning,
}

/// 모니터링 설정
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// 폴링 간격 (초)
    pub polling_interval: u64,
    
    /// 확인 횟수 요구사항
    pub confirmation_requirements: HashMap<ChainId, u64>,
    
    /// 타임아웃 설정 (초)
    pub timeout_config: TimeoutConfig,
    
    /// 재시도 설정
    pub retry_config: RetryConfig,
    
    /// 알림 설정
    pub notification_config: NotificationConfig,
}

/// 타임아웃 설정
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub source_tx_timeout: u64,     // 소스 트랜잭션 타임아웃
    pub bridge_processing_timeout: u64, // 브리지 처리 타임아웃
    pub dest_tx_timeout: u64,       // 대상 트랜잭션 타임아웃
    pub total_timeout: u64,         // 전체 타임아웃
}

/// 재시도 설정
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub retry_delay: u64,           // 재시도 지연 (초)
    pub exponential_backoff: bool,   // 지수 백오프 사용 여부
    pub max_delay: u64,             // 최대 지연 시간
}

/// 알림 설정
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    pub enable_progress_notifications: bool,
    pub enable_error_notifications: bool,
    pub enable_completion_notifications: bool,
    pub enable_timeout_warnings: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        let mut confirmation_requirements = HashMap::new();
        confirmation_requirements.insert(ChainId::Ethereum, 12);
        confirmation_requirements.insert(ChainId::Polygon, 20);
        confirmation_requirements.insert(ChainId::BSC, 15);
        confirmation_requirements.insert(ChainId::Arbitrum, 1);
        confirmation_requirements.insert(ChainId::Optimism, 1);
        confirmation_requirements.insert(ChainId::Avalanche, 6);
        
        Self {
            polling_interval: 10, // 10초마다 체크
            confirmation_requirements,
            timeout_config: TimeoutConfig {
                source_tx_timeout: 600,        // 10분
                bridge_processing_timeout: 1800, // 30분
                dest_tx_timeout: 600,          // 10분
                total_timeout: 3600,           // 1시간
            },
            retry_config: RetryConfig {
                max_retries: 3,
                retry_delay: 30,               // 30초
                exponential_backoff: true,
                max_delay: 300,                // 5분
            },
            notification_config: NotificationConfig {
                enable_progress_notifications: true,
                enable_error_notifications: true,
                enable_completion_notifications: true,
                enable_timeout_warnings: true,
            },
        }
    }
}

impl BridgeTransactionMonitor {
    /// 새로운 트랜잭션 모니터 생성
    pub fn new(performance_tracker: Arc<BridgePerformanceTracker>) -> Self {
        let mut rpc_endpoints = HashMap::new();
        
        // 기본 RPC 엔드포인트 설정 (환경변수 또는 기본값)
        rpc_endpoints.insert(
            ChainId::Ethereum,
            std::env::var("ETHEREUM_RPC_URL").unwrap_or_else(|_| "https://eth.llamarpc.com".to_string())
        );
        rpc_endpoints.insert(
            ChainId::Polygon,
            std::env::var("POLYGON_RPC_URL").unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string())
        );
        rpc_endpoints.insert(
            ChainId::BSC,
            std::env::var("BSC_RPC_URL").unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string())
        );
        rpc_endpoints.insert(
            ChainId::Arbitrum,
            std::env::var("ARBITRUM_RPC_URL").unwrap_or_else(|_| "https://arb1.arbitrum.io/rpc".to_string())
        );
        rpc_endpoints.insert(
            ChainId::Optimism,
            std::env::var("OPTIMISM_RPC_URL").unwrap_or_else(|_| "https://mainnet.optimism.io".to_string())
        );
        rpc_endpoints.insert(
            ChainId::Avalanche,
            std::env::var("AVALANCHE_RPC_URL").unwrap_or_else(|_| "https://api.avax.network/ext/bc/C/rpc".to_string())
        );
        
        Self {
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            completed_transactions: Arc::new(RwLock::new(Vec::new())),
            performance_tracker,
            rpc_endpoints,
            config: MonitorConfig::default(),
            is_running: Arc::new(RwLock::new(false)),
            notification_sender: None,
        }
    }
    
    /// 커스텀 설정으로 생성
    pub fn with_config(
        performance_tracker: Arc<BridgePerformanceTracker>,
        config: MonitorConfig,
        rpc_endpoints: HashMap<ChainId, String>,
    ) -> Self {
        Self {
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            completed_transactions: Arc::new(RwLock::new(Vec::new())),
            performance_tracker,
            rpc_endpoints,
            config,
            is_running: Arc::new(RwLock::new(false)),
            notification_sender: None,
        }
    }
    
    /// 알림 채널 설정
    pub fn with_notifications(mut self, sender: mpsc::UnboundedSender<TransactionEvent>) -> Self {
        self.notification_sender = Some(sender);
        self
    }
    
    /// 모니터링 시작
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("🔍 브리지 트랜잭션 모니터링 시작");
        
        // 모니터링 루프 시작
        self.start_monitoring_loop().await;
        
        // 타임아웃 체크 루프 시작
        self.start_timeout_check_loop().await;
        
        // 재시도 루프 시작
        self.start_retry_loop().await;
        
        info!("✅ 브리지 트랜잭션 모니터링 시작 완료");
        Ok(())
    }
    
    /// 모니터링 중지
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        
        info!("🛑 브리지 트랜잭션 모니터링 중지");
        Ok(())
    }
    
    /// 새로운 트랜잭션 모니터링 시작
    pub async fn start_monitoring_transaction(
        &self,
        execution_id: String,
        bridge_protocol: BridgeProtocol,
        source_chain: ChainId,
        dest_chain: ChainId,
        token_symbol: String,
        amount: U256,
        amount_usd: f64,
        estimated_time: u64,
    ) -> Result<()> {
        let source_chain_info = self.get_chain_info(source_chain)?;
        let dest_chain_info = self.get_chain_info(dest_chain)?;
        
        let now = Utc::now();
        let estimated_completion = now + chrono::Duration::seconds(estimated_time as i64);
        let timeout_at = now + chrono::Duration::seconds(self.config.timeout_config.total_timeout as i64);
        
        let transaction = MonitoredTransaction {
            execution_id: execution_id.clone(),
            bridge_protocol: bridge_protocol.clone(),
            source_chain: source_chain_info,
            dest_chain: dest_chain_info,
            token_symbol,
            amount,
            amount_usd,
            status: TransactionStatus::Initialized,
            source_tx: None,
            bridge_txs: Vec::new(),
            dest_tx: None,
            progress_stages: vec![
                ProgressStage {
                    stage: "initialized".to_string(),
                    description: "트랜잭션 모니터링 시작".to_string(),
                    timestamp: now,
                    is_completed: true,
                    additional_data: HashMap::new(),
                }
            ],
            monitoring_started: now,
            last_updated: now,
            estimated_completion,
            actual_completion: None,
            timeout_at,
            error_info: None,
            retry_count: 0,
            max_retries: self.config.retry_config.max_retries,
        };
        
        let mut active = self.active_transactions.write().await;
        active.insert(execution_id.clone(), transaction);
        
        let bridge_for_log = bridge_protocol.clone();
        info!("📍 트랜잭션 모니터링 시작: {} via {}", 
              execution_id, bridge_for_log.name());
        
        // 진행 상황 알림
        if self.config.notification_config.enable_progress_notifications {
            self.send_notification(TransactionEvent {
                execution_id,
                event_type: EventType::ProgressUpdate,
                data: [("stage".to_string(), "monitoring_started".to_string())].into(),
                timestamp: now,
            }).await;
        }
        
        Ok(())
    }
    
    /// 소스 체인 트랜잭션 해시 업데이트
    pub async fn update_source_transaction(
        &self,
        execution_id: String,
        tx_hash: B256,
        from: Address,
        to: Option<Address>,
        value: U256,
        gas_price: U256,
        gas_limit: U256,
    ) -> Result<()> {
        let mut active = self.active_transactions.write().await;
        
        if let Some(transaction) = active.get_mut(&execution_id) {
            let required_confirmations = self.config.confirmation_requirements
                .get(&transaction.source_chain.chain_id)
                .copied()
                .unwrap_or(12);
            
            transaction.source_tx = Some(TransactionDetails {
                hash: tx_hash,
                chain_id: transaction.source_chain.chain_id,
                from,
                to,
                value,
                gas_price,
                gas_limit,
                gas_used: None,
                block_number: None,
                block_hash: None,
                transaction_index: None,
                confirmations: 0,
                required_confirmations,
                submitted_at: Utc::now(),
                confirmed_at: None,
                tx_status: TxStatus::Pending,
                receipt: None,
            });
            
            transaction.status = TransactionStatus::SourceSubmitted;
            transaction.last_updated = Utc::now();
            
            transaction.progress_stages.push(ProgressStage {
                stage: "source_submitted".to_string(),
                description: "소스 체인 트랜잭션 제출 완료".to_string(),
                timestamp: Utc::now(),
                is_completed: true,
                additional_data: [("tx_hash".to_string(), format!("{:?}", tx_hash))].into(),
            });
            
            info!("📤 소스 트랜잭션 제출: {} - {:?}", execution_id, tx_hash);
            
            // 상태 변경 알림
            self.send_notification(TransactionEvent {
                execution_id,
                event_type: EventType::StatusChanged,
                data: [
                    ("status".to_string(), "source_submitted".to_string()),
                    ("tx_hash".to_string(), format!("{:?}", tx_hash)),
                ].into(),
                timestamp: Utc::now(),
            }).await;
        }
        
        Ok(())
    }
    
    /// 브리지 트랜잭션 해시 추가
    pub async fn add_bridge_transaction(
        &self,
        execution_id: String,
        tx_hash: B256,
        chain_id: ChainId,
    ) -> Result<()> {
        let mut active = self.active_transactions.write().await;
        
        if let Some(transaction) = active.get_mut(&execution_id) {
            let bridge_tx = TransactionDetails {
                hash: tx_hash,
                chain_id,
                from: Address::ZERO, // 브리지에서 설정됨
                to: None,
                value: U256::ZERO,
                gas_price: U256::ZERO,
                gas_limit: U256::ZERO,
                gas_used: None,
                block_number: None,
                block_hash: None,
                transaction_index: None,
                confirmations: 0,
                required_confirmations: 1, // 브리지 트랜잭션은 1 확인으로 충분
                submitted_at: Utc::now(),
                confirmed_at: None,
                tx_status: TxStatus::Pending,
                receipt: None,
            };
            
            transaction.bridge_txs.push(bridge_tx);
            
            if transaction.status == TransactionStatus::SourceConfirmed {
                transaction.status = TransactionStatus::BridgeProcessing;
            }
            
            transaction.last_updated = Utc::now();
            
            transaction.progress_stages.push(ProgressStage {
                stage: "bridge_processing".to_string(),
                description: format!("브리지 트랜잭션 처리 중 ({})", chain_id.name()),
                timestamp: Utc::now(),
                is_completed: false,
                additional_data: [("bridge_tx_hash".to_string(), format!("{:?}", tx_hash))].into(),
            });
            
            info!("🌉 브리지 트랜잭션 추가: {} - {:?} on {}", 
                  execution_id, tx_hash, chain_id.name());
        }
        
        Ok(())
    }
    
    /// 대상 체인 트랜잭션 해시 업데이트
    pub async fn update_destination_transaction(
        &self,
        execution_id: String,
        tx_hash: B256,
        from: Address,
        to: Option<Address>,
        value: U256,
    ) -> Result<()> {
        let mut active = self.active_transactions.write().await;
        
        if let Some(transaction) = active.get_mut(&execution_id) {
            let required_confirmations = self.config.confirmation_requirements
                .get(&transaction.dest_chain.chain_id)
                .copied()
                .unwrap_or(12);
            
            transaction.dest_tx = Some(TransactionDetails {
                hash: tx_hash,
                chain_id: transaction.dest_chain.chain_id,
                from,
                to,
                value,
                gas_price: U256::ZERO, // 대상 체인에서는 가스 가격이 다를 수 있음
                gas_limit: U256::ZERO,
                gas_used: None,
                block_number: None,
                block_hash: None,
                transaction_index: None,
                confirmations: 0,
                required_confirmations,
                submitted_at: Utc::now(),
                confirmed_at: None,
                tx_status: TxStatus::Pending,
                receipt: None,
            });
            
            transaction.status = TransactionStatus::DestSubmitted;
            transaction.last_updated = Utc::now();
            
            transaction.progress_stages.push(ProgressStage {
                stage: "dest_submitted".to_string(),
                description: "대상 체인 트랜잭션 제출 완료".to_string(),
                timestamp: Utc::now(),
                is_completed: true,
                additional_data: [("dest_tx_hash".to_string(), format!("{:?}", tx_hash))].into(),
            });
            
            info!("📥 대상 트랜잭션 제출: {} - {:?}", execution_id, tx_hash);
        }
        
        Ok(())
    }
    
    /// 메인 모니터링 루프
    async fn start_monitoring_loop(&self) {
        let active_transactions = Arc::clone(&self.active_transactions);
        let is_running = Arc::clone(&self.is_running);
        let polling_interval = self.config.polling_interval;
        let rpc_endpoints = self.rpc_endpoints.clone();
        let confirmation_requirements = self.config.confirmation_requirements.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(polling_interval));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let mut active = active_transactions.write().await;
                let mut completed_ids = Vec::new();
                
                for (execution_id, transaction) in active.iter_mut() {
                    // Mock 모드에서는 실제 RPC 호출 대신 시뮬레이션
                    if std::env::var("API_MODE").unwrap_or_default() == "mock" {
                        Self::simulate_transaction_progress(transaction).await;
                    } else {
                        // 실제 RPC 호출로 트랜잭션 상태 확인
                        if let Err(e) = Self::check_transaction_status(
                            transaction,
                            &rpc_endpoints,
                            &confirmation_requirements
                        ).await {
                            error!("트랜잭션 상태 확인 실패 {}: {}", execution_id, e);
                        }
                    }
                    
                    // 완료된 트랜잭션 체크
                    if matches!(transaction.status, 
                        TransactionStatus::DestConfirmed | 
                        TransactionStatus::Failed | 
                        TransactionStatus::Timeout |
                        TransactionStatus::Cancelled
                    ) {
                        completed_ids.push(execution_id.clone());
                    }
                }
                
                // 완료된 트랜잭션들을 히스토리로 이동
                for execution_id in completed_ids {
                    if let Some(completed_tx) = active.remove(&execution_id) {
                        // 성능 추적기에 완료 상태 기록
                        // 이는 이미 완료 처리된 경우 중복 호출될 수 있으므로 체크 필요
                        // (별도 로직에서 처리되므로 여기서는 생략)
                        
                        debug!("트랜잭션 모니터링 완료: {}", execution_id);
                    }
                }
            }
        });
    }
    
    /// 타임아웃 체크 루프
    async fn start_timeout_check_loop(&self) {
        let active_transactions = Arc::clone(&self.active_transactions);
        let is_running = Arc::clone(&self.is_running);
        let performance_tracker = Arc::clone(&self.performance_tracker);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // 30초마다 체크
            
            while *is_running.read().await {
                interval.tick().await;
                
                let mut active = active_transactions.write().await;
                let now = Utc::now();
                
                for (execution_id, transaction) in active.iter_mut() {
                    if now > transaction.timeout_at && 
                       !matches!(transaction.status, 
                           TransactionStatus::DestConfirmed | 
                           TransactionStatus::Failed | 
                           TransactionStatus::Timeout
                       ) {
                        
                        warn!("⏰ 트랜잭션 타임아웃: {}", execution_id);
                        
                        transaction.status = TransactionStatus::Timeout;
                        transaction.last_updated = now;
                        transaction.error_info = Some(ErrorInfo {
                            error_type: ErrorType::TimeoutError,
                            message: "트랜잭션 처리 시간 초과".to_string(),
                            details: HashMap::new(),
                            is_recoverable: false,
                            suggested_action: "브리지 상태 확인 후 수동 처리 필요".to_string(),
                            occurred_at: now,
                        });
                        
                        // 성능 추적기에 타임아웃 기록
                        let _ = performance_tracker.record_execution_completion(
                            execution_id.clone(),
                            ExecutionStatus::Timeout,
                            None,
                            None,
                            Some("Transaction timeout".to_string()),
                            Vec::new(),
                        ).await;
                    }
                }
            }
        });
    }
    
    /// 재시도 루프
    async fn start_retry_loop(&self) {
        let active_transactions = Arc::clone(&self.active_transactions);
        let is_running = Arc::clone(&self.is_running);
        let retry_config = self.config.retry_config.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(retry_config.retry_delay));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let active = active_transactions.read().await;
                
                for (execution_id, transaction) in active.iter() {
                    if let Some(error_info) = &transaction.error_info {
                        if error_info.is_recoverable && 
                           transaction.retry_count < transaction.max_retries {
                            
                            info!("🔄 트랜잭션 재시도 준비: {} (시도 {}/{})", 
                                  execution_id, 
                                  transaction.retry_count + 1, 
                                  transaction.max_retries);
                            
                            // 실제 재시도 로직은 별도 함수에서 처리
                            // 여기서는 재시도 가능한 트랜잭션을 식별만 함
                        }
                    }
                }
            }
        });
    }
    
    /// Mock 트랜잭션 진행 시뮬레이션
    async fn simulate_transaction_progress(transaction: &mut MonitoredTransaction) {
        let now = Utc::now();
        let elapsed = (now - transaction.monitoring_started).num_seconds();
        
        match transaction.status {
            TransactionStatus::Initialized => {
                // 5초 후 소스 제출됨으로 변경
                if elapsed > 5 {
                    transaction.status = TransactionStatus::SourceSubmitted;
                    transaction.last_updated = now;
                }
            }
            TransactionStatus::SourceSubmitted => {
                // 30초 후 소스 확인됨으로 변경
                if elapsed > 30 {
                    transaction.status = TransactionStatus::SourceConfirmed;
                    transaction.last_updated = now;
                    
                    if let Some(ref mut source_tx) = transaction.source_tx {
                        source_tx.confirmations = source_tx.required_confirmations;
                        source_tx.confirmed_at = Some(now);
                        source_tx.tx_status = TxStatus::Confirmed;
                    }
                }
            }
            TransactionStatus::SourceConfirmed => {
                // 60초 후 브리지 처리 중으로 변경
                if elapsed > 60 {
                    transaction.status = TransactionStatus::BridgeProcessing;
                    transaction.last_updated = now;
                }
            }
            TransactionStatus::BridgeProcessing => {
                // 180초 후 브리지 완료로 변경
                if elapsed > 180 {
                    transaction.status = TransactionStatus::BridgeCompleted;
                    transaction.last_updated = now;
                }
            }
            TransactionStatus::BridgeCompleted => {
                // 210초 후 대상 제출됨으로 변경
                if elapsed > 210 {
                    transaction.status = TransactionStatus::DestSubmitted;
                    transaction.last_updated = now;
                }
            }
            TransactionStatus::DestSubmitted => {
                // 240초 후 완료로 변경
                if elapsed > 240 {
                    transaction.status = TransactionStatus::DestConfirmed;
                    transaction.actual_completion = Some(now);
                    transaction.last_updated = now;
                    
                    if let Some(ref mut dest_tx) = transaction.dest_tx {
                        dest_tx.confirmations = dest_tx.required_confirmations;
                        dest_tx.confirmed_at = Some(now);
                        dest_tx.tx_status = TxStatus::Confirmed;
                    }
                }
            }
            _ => {}
        }
    }
    
    /// 실제 트랜잭션 상태 확인 (RPC 호출)
    async fn check_transaction_status(
        transaction: &mut MonitoredTransaction,
        rpc_endpoints: &HashMap<ChainId, String>,
        confirmation_requirements: &HashMap<ChainId, u64>,
    ) -> Result<()> {
        // 실제 구현에서는 각 체인의 RPC를 호출하여 트랜잭션 상태 확인
        // 여기서는 기본 구조만 제공
        
        // 소스 체인 트랜잭션 확인
        if let Some(ref mut source_tx) = transaction.source_tx {
            if source_tx.tx_status == TxStatus::Pending {
                // RPC 호출하여 트랜잭션 상태 확인
                // let receipt = get_transaction_receipt(rpc_endpoints, source_tx.chain_id, source_tx.hash).await?;
                // source_tx 상태 업데이트
            }
        }
        
        // 브리지 트랜잭션들 확인
        for bridge_tx in &mut transaction.bridge_txs {
            if bridge_tx.tx_status == TxStatus::Pending {
                // 브리지 트랜잭션 상태 확인
            }
        }
        
        // 대상 체인 트랜잭션 확인
        if let Some(ref mut dest_tx) = transaction.dest_tx {
            if dest_tx.tx_status == TxStatus::Pending {
                // 대상 체인 트랜잭션 상태 확인
            }
        }
        
        Ok(())
    }
    
    /// 체인 정보 조회
    fn get_chain_info(&self, chain_id: ChainId) -> Result<ChainInfo> {
        let rpc_url = self.rpc_endpoints.get(&chain_id)
            .ok_or_else(|| anyhow!("RPC endpoint not found for chain: {:?}", chain_id))?
            .clone();
        
        let (name, block_time) = match chain_id {
            ChainId::Ethereum => ("Ethereum".to_string(), 12),
            ChainId::Polygon => ("Polygon".to_string(), 2),
            ChainId::BSC => ("BSC".to_string(), 3),
            ChainId::Arbitrum => ("Arbitrum".to_string(), 1),
            ChainId::Optimism => ("Optimism".to_string(), 2),
            ChainId::Avalanche => ("Avalanche".to_string(), 2),
        };
        
        Ok(ChainInfo {
            chain_id,
            name,
            rpc_url,
            block_time,
        })
    }
    
    /// 알림 전송
    async fn send_notification(&self, event: TransactionEvent) {
        if let Some(ref sender) = self.notification_sender {
            if let Err(e) = sender.send(event) {
                error!("알림 전송 실패: {}", e);
            }
        }
    }
    
    /// 진행 중인 트랜잭션 목록 조회
    pub async fn get_active_transactions(&self) -> Vec<MonitoredTransaction> {
        let active = self.active_transactions.read().await;
        active.values().cloned().collect()
    }
    
    /// 특정 트랜잭션 상태 조회
    pub async fn get_transaction_status(&self, execution_id: &str) -> Option<MonitoredTransaction> {
        let active = self.active_transactions.read().await;
        active.get(execution_id).cloned()
    }
    
    /// 완료된 트랜잭션 히스토리 조회
    pub async fn get_completed_transactions(&self, limit: usize) -> Vec<MonitoredTransaction> {
        let completed = self.completed_transactions.read().await;
        let start = if completed.len() > limit {
            completed.len() - limit
        } else {
            0
        };
        completed[start..].to_vec()
    }
    
    /// 트랜잭션 강제 취소
    pub async fn cancel_transaction(&self, execution_id: String, reason: String) -> Result<()> {
        let mut active = self.active_transactions.write().await;
        
        if let Some(transaction) = active.get_mut(&execution_id) {
            transaction.status = TransactionStatus::Cancelled;
            transaction.last_updated = Utc::now();
            transaction.error_info = Some(ErrorInfo {
                error_type: ErrorType::ValidationError,
                message: format!("Transaction cancelled: {}", reason),
                details: HashMap::new(),
                is_recoverable: false,
                suggested_action: "Manual review required".to_string(),
                occurred_at: Utc::now(),
            });
            
            info!("❌ 트랜잭션 취소: {} - {}", execution_id, reason);
            
            // 성능 추적기에 취소 기록
            self.performance_tracker.record_execution_completion(
                execution_id.clone(),
                ExecutionStatus::Cancelled,
                None,
                None,
                Some(reason),
                Vec::new(),
            ).await?;
        }
        
        Ok(())
    }
}