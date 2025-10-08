use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http, Ws, Middleware, StreamExt},
    types::{Block, Transaction, H256, U256, Address, BlockNumber, Filter, Log, TransactionRequest},
    abi::Abi,
    contract::Contract,
    signers::{LocalWallet, Signer},
    middleware::SignerMiddleware,
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
    /// 트랜잭션 서명용 Wallet
    wallet: Option<LocalWallet>,
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
        Self::new_with_wallet(http_url, ws_url, None).await
    }

    /// Wallet과 함께 블록체인 클라이언트 생성
    pub async fn new_with_wallet(http_url: &str, ws_url: Option<&str>, private_key: Option<&str>) -> Result<Self> {
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

        // Wallet 생성 (private key가 제공된 경우)
        let wallet = if let Some(pk) = private_key {
            let wallet: LocalWallet = pk.parse()
                .map_err(|e| anyhow!("Invalid private key: {}", e))?;
            let wallet = wallet.with_chain_id(chain_id);
            info!("🔑 Wallet 초기화 완료: {}", wallet.address());
            Some(wallet)
        } else {
            warn!("⚠️ Private key 없음 - 트랜잭션 서명 불가 (읽기 전용 모드)");
            None
        };

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
            wallet,
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
        
        // 동적 Priority fee 계산
        let priority_fee = self.calculate_optimal_priority_fee().await?;
        
        // 캐시 업데이트
        *self.gas_price_cache.write().await = base_fee + priority_fee;
        
        debug!("⛽ 가스 가격: base_fee={} gwei, priority_fee={} gwei", 
               base_fee.as_u128() / 1_000_000_000,
               priority_fee.as_u128() / 1_000_000_000);
        
        Ok((base_fee, priority_fee))
    }
    
    /// 최적 Priority Fee 계산
    async fn calculate_optimal_priority_fee(&self) -> Result<U256> {
        // 최근 블록들의 priority fee 분석
        let recent_blocks = self.get_recent_blocks(10).await?;
        let mut priority_fees = Vec::new();
        
        for block in recent_blocks {
            for tx_hash in &block.transactions {
                if let Ok(Some(tx)) = self.http_provider.get_transaction(*tx_hash).await {
                    if let Some(priority_fee) = self.extract_priority_fee(&tx).await? {
                        priority_fees.push(priority_fee);
                    }
                }
            }
        }
        
        if priority_fees.is_empty() {
            // 기본값: 2 gwei
            return Ok(U256::from(2_000_000_000u64));
        }
        
        // 중간값 계산 (더 안정적)
        priority_fees.sort();
        let median_index = priority_fees.len() / 2;
        let median_priority_fee = priority_fees[median_index];
        
        // 10% 추가 (경쟁력 확보)
        let optimal_priority_fee = median_priority_fee * U256::from(110) / U256::from(100);
        
        // 최소 1 gwei, 최대 50 gwei 제한
        let min_priority_fee = U256::from(1_000_000_000u64);
        let max_priority_fee = U256::from(50_000_000_000u64);
        
        Ok(optimal_priority_fee.max(min_priority_fee).min(max_priority_fee))
    }
    
    /// 최근 블록들 조회
    async fn get_recent_blocks(&self, count: usize) -> Result<Vec<Block<H256>>> {
        let mut blocks = Vec::new();
        let current_block = self.get_current_block().await?;
        
        for i in 0..count {
            let block_number = current_block.saturating_sub(i as u64);
            if let Some(block) = self.get_block(block_number).await? {
                blocks.push(block);
            }
        }
        
        Ok(blocks)
    }
    
    /// 트랜잭션에서 Priority Fee 추출
    async fn extract_priority_fee(&self, tx: &Transaction) -> Result<Option<U256>> {
        // EIP-1559 트랜잭션인지 확인
        if let Some(max_fee_per_gas) = tx.max_fee_per_gas {
            if let Some(max_priority_fee_per_gas) = tx.max_priority_fee_per_gas {
                // Base fee는 블록에서 가져와야 하지만, 간단화를 위해 0으로 가정
                // 실제로는 블록의 base_fee_per_gas를 사용해야 함
                let base_fee = U256::from(20_000_000_000u64); // 20 gwei 가정
                let priority_fee = if max_priority_fee_per_gas > base_fee {
                    max_priority_fee_per_gas - base_fee
                } else {
                    max_priority_fee_per_gas
                };
                return Ok(Some(priority_fee));
            }
        }
        
        // Legacy 트랜잭션의 경우 gas_price 사용
        if let Some(gas_price) = tx.gas_price {
            let base_fee = U256::from(20_000_000_000u64); // 20 gwei 가정
            if gas_price > base_fee {
                return Ok(Some(gas_price - base_fee));
            }
        }
        
        Ok(None)
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
    
    /// 트랜잭션 전송 (Wallet으로 서명)
    pub async fn send_transaction(&self, tx: TransactionRequest) -> Result<H256> {
        info!("📤 트랜잭션 전송 시작: to={:?}, value={:?}", tx.to, tx.value);

        // Wallet이 없으면 에러
        let wallet = self.wallet.as_ref()
            .ok_or_else(|| anyhow!("트랜잭션 전송 불가: Wallet이 설정되지 않음. new_with_wallet()을 사용하세요."))?;

        // SignerMiddleware 생성
        let client = SignerMiddleware::new(self.http_provider.clone(), wallet.clone());

        // 트랜잭션 전송 (자동 서명)
        let pending_tx = client.send_transaction(tx, None).await?;

        // 트랜잭션 해시 반환
        let tx_hash = *pending_tx;
        info!("✅ 트랜잭션 제출 성공: {} (from: {})", tx_hash, wallet.address());

        Ok(tx_hash)
    }
    
    /// 트랜잭션 전송 및 영수증 대기
    pub async fn send_transaction_and_wait(&self, tx: TransactionRequest) -> Result<ethers::types::TransactionReceipt> {
        info!("📤 트랜잭션 전송 및 영수증 대기 시작");

        // Wallet이 없으면 에러
        let wallet = self.wallet.as_ref()
            .ok_or_else(|| anyhow!("트랜잭션 전송 불가: Wallet이 설정되지 않음"))?;

        // SignerMiddleware 생성
        let client = SignerMiddleware::new(self.http_provider.clone(), wallet.clone());

        // 트랜잭션 전송 및 대기
        let pending_tx = client.send_transaction(tx, None).await?;
        let receipt = pending_tx.await?
            .ok_or_else(|| anyhow!("트랜잭션 영수증을 받을 수 없습니다"))?;

        info!("✅ 트랜잭션 확인됨: block={}, gas_used={}",
               receipt.block_number.unwrap_or_default(), receipt.gas_used.unwrap_or_default());

        Ok(receipt)
    }
    
    /// 가스 추정
    pub async fn estimate_gas(&self, tx: &TransactionRequest) -> Result<U256> {
        use ethers::types::transaction::eip2718::TypedTransaction;
        let typed_tx: TypedTransaction = tx.clone().into();
        let gas_estimate = self.http_provider.estimate_gas(&typed_tx, None).await?;
        debug!("가스 추정: {} gas", gas_estimate);
        Ok(gas_estimate)
    }

    /// Wallet 주소 조회
    pub fn get_wallet_address(&self) -> Option<Address> {
        self.wallet.as_ref().map(|w| w.address())
    }

    /// Wallet이 설정되어 있는지 확인
    pub fn has_wallet(&self) -> bool {
        self.wallet.is_some()
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