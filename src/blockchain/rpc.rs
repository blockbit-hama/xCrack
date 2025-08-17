use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http, Ws, Middleware, StreamExt},
    types::{Block, Transaction, H256, U256, Address, BlockNumber, Filter, Log},
    abi::Abi,
    contract::Contract,
};
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;

/// 블록체인 RPC 클라이언트
/// 실제 온체인 데이터를 읽고 트랜잭션을 전송하는 핵심 모듈
pub struct BlockchainClient {
    /// HTTP Provider (읽기 전용 작업)
    http_provider: Arc<Provider<Http>>,
    /// WebSocket Provider (실시간 이벤트 수신)
    ws_provider: Option<Arc<Provider<Ws>>>,
    /// 체인 ID
    chain_id: u64,
    /// 현재 블록 번호
    current_block: Arc<RwLock<u64>>,
    /// 가스 가격 캐시
    gas_price_cache: Arc<RwLock<U256>>,
    /// 네트워크 상태
    is_connected: Arc<RwLock<bool>>,
}

impl BlockchainClient {
    /// 새로운 블록체인 클라이언트 생성
    pub async fn new(http_url: &str, ws_url: Option<&str>) -> Result<Self> {
        info!("🔌 블록체인 RPC 클라이언트 초기화: {}", http_url);
        
        // HTTP Provider 생성
        let http_provider = Provider::<Http>::try_from(http_url)?;
        let http_provider = Arc::new(http_provider);
        
        // 체인 ID 확인
        let chain_id = http_provider.get_chainid().await?.as_u64();
        info!("🔗 체인 ID: {}", chain_id);
        
        // 현재 블록 번호 가져오기
        let current_block = http_provider.get_block_number().await?.as_u64();
        info!("📦 현재 블록: {}", current_block);
        
        // WebSocket Provider 생성 (옵션)
        let ws_provider = if let Some(ws_url) = ws_url {
            match Provider::<Ws>::connect(ws_url).await {
                Ok(provider) => {
                    info!("✅ WebSocket 연결 성공: {}", ws_url);
                    Some(Arc::new(provider))
                }
                Err(e) => {
                    warn!("⚠️ WebSocket 연결 실패: {} - {}", ws_url, e);
                    None
                }
            }
        } else {
            None
        };
        
        // 초기 가스 가격 가져오기
        let gas_price = http_provider.get_gas_price().await?;
        
        Ok(Self {
            http_provider,
            ws_provider,
            chain_id,
            current_block: Arc::new(RwLock::new(current_block)),
            gas_price_cache: Arc::new(RwLock::new(gas_price)),
            is_connected: Arc::new(RwLock::new(true)),
        })
    }
    
    /// 현재 블록 번호 조회
    pub async fn get_current_block(&self) -> Result<u64> {
        let block_number = self.http_provider.get_block_number().await?.as_u64();
        *self.current_block.write().await = block_number;
        Ok(block_number)
    }
    
    /// 블록 조회
    pub async fn get_block(&self, block_number: u64) -> Result<Option<Block<H256>>> {
        Ok(self.http_provider
            .get_block(BlockNumber::Number(block_number.into()))
            .await?)
    }
    
    /// 트랜잭션 조회
    pub async fn get_transaction(&self, tx_hash: H256) -> Result<Option<Transaction>> {
        Ok(self.http_provider.get_transaction(tx_hash).await?)
    }
    
    /// 펜딩 트랜잭션 조회
    pub async fn get_pending_transactions(&self) -> Result<Vec<Transaction>> {
        // WebSocket이 있으면 실시간 스트림 사용
        if let Some(ws) = &self.ws_provider {
            let mut stream = ws.watch_pending_transactions().await?;
            let mut transactions = Vec::new();
            
            // 최대 100개까지만 수집
            for _ in 0..100 {
                if let Some(tx_hash) = stream.next().await {
                    if let Some(tx) = self.get_transaction(tx_hash).await? {
                        transactions.push(tx);
                    }
                }
            }
            
            Ok(transactions)
        } else {
            // HTTP fallback - 빈 벡터 반환
            warn!("WebSocket 연결 없음 - 펜딩 트랜잭션 조회 불가");
            Ok(Vec::new())
        }
    }
    
    /// 스마트 컨트랙트 호출 (읽기 전용)
    pub async fn call_contract<T: ethers::abi::Detokenize>(
        &self,
        contract_address: Address,
        abi: &Abi,
        function_name: &str,
        args: impl ethers::abi::Tokenize,
    ) -> Result<T> {
        let contract = Contract::new(contract_address, abi.clone(), self.http_provider.clone());
        
        let result: T = contract
            .method(function_name, args)?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 토큰 잔액 조회
    pub async fn get_token_balance(&self, token_address: Address, wallet_address: Address) -> Result<U256> {
        // ERC20 balanceOf ABI
        let abi_json = r#"[{"constant":true,"inputs":[{"name":"_owner","type":"address"}],"name":"balanceOf","outputs":[{"name":"balance","type":"uint256"}],"type":"function"}]"#;
        let abi: Abi = serde_json::from_str(abi_json)?;
        
        let balance: U256 = self.call_contract(
            token_address,
            &abi,
            "balanceOf",
            wallet_address,
        ).await?;
        
        Ok(balance)
    }
    
    /// ETH 잔액 조회
    pub async fn get_eth_balance(&self, address: Address) -> Result<U256> {
        Ok(self.http_provider.get_balance(address, None).await?)
    }
    
    /// 가스 가격 조회 (EIP-1559)
    pub async fn get_gas_price(&self) -> Result<(U256, U256)> {
        let block = self.http_provider.get_block(BlockNumber::Latest).await?
            .ok_or_else(|| anyhow!("최신 블록을 가져올 수 없습니다"))?;
        
        let base_fee = block.base_fee_per_gas
            .ok_or_else(|| anyhow!("Base fee를 가져올 수 없습니다"))?;
        
        // Priority fee 계산 (2 gwei 기본값)
        let priority_fee = U256::from(2_000_000_000u64);
        
        // 캐시 업데이트
        *self.gas_price_cache.write().await = base_fee + priority_fee;
        
        Ok((base_fee, priority_fee))
    }
    
    /// 동적 가스 가격 계산 (경쟁 상황 고려)
    pub async fn calculate_competitive_gas_price(&self, urgency: f64) -> Result<U256> {
        let (base_fee, priority_fee) = self.get_gas_price().await?;
        
        // urgency: 0.0 (낮음) ~ 1.0 (매우 높음)
        let multiplier = 1.0 + urgency * 2.0; // 최대 3배까지
        let adjusted_priority = U256::from((priority_fee.as_u64() as f64 * multiplier) as u64);
        
        Ok(base_fee + adjusted_priority)
    }
    
    /// 이벤트 로그 조회
    pub async fn get_logs(&self, filter: Filter) -> Result<Vec<Log>> {
        Ok(self.http_provider.get_logs(&filter).await?)
    }
    
    /// 스마트 컨트랙트 코드 조회
    pub async fn get_code(&self, address: Address) -> Result<ethers::types::Bytes> {
        Ok(self.http_provider.get_code(address, None).await?)
    }
    
    /// Nonce 조회
    pub async fn get_nonce(&self, address: Address) -> Result<U256> {
        Ok(self.http_provider.get_transaction_count(address, None).await?)
    }
    
    /// 트랜잭션 영수증 조회
    pub async fn get_transaction_receipt(&self, tx_hash: H256) -> Result<Option<ethers::types::TransactionReceipt>> {
        Ok(self.http_provider.get_transaction_receipt(tx_hash).await?)
    }
    
    /// 블록 구독 (WebSocket 필요)
    pub async fn subscribe_blocks<F>(&self, _callback: F) -> Result<()> 
    where
        F: Fn(Block<H256>) + Send + Sync + 'static,
    {
        if let Some(ws) = &self.ws_provider {
            let ws_clone = ws.clone();
            
            tokio::spawn(async move {
                match ws_clone.watch_blocks().await {
                    Ok(mut stream) => {
                        while let Some(block_hash) = stream.next().await {
                            debug!("새 블록: {:?}", block_hash);
                            // callback 사용 시 블록 상세 정보 조회하여 전달
                            // if let Ok(Some(block)) = ws_clone.get_block(block_hash).await {
                            //     callback(block);
                            // }
                        }
                    }
                    Err(e) => {
                        error!("블록 구독 오류: {}", e);
                    }
                }
            });
            
            Ok(())
        } else {
            Err(anyhow!("WebSocket 연결이 필요합니다"))
        }
    }
    
    /// 펜딩 트랜잭션 구독 (WebSocket 필요)
    pub async fn subscribe_pending_transactions<F>(&self, _callback: F) -> Result<()>
    where
        F: Fn(H256) + Send + Sync + 'static,
    {
        if let Some(ws) = &self.ws_provider {
            let ws_clone = ws.clone();
            
            tokio::spawn(async move {
                match ws_clone.watch_pending_transactions().await {
                    Ok(mut stream) => {
                        while let Some(tx_hash) = stream.next().await {
                            debug!("새 펜딩 트랜잭션: {:?}", tx_hash);
                            // callback(tx_hash); // callback 사용 시 활성화
                        }
                    }
                    Err(e) => {
                        error!("펜딩 트랜잭션 구독 오류: {}", e);
                    }
                }
            });
            
            Ok(())
        } else {
            Err(anyhow!("WebSocket 연결이 필요합니다"))
        }
    }
    
    /// 연결 상태 확인
    pub async fn is_connected(&self) -> bool {
        // 간단한 호출로 연결 상태 확인
        match self.http_provider.get_block_number().await {
            Ok(_) => {
                *self.is_connected.write().await = true;
                true
            }
            Err(_) => {
                *self.is_connected.write().await = false;
                false
            }
        }
    }
    
    /// Provider 참조 반환 (고급 사용)
    pub fn get_provider(&self) -> Arc<Provider<Http>> {
        self.http_provider.clone()
    }
    
    /// WebSocket Provider 참조 반환 (고급 사용)
    pub fn get_ws_provider(&self) -> Option<Arc<Provider<Ws>>> {
        self.ws_provider.clone()
    }
}

/// 멀티체인 RPC 매니저
pub struct MultiChainRpcManager {
    clients: HashMap<u64, Arc<BlockchainClient>>,
}

impl MultiChainRpcManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
    
    /// 체인 클라이언트 추가
    pub async fn add_chain(&mut self, chain_id: u64, http_url: &str, ws_url: Option<&str>) -> Result<()> {
        let client = BlockchainClient::new(http_url, ws_url).await?;
        self.clients.insert(chain_id, Arc::new(client));
        info!("✅ 체인 {} 클라이언트 추가됨", chain_id);
        Ok(())
    }
    
    /// 체인 클라이언트 가져오기
    pub fn get_client(&self, chain_id: u64) -> Option<Arc<BlockchainClient>> {
        self.clients.get(&chain_id).cloned()
    }
    
    /// 모든 체인 ID 반환
    pub fn get_chain_ids(&self) -> Vec<u64> {
        self.clients.keys().cloned().collect()
    }
}