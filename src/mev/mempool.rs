use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http, Ws, Middleware, StreamExt},
    types::{Transaction, H256, U256, Address, TxHash},
    utils::hex,
};
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex, RwLock};
use std::collections::{HashMap, HashSet, VecDeque};
use reqwest::Client as HttpClient;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};

use crate::blockchain::BlockchainClient;

/// 프라이빗 멤풀 클라이언트
/// 
/// 고급 MEV 기회를 위한 프라이빗 멤풀 접근 및 모니터링
pub struct PrivateMempoolClient {
    blockchain_client: Arc<BlockchainClient>,
    connections: Arc<RwLock<HashMap<String, PoolConnection>>>,
    transaction_queue: Arc<Mutex<VecDeque<PendingTransaction>>>,
    filters: Arc<RwLock<Vec<TransactionFilter>>>,
    subscribers: Arc<Mutex<Vec<mpsc::UnboundedSender<MempoolEvent>>>>,
    stats: Arc<Mutex<MempoolStats>>,
    config: MempoolConfig,
}

/// 풀 연결
#[derive(Debug)]
struct PoolConnection {
    pool_name: String,
    connection_type: ConnectionType,
    status: ConnectionStatus,
    last_ping: SystemTime,
    transaction_count: u64,
    error_count: u64,
    latency_ms: u64,
}

/// 연결 타입
#[derive(Debug, Clone)]
enum ConnectionType {
    Websocket(String),      // WebSocket URL
    Http(String),           // HTTP API URL
    P2P(String),           // P2P 노드 주소
    PrivateRelay(String),  // 프라이빗 릴레이
}

/// 연결 상태
#[derive(Debug, Clone, PartialEq)]
enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error(String),
}

/// 대기 중인 트랜잭션
#[derive(Debug, Clone)]
pub struct PendingTransaction {
    pub hash: H256,
    pub transaction: Transaction,
    pub received_at: SystemTime,
    pub pool_source: String,
    pub confidence_score: f64,
    pub priority_fee: U256,
    pub effective_gas_price: U256,
    pub is_private: bool,
    pub bundle_hint: Option<String>,
}

/// 트랜잭션 필터
#[derive(Debug, Clone)]
pub struct TransactionFilter {
    pub name: String,
    pub filter_type: FilterType,
    pub conditions: Vec<FilterCondition>,
    pub priority: u8,
    pub enabled: bool,
}

/// 필터 타입
#[derive(Debug, Clone)]
pub enum FilterType {
    MEVOpportunity,     // MEV 기회
    HighValue,          // 고가치 트랜잭션
    ContractCall,       // 컨트랙트 호출
    TokenTransfer,      // 토큰 전송
    DeFiProtocol,       // DeFi 프로토콜
    NFTTrade,           // NFT 거래
    Custom(String),     // 커스텀 필터
}

/// 필터 조건
#[derive(Debug, Clone)]
pub struct FilterCondition {
    pub field: FilterField,
    pub operator: FilterOperator,
    pub value: FilterValue,
}

/// 필터 필드
#[derive(Debug, Clone)]
pub enum FilterField {
    To,
    From,
    Value,
    GasPrice,
    GasLimit,
    Data,
    Nonce,
    MethodId,
}

/// 필터 연산자
#[derive(Debug, Clone)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    StartsWith,
    EndsWith,
    In(Vec<String>),
}

/// 필터 값
#[derive(Debug, Clone)]
pub enum FilterValue {
    String(String),
    Number(u64),
    Address(Address),
    Hash(H256),
    Boolean(bool),
}

/// 멤풀 이벤트
#[derive(Debug, Clone)]
pub enum MempoolEvent {
    NewTransaction(PendingTransaction),
    TransactionMined(H256),
    TransactionDropped(H256),
    PoolConnected(String),
    PoolDisconnected(String),
    FilterMatch {
        filter_name: String,
        transaction: PendingTransaction,
    },
}

/// 멤풀 통계
#[derive(Debug, Default)]
struct MempoolStats {
    total_transactions: u64,
    filtered_transactions: u64,
    mev_opportunities: u64,
    high_value_transactions: u64,
    avg_latency_ms: f64,
    pool_statistics: HashMap<String, PoolStats>,
}

/// 풀 통계
#[derive(Debug, Default)]
struct PoolStats {
    transactions_received: u64,
    unique_transactions: u64,
    duplicate_transactions: u64,
    avg_first_seen_advantage_ms: f64,
    connection_uptime: f64,
}

/// 멤풀 설정
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    pub max_queue_size: usize,
    pub connection_timeout: Duration,
    pub ping_interval: Duration,
    pub retry_attempts: u32,
    pub enable_deduplication: bool,
    pub enable_statistics: bool,
    pub latency_tracking: bool,
    pub priority_pools: Vec<String>,
}

impl PrivateMempoolClient {
    /// 새로운 프라이빗 멤풀 클라이언트 생성
    pub fn new(
        blockchain_client: Arc<BlockchainClient>,
        config: MempoolConfig,
    ) -> Self {
        Self {
            blockchain_client,
            connections: Arc::new(RwLock::new(HashMap::new())),
            transaction_queue: Arc::new(Mutex::new(VecDeque::new())),
            filters: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(MempoolStats::default())),
            config,
        }
    }

    /// 프라이빗 풀에 연결
    pub async fn connect_to_pool(
        &self,
        pool_name: String,
        connection_type: ConnectionType,
    ) -> Result<()> {
        info!("🔗 프라이빗 풀에 연결 중: {}", pool_name);

        let mut connections = self.connections.write().await;
        
        let connection = PoolConnection {
            pool_name: pool_name.clone(),
            connection_type: connection_type.clone(),
            status: ConnectionStatus::Connecting,
            last_ping: SystemTime::now(),
            transaction_count: 0,
            error_count: 0,
            latency_ms: 0,
        };

        connections.insert(pool_name.clone(), connection);
        drop(connections);

        // 연결 타입에 따른 실제 연결 수행
        match connection_type {
            ConnectionType::Websocket(url) => {
                self.connect_websocket(pool_name.clone(), url).await?;
            }
            ConnectionType::Http(url) => {
                self.connect_http_polling(pool_name.clone(), url).await?;
            }
            ConnectionType::P2P(address) => {
                self.connect_p2p(pool_name.clone(), address).await?;
            }
            ConnectionType::PrivateRelay(url) => {
                self.connect_private_relay(pool_name.clone(), url).await?;
            }
        }

        // 연결 상태 업데이트
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(&pool_name) {
            conn.status = ConnectionStatus::Connected;
        }

        info!("✅ 프라이빗 풀 연결 성공: {}", pool_name);

        // 이벤트 발송
        self.send_event(MempoolEvent::PoolConnected(pool_name)).await;

        Ok(())
    }

    /// WebSocket 연결
    async fn connect_websocket(&self, pool_name: String, url: String) -> Result<()> {
        debug!("🌐 WebSocket 연결: {} -> {}", pool_name, url);

        let (ws_stream, _) = connect_async(&url).await
            .map_err(|e| anyhow!("WebSocket 연결 실패: {}", e))?;

        let (mut write, mut read) = ws_stream.split();

        // 구독 메시지 전송
        let subscribe_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["newPendingTransactions", true]
        });

        write.send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe_msg.to_string()
        )).await?;

        // 메시지 수신 루프
        let pool_name_clone = pool_name.clone();
        let self_clone = Arc::new(self.clone());
        
        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                        if let Err(e) = self_clone.handle_websocket_message(&pool_name_clone, text).await {
                            warn!("WebSocket 메시지 처리 실패: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("WebSocket 연결 오류: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// HTTP 폴링 연결
    async fn connect_http_polling(&self, pool_name: String, url: String) -> Result<()> {
        debug!("📡 HTTP 폴링 시작: {} -> {}", pool_name, url);

        let client = HttpClient::new();
        let pool_name_clone = pool_name.clone();
        let self_clone = Arc::new(self.clone());
        let url_clone = url.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500)); // 500ms 간격

            loop {
                interval.tick().await;

                match self_clone.poll_pending_transactions(&client, &url_clone).await {
                    Ok(transactions) => {
                        for tx in transactions {
                            let pending_tx = PendingTransaction {
                                hash: tx.hash,
                                transaction: tx,
                                received_at: SystemTime::now(),
                                pool_source: pool_name_clone.clone(),
                                confidence_score: 0.8, // HTTP 폴링은 낮은 신뢰도
                                priority_fee: U256::zero(),
                                effective_gas_price: U256::zero(),
                                is_private: true,
                                bundle_hint: None,
                            };

                            if let Err(e) = self_clone.process_transaction(pending_tx).await {
                                warn!("트랜잭션 처리 실패: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("HTTP 폴링 실패: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// P2P 연결
    async fn connect_p2p(&self, pool_name: String, address: String) -> Result<()> {
        debug!("🔗 P2P 연결: {} -> {}", pool_name, address);
        
        // P2P 연결 구현은 복잡하므로 스텁으로 대체
        warn!("P2P 연결은 아직 구현되지 않았습니다: {}", address);
        
        Ok(())
    }

    /// 프라이빗 릴레이 연결
    async fn connect_private_relay(&self, pool_name: String, url: String) -> Result<()> {
        debug!("🏪 프라이빗 릴레이 연결: {} -> {}", pool_name, url);
        
        // 프라이빗 릴레이 연결 구현
        // 대부분의 프라이빗 릴레이는 WebSocket이나 HTTP API를 사용
        self.connect_websocket(pool_name, url).await
    }

    /// WebSocket 메시지 처리
    async fn handle_websocket_message(&self, pool_name: &str, message: String) -> Result<()> {
        let json: serde_json::Value = serde_json::from_str(&message)?;

        if let Some(params) = json.get("params") {
            if let Some(result) = params.get("result") {
                // 트랜잭션 파싱
                if let Ok(tx_hex) = serde_json::from_value::<String>(result.clone()) {
                    if let Ok(tx_bytes) = hex::decode(tx_hex.trim_start_matches("0x")) {
                        if let Ok(tx) = alloy::rlp::Decodable::decode(&tx_bytes) {
                            let pending_tx = PendingTransaction {
                                hash: tx.hash,
                                transaction: tx,
                                received_at: SystemTime::now(),
                                pool_source: pool_name.to_string(),
                                confidence_score: 0.95, // WebSocket은 높은 신뢰도
                                priority_fee: U256::zero(),
                                effective_gas_price: U256::zero(),
                                is_private: true,
                                bundle_hint: None,
                            };

                            self.process_transaction(pending_tx).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 대기 중인 트랜잭션 폴링
    async fn poll_pending_transactions(&self, client: &HttpClient, url: &str) -> Result<Vec<Transaction>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": ["pending", true]
        });

        let response = client
            .post(url)
            .json(&request)
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        
        if let Some(result) = json.get("result") {
            if let Some(transactions) = result.get("transactions") {
                let mut parsed_transactions = Vec::new();
                
                if let Ok(tx_array) = serde_json::from_value::<Vec<serde_json::Value>>(transactions.clone()) {
                    for tx_json in tx_array {
                        if let Ok(tx) = serde_json::from_value::<Transaction>(tx_json) {
                            parsed_transactions.push(tx);
                        }
                    }
                }
                
                return Ok(parsed_transactions);
            }
        }

        Ok(Vec::new())
    }

    /// 트랜잭션 처리
    async fn process_transaction(&self, pending_tx: PendingTransaction) -> Result<()> {
        debug!("📥 트랜잭션 처리: {:?}", pending_tx.hash);

        // 중복 제거
        if self.config.enable_deduplication {
            if self.is_duplicate_transaction(&pending_tx).await {
                debug!("🔄 중복 트랜잭션 무시: {:?}", pending_tx.hash);
                return Ok(());
            }
        }

        // 필터 적용
        let matching_filters = self.apply_filters(&pending_tx).await?;
        
        // 매칭된 필터가 있으면 이벤트 발송
        for filter_name in matching_filters {
            self.send_event(MempoolEvent::FilterMatch {
                filter_name,
                transaction: pending_tx.clone(),
            }).await;
        }

        // 큐에 추가
        {
            let mut queue = self.transaction_queue.lock().await;
            
            // 큐 크기 제한
            while queue.len() >= self.config.max_queue_size {
                queue.pop_front();
            }
            
            queue.push_back(pending_tx.clone());
        }

        // 새 트랜잭션 이벤트 발송
        self.send_event(MempoolEvent::NewTransaction(pending_tx)).await;

        // 통계 업데이트
        if self.config.enable_statistics {
            self.update_statistics().await;
        }

        Ok(())
    }

    /// 중복 트랜잭션 확인
    async fn is_duplicate_transaction(&self, pending_tx: &PendingTransaction) -> bool {
        let queue = self.transaction_queue.lock().await;
        queue.iter().any(|tx| tx.hash == pending_tx.hash)
    }

    /// 필터 적용
    async fn apply_filters(&self, pending_tx: &PendingTransaction) -> Result<Vec<String>> {
        let filters = self.filters.read().await;
        let mut matching_filters = Vec::new();

        for filter in filters.iter() {
            if !filter.enabled {
                continue;
            }

            if self.transaction_matches_filter(pending_tx, filter) {
                matching_filters.push(filter.name.clone());
            }
        }

        Ok(matching_filters)
    }

    /// 트랜잭션이 필터와 매칭되는지 확인
    fn transaction_matches_filter(&self, pending_tx: &PendingTransaction, filter: &TransactionFilter) -> bool {
        // 모든 조건이 만족되어야 매칭
        filter.conditions.iter().all(|condition| {
            self.condition_matches(pending_tx, condition)
        })
    }

    /// 개별 조건 매칭 확인
    fn condition_matches(&self, pending_tx: &PendingTransaction, condition: &FilterCondition) -> bool {
        let field_value = self.get_field_value(pending_tx, &condition.field);
        self.compare_values(&field_value, &condition.operator, &condition.value)
    }

    /// 필드 값 추출
    fn get_field_value(&self, pending_tx: &PendingTransaction, field: &FilterField) -> FilterValue {
        match field {
            FilterField::To => {
                match pending_tx.transaction.to {
                    Some(addr) => FilterValue::Address(addr),
                    None => FilterValue::Address(Address::zero()),
                }
            }
            FilterField::From => FilterValue::Address(pending_tx.transaction.from),
            FilterField::Value => FilterValue::Number(pending_tx.transaction.value.as_u64()),
            FilterField::GasPrice => FilterValue::Number(
                pending_tx.transaction.gas_price.unwrap_or_default().as_u64()
            ),
            FilterField::GasLimit => FilterValue::Number(pending_tx.transaction.gas.as_u64()),
            FilterField::Data => FilterValue::String(hex::encode(&pending_tx.transaction.input)),
            FilterField::Nonce => FilterValue::Number(pending_tx.transaction.nonce.as_u64()),
            FilterField::MethodId => {
                if pending_tx.transaction.input.len() >= 4 {
                    let method_id = hex::encode(&pending_tx.transaction.input[0..4]);
                    FilterValue::String(method_id)
                } else {
                    FilterValue::String(String::new())
                }
            }
        }
    }

    /// 값 비교
    fn compare_values(&self, field_value: &FilterValue, operator: &FilterOperator, target_value: &FilterValue) -> bool {
        match (field_value, operator, target_value) {
            (FilterValue::Number(a), FilterOperator::GreaterThan, FilterValue::Number(b)) => a > b,
            (FilterValue::Number(a), FilterOperator::LessThan, FilterValue::Number(b)) => a < b,
            (FilterValue::Number(a), FilterOperator::Equals, FilterValue::Number(b)) => a == b,
            (FilterValue::String(a), FilterOperator::Contains, FilterValue::String(b)) => a.contains(b),
            (FilterValue::String(a), FilterOperator::StartsWith, FilterValue::String(b)) => a.starts_with(b),
            (FilterValue::Address(a), FilterOperator::Equals, FilterValue::Address(b)) => a == b,
            _ => false, // 다른 조합들은 기본적으로 false
        }
    }

    /// 이벤트 발송
    async fn send_event(&self, event: MempoolEvent) {
        let subscribers = self.subscribers.lock().await;
        let mut failed_subscribers = Vec::new();

        for (i, sender) in subscribers.iter().enumerate() {
            if sender.send(event.clone()).is_err() {
                failed_subscribers.push(i);
            }
        }

        // 실패한 구독자 제거는 별도 함수에서 처리
        drop(subscribers);
        
        if !failed_subscribers.is_empty() {
            self.cleanup_failed_subscribers(failed_subscribers).await;
        }
    }

    /// 실패한 구독자 정리
    async fn cleanup_failed_subscribers(&self, failed_indices: Vec<usize>) {
        let mut subscribers = self.subscribers.lock().await;
        
        // 역순으로 제거해야 인덱스가 안 꼬임
        for &index in failed_indices.iter().rev() {
            if index < subscribers.len() {
                subscribers.remove(index);
            }
        }
    }

    /// 통계 업데이트
    async fn update_statistics(&self) {
        let mut stats = self.stats.lock().await;
        stats.total_transactions += 1;
    }

    /// 필터 추가
    pub async fn add_filter(&self, filter: TransactionFilter) {
        let mut filters = self.filters.write().await;
        filters.push(filter);
    }

    /// 구독자 추가
    pub async fn subscribe(&self) -> mpsc::UnboundedReceiver<MempoolEvent> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut subscribers = self.subscribers.lock().await;
        subscribers.push(sender);
        receiver
    }

    /// 대기 중인 트랜잭션 가져오기
    pub async fn get_pending_transactions(&self, limit: Option<usize>) -> Vec<PendingTransaction> {
        let queue = self.transaction_queue.lock().await;
        let count = limit.unwrap_or(queue.len());
        queue.iter().rev().take(count).cloned().collect()
    }

    /// 연결 상태 확인
    pub async fn get_connection_status(&self) -> HashMap<String, ConnectionStatus> {
        let connections = self.connections.read().await;
        connections.iter()
            .map(|(name, conn)| (name.clone(), conn.status.clone()))
            .collect()
    }

    /// 통계 가져오기
    pub async fn get_statistics(&self) -> MempoolStats {
        let stats = self.stats.lock().await;
        stats.clone()
    }
}

impl Clone for PrivateMempoolClient {
    fn clone(&self) -> Self {
        Self {
            blockchain_client: Arc::clone(&self.blockchain_client),
            connections: Arc::clone(&self.connections),
            transaction_queue: Arc::clone(&self.transaction_queue),
            filters: Arc::clone(&self.filters),
            subscribers: Arc::clone(&self.subscribers),
            stats: Arc::clone(&self.stats),
            config: self.config.clone(),
        }
    }
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            connection_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(30),
            retry_attempts: 3,
            enable_deduplication: true,
            enable_statistics: true,
            latency_tracking: true,
            priority_pools: vec![
                "flashbots".to_string(),
                "eden".to_string(),
                "bloxroute".to_string(),
            ],
        }
    }
}

impl Clone for MempoolStats {
    fn clone(&self) -> Self {
        Self {
            total_transactions: self.total_transactions,
            filtered_transactions: self.filtered_transactions,
            mev_opportunities: self.mev_opportunities,
            high_value_transactions: self.high_value_transactions,
            avg_latency_ms: self.avg_latency_ms,
            pool_statistics: self.pool_statistics.clone(),
        }
    }
}

/// 고급 트랜잭션 분석기
pub struct TransactionAnalyzer {
    dex_contracts: HashSet<Address>,
    lending_contracts: HashSet<Address>,
    nft_contracts: HashSet<Address>,
    method_signatures: HashMap<String, String>,
}

impl TransactionAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            dex_contracts: HashSet::new(),
            lending_contracts: HashSet::new(),
            nft_contracts: HashSet::new(),
            method_signatures: HashMap::new(),
        };

        analyzer.initialize_known_contracts();
        analyzer.initialize_method_signatures();
        analyzer
    }

    fn initialize_known_contracts(&mut self) {
        // Uniswap V2/V3 라우터
        self.dex_contracts.insert("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap());
        self.dex_contracts.insert("0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap());
        
        // Aave V2/V3
        self.lending_contracts.insert("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap());
        
        // OpenSea
        self.nft_contracts.insert("0x00000000006c3852cbEf3e08E8dF289169EdE581".parse().unwrap());
    }

    fn initialize_method_signatures(&mut self) {
        // DEX 메소드들
        self.method_signatures.insert("0xa9059cbb".to_string(), "transfer(address,uint256)".to_string());
        self.method_signatures.insert("0x095ea7b3".to_string(), "approve(address,uint256)".to_string());
        self.method_signatures.insert("0x38ed1739".to_string(), "swapExactTokensForTokens".to_string());
        self.method_signatures.insert("0x7ff36ab5".to_string(), "swapExactETHForTokens".to_string());
        
        // 대출 프로토콜 메소드들
        self.method_signatures.insert("0xe8eda9df".to_string(), "deposit(address,uint256,address,uint16)".to_string());
        self.method_signatures.insert("0x69328dec".to_string(), "withdraw(address,uint256,address)".to_string());
    }

    pub fn analyze_transaction(&self, tx: &Transaction) -> TransactionAnalysis {
        let mut analysis = TransactionAnalysis::default();

        // 컨트랙트 타입 분석
        if let Some(to_address) = tx.to {
            if self.dex_contracts.contains(&to_address) {
                analysis.contract_type = Some(ContractType::DEX);
                analysis.is_mev_relevant = true;
            } else if self.lending_contracts.contains(&to_address) {
                analysis.contract_type = Some(ContractType::Lending);
                analysis.is_mev_relevant = true;
            } else if self.nft_contracts.contains(&to_address) {
                analysis.contract_type = Some(ContractType::NFT);
            }
        }

        // 메소드 분석
        if tx.input.len() >= 4 {
            let method_id = hex::encode(&tx.input[0..4]);
            if let Some(method_name) = self.method_signatures.get(&format!("0x{}", method_id)) {
                analysis.method_name = Some(method_name.clone());
                
                // MEV 관련 메소드 확인
                if method_name.contains("swap") || method_name.contains("liquidate") {
                    analysis.is_mev_relevant = true;
                    analysis.mev_type = Some(self.classify_mev_type(method_name));
                }
            }
        }

        // 가치 분석
        analysis.value_category = self.classify_value(tx.value);
        
        // 가스 분석
        analysis.gas_category = self.classify_gas(tx.gas_price.unwrap_or_default());

        analysis
    }

    fn classify_mev_type(&self, method_name: &str) -> MEVType {
        if method_name.contains("swap") {
            MEVType::Arbitrage
        } else if method_name.contains("liquidate") {
            MEVType::Liquidation
        } else {
            MEVType::Other
        }
    }

    fn classify_value(&self, value: U256) -> ValueCategory {
        let eth_value = value.as_u128() as f64 / 1e18;
        
        if eth_value >= 100.0 {
            ValueCategory::VeryHigh
        } else if eth_value >= 10.0 {
            ValueCategory::High
        } else if eth_value >= 1.0 {
            ValueCategory::Medium
        } else if eth_value > 0.0 {
            ValueCategory::Low
        } else {
            ValueCategory::Zero
        }
    }

    fn classify_gas(&self, gas_price: U256) -> GasCategory {
        let gwei = gas_price.as_u64() / 1_000_000_000;
        
        if gwei >= 100 {
            GasCategory::VeryHigh
        } else if gwei >= 50 {
            GasCategory::High
        } else if gwei >= 20 {
            GasCategory::Medium
        } else {
            GasCategory::Low
        }
    }
}

/// 트랜잭션 분석 결과
#[derive(Debug, Default)]
pub struct TransactionAnalysis {
    pub contract_type: Option<ContractType>,
    pub method_name: Option<String>,
    pub is_mev_relevant: bool,
    pub mev_type: Option<MEVType>,
    pub value_category: ValueCategory,
    pub gas_category: GasCategory,
    pub complexity_score: f64,
}

#[derive(Debug)]
pub enum ContractType {
    DEX,
    Lending,
    NFT,
    Token,
    Bridge,
    Other,
}

#[derive(Debug)]
pub enum MEVType {
    Arbitrage,
    Liquidation,
    Sandwich,
    Frontrun,
    Backrun,
    Other,
}

#[derive(Debug, Default)]
pub enum ValueCategory {
    #[default]
    Zero,
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Default)]
pub enum GasCategory {
    #[default]
    Low,
    Medium,
    High,
    VeryHigh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mempool_client_creation() {
        let config = MempoolConfig::default();
        let blockchain_client = Arc::new(BlockchainClient::new("", None).await.unwrap());
        let client = PrivateMempoolClient::new(blockchain_client, config);
        
        let status = client.get_connection_status().await;
        assert!(status.is_empty());
    }

    #[test]
    fn test_transaction_analyzer() {
        let analyzer = TransactionAnalyzer::new();
        
        // Uniswap 라우터 주소 테스트
        assert!(analyzer.dex_contracts.contains(
            &"0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()
        ));
        
        // 메소드 시그니처 테스트
        assert!(analyzer.method_signatures.contains_key("0xa9059cbb"));
    }

    #[test]
    fn test_filter_conditions() {
        let condition = FilterCondition {
            field: FilterField::Value,
            operator: FilterOperator::GreaterThan,
            value: FilterValue::Number(1000000),
        };
        
        // 조건 구조 테스트
        assert!(matches!(condition.field, FilterField::Value));
        assert!(matches!(condition.operator, FilterOperator::GreaterThan));
    }
}