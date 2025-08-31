use std::sync::Arc;
use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug, error};
use std::collections::HashMap;
use std::time::{Instant, Duration};
use ethers::providers::{Provider, Ws, Middleware};
use ethers::types::H256;
use alloy::primitives::Address;

use crate::config::Config;
use crate::types::Transaction;
use crate::mempool::MempoolMonitor;
use crate::mocks::{is_mock_mode, MockMempoolMonitor};

pub struct CoreMempoolMonitor {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    mempool_monitor: Option<Arc<MempoolMonitor>>,
    mock_mempool_monitor: Option<Arc<MockMempoolMonitor>>,
    is_running: Arc<RwLock<bool>>,
    transaction_cache: Arc<RwLock<HashMap<H256, Transaction>>>,
    stats: Arc<RwLock<MempoolStats>>,
    tx_sender: Arc<RwLock<Option<mpsc::UnboundedSender<Transaction>>>>,
}

#[derive(Debug, Clone)]
pub struct MempoolStats {
    pub transactions_received: u64,
    pub transactions_processed: u64,
    pub transactions_filtered: u64,
    pub avg_processing_time_ms: f64,
    pub last_transaction_time: Option<Instant>,
    pub cache_size: usize,
}

impl CoreMempoolMonitor {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        let stats = MempoolStats {
            transactions_received: 0,
            transactions_processed: 0,
            transactions_filtered: 0,
            avg_processing_time_ms: 0.0,
            last_transaction_time: None,
            cache_size: 0,
        };
        
        let (mempool_monitor, mock_mempool_monitor) = if is_mock_mode() {
            info!("🎭 CoreMempoolMonitor initialized with mock mempool monitor");
            let mock_monitor = MockMempoolMonitor::new(Arc::clone(&config)).await?;
            (None, Some(Arc::new(mock_monitor)))
        } else {
            info!("🌐 CoreMempoolMonitor initialized with real mempool monitor");
            let real_monitor = MempoolMonitor::new(Arc::clone(&config), Arc::clone(&provider)).await?;
            (Some(Arc::new(real_monitor)), None)
        };
        
        Ok(Self {
            config,
            provider,
            mempool_monitor,
            mock_mempool_monitor,
            is_running: Arc::new(RwLock::new(false)),
            transaction_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(stats)),
            tx_sender: Arc::new(RwLock::new(None)),
        })
    }

    /// 멤풀 모니터링 시작
    pub async fn start(&self, tx_sender: mpsc::UnboundedSender<Transaction>) -> Result<()> {
        info!("🚀 CoreMempoolMonitor 시작 중...");
        
        let mut is_running = self.is_running.write().await;
        *is_running = true;
        
        // 트랜잭션 전송자 저장
        *self.tx_sender.write().await = Some(tx_sender.clone());
        
        // 멤풀 모니터링 시작 (mock 또는 real monitor 사용)
        let tx_sender_clone = tx_sender;
        
        if let Some(mock_monitor) = &self.mock_mempool_monitor {
            let mock_monitor_clone = Arc::clone(mock_monitor);
            tokio::spawn(async move {
                if let Err(e) = mock_monitor_clone.start_monitoring(tx_sender_clone).await {
                    error!("❌ 🎭 Mock 멤풀 모니터링 시작 실패: {}", e);
                }
            });
        } else if let Some(_real_monitor) = &self.mempool_monitor {
            // 실제 구현에서는 별도의 비동기 태스크에서 멤풀 모니터링 시작
            // Arc<MempoolMonitor>에서 직접 start_monitoring을 호출할 수 없으므로
            // 실제 구현에서는 내부 상태를 변경하지 않는 방식으로 구현해야 함
            info!("🌐 실제 멤풀 모니터링은 별도 구현 필요");
        } else {
            return Err(anyhow::anyhow!("No mempool monitor available"));
        }
        
        // 캐시 정리 태스크 시작
        let cache = Arc::clone(&self.transaction_cache);
        let stats = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5분마다
            
            loop {
                interval.tick().await;
                
                // 오래된 트랜잭션 제거 (1시간 이상)
                let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(1);
                let mut cache_guard = cache.write().await;
                let initial_size = cache_guard.len();
                
                cache_guard.retain(|_, tx| tx.timestamp > cutoff_time);
                
                let removed_count = initial_size - cache_guard.len();
                if removed_count > 0 {
                    debug!("🧹 {}개 오래된 트랜잭션 캐시에서 제거됨", removed_count);
                }
                
                // 통계 업데이트
                let mut stats_guard = stats.write().await;
                stats_guard.cache_size = cache_guard.len();
            }
        });
        
        info!("✅ CoreMempoolMonitor 시작됨");
        Ok(())
    }

    /// 멤풀 모니터링 중지
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 CoreMempoolMonitor 중지 중...");
        
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        
        // 멤풀 모니터 중지 (unsafe 코드 제거됨)
        
        info!("✅ CoreMempoolMonitor 중지됨");
        Ok(())
    }

    /// 트랜잭션 처리
    pub async fn process_transaction(&self, tx: Transaction) -> Result<()> {
        let start_time = Instant::now();
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.transactions_received += 1;
        stats.last_transaction_time = Some(Instant::now());
        
        // 트랜잭션 필터링
        if !self.should_process_transaction(&tx).await? {
            stats.transactions_filtered += 1;
            return Ok(());
        }
        
        // 캐시에 저장
        let mut cache = self.transaction_cache.write().await;
        let tx_hash_h256 = H256::from_slice(tx.hash.as_slice());
        cache.insert(tx_hash_h256, tx.clone());
        
        // 트랜잭션 전송
        if let Some(sender) = self.tx_sender.read().await.as_ref() {
            if let Err(e) = sender.send(tx) {
                error!("❌ 트랜잭션 전송 실패: {}", e);
            } else {
                stats.transactions_processed += 1;
            }
        }
        
        // 처리 시간 통계 업데이트
        let processing_time = start_time.elapsed();
        let processing_time_ms = processing_time.as_millis() as f64;
        let total_processed = stats.transactions_processed as f64;
        
        stats.avg_processing_time_ms = (stats.avg_processing_time_ms * (total_processed - 1.0) + processing_time_ms) / total_processed;
        
        Ok(())
    }

    /// 트랜잭션이 처리 대상인지 확인
    async fn should_process_transaction(&self, tx: &Transaction) -> Result<bool> {
        // 최소 가스 가격 필터
        let min_gas_price = self.config.performance.mempool_filter_min_gas_price.parse::<u64>().unwrap_or(10);
        if tx.gas_price.to::<u64>() < min_gas_price * 1_000_000_000 {
            return Ok(false);
        }
        
        // 최소 가치 필터
        let min_value = self.config.performance.mempool_filter_min_value.parse::<u64>().unwrap_or(0);
        if tx.value.as_limbs()[0] < min_value as u64 * 1_000_000_000_000_000_000 {
            return Ok(false);
        }
        
        // 데이터 크기 필터 (스왑 트랜잭션 등)
        if tx.data.len() < 4 {
            return Ok(false); // 함수 호출이 아닌 단순 ETH 전송
        }
        
        // 중복 트랜잭션 확인
        let cache = self.transaction_cache.read().await;
        let tx_hash_h256 = H256::from_slice(tx.hash.as_slice());
        if cache.contains_key(&tx_hash_h256) {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// 특정 트랜잭션 조회
    pub async fn get_transaction(&self, hash: H256) -> Option<Transaction> {
        let cache = self.transaction_cache.read().await;
        cache.get(&hash).cloned()
    }

    /// 캐시된 트랜잭션들 조회
    pub async fn get_cached_transactions(&self) -> Vec<Transaction> {
        let cache = self.transaction_cache.read().await;
        cache.values().cloned().collect()
    }

    /// 멤풀 통계 조회
    pub async fn get_stats(&self) -> MempoolStats {
        self.stats.read().await.clone()
    }

    /// 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// 캐시 크기 조회
    pub async fn get_cache_size(&self) -> usize {
        self.transaction_cache.read().await.len()
    }

    /// 캐시 정리
    pub async fn clear_cache(&self) -> Result<()> {
        let mut cache = self.transaction_cache.write().await;
        let cleared_count = cache.len();
        cache.clear();
        
        let mut stats = self.stats.write().await;
        stats.cache_size = 0;
        
        info!("🧹 캐시 정리됨: {}개 트랜잭션 제거", cleared_count);
        Ok(())
    }

    /// 실시간 멤풀 상태 조회
    pub async fn get_mempool_status(&self) -> Result<MempoolStatus> {
        let current_block = self.provider.get_block_number().await?.as_u64();
        let pending_count = self.get_cache_size().await;
        
        Ok(MempoolStatus {
            current_block,
            pending_transactions: pending_count,
            is_monitoring: self.is_running().await,
            last_update: chrono::Utc::now(),
        })
    }

    /// 트랜잭션 검색 (캐시에서)
    pub async fn search_transactions(&self, criteria: TransactionSearchCriteria) -> Vec<Transaction> {
        let cache = self.transaction_cache.read().await;
        let mut results = Vec::new();
        
        for tx in cache.values() {
            if criteria.matches(tx) {
                results.push(tx.clone());
            }
        }
        
        results
    }
}

#[derive(Debug, Clone)]
pub struct MempoolStatus {
    pub current_block: u64,
    pub pending_transactions: usize,
    pub is_monitoring: bool,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct TransactionSearchCriteria {
    pub min_value: Option<u128>,
    pub max_value: Option<u128>,
    pub min_gas_price: Option<u64>,
    pub max_gas_price: Option<u64>,
    pub from_address: Option<Address>,
    pub to_address: Option<Address>,
    pub function_selector: Option<[u8; 4]>,
}

impl TransactionSearchCriteria {
    pub fn new() -> Self {
        Self {
            min_value: None,
            max_value: None,
            min_gas_price: None,
            max_gas_price: None,
            from_address: None,
            to_address: None,
            function_selector: None,
        }
    }

    pub fn with_min_value(mut self, value: u128) -> Self {
        self.min_value = Some(value);
        self
    }

    pub fn with_max_value(mut self, value: u128) -> Self {
        self.max_value = Some(value);
        self
    }

    pub fn with_gas_price_range(mut self, min: u64, max: u64) -> Self {
        self.min_gas_price = Some(min);
        self.max_gas_price = Some(max);
        self
    }

    pub fn with_from_address(mut self, address: Address) -> Self {
        self.from_address = Some(address);
        self
    }

    pub fn with_to_address(mut self, address: Address) -> Self {
        self.to_address = Some(address);
        self
    }

    pub fn with_function_selector(mut self, selector: [u8; 4]) -> Self {
        self.function_selector = Some(selector);
        self
    }

    pub fn matches(&self, tx: &Transaction) -> bool {
        // 가치 범위 확인
        if let Some(min_value) = self.min_value {
            if tx.value.to::<u128>() < min_value {
                return false;
            }
        }
        
        if let Some(max_value) = self.max_value {
            if tx.value.to::<u128>() > max_value {
                return false;
            }
        }
        
        // 가스 가격 범위 확인
        if let Some(min_gas_price) = self.min_gas_price {
            if tx.gas_price.to::<u64>() < min_gas_price {
                return false;
            }
        }
        
        if let Some(max_gas_price) = self.max_gas_price {
            if tx.gas_price.to::<u64>() > max_gas_price {
                return false;
            }
        }
        
        // 주소 확인
        if let Some(from_addr) = self.from_address {
            if tx.from != from_addr {
                return false;
            }
        }
        
        if let Some(to_addr) = self.to_address {
            if tx.to != Some(to_addr) {
                return false;
            }
        }
        
        // 함수 선택자 확인
        if let Some(selector) = self.function_selector {
            if tx.data.len() < 4 || &tx.data[0..4] != selector {
                return false;
            }
        }
        
        true
    }
}

impl std::fmt::Debug for CoreMempoolMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreMempoolMonitor")
            .field("config", &"Arc<Config>")
            .field("provider", &"Arc<Provider<Ws>>")
            .field("is_running", &"Arc<RwLock<bool>>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use ethers::types::{H256, H160, U256};
    use alloy::primitives::{Address, B256};
    use chrono::Utc;

    #[tokio::test]
    async fn test_transaction_search_criteria() {
        let criteria = TransactionSearchCriteria::new()
            .with_min_value(1000000000000000000u128) // 1 ETH
            .with_gas_price_range(15_000_000_000, 25_000_000_000) // 15-25 gwei
            .with_function_selector([0x7f, 0xf3, 0x6a, 0xb5]); // swapExactETHForTokens
        
        // 매칭되는 트랜잭션
        let matching_tx = Transaction {
            hash: B256::ZERO,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: alloy::primitives::U256::from(2000000000000000000u128), // 2 ETH
            gas_price: alloy::primitives::U256::from(20_000_000_000u64), // 20 gwei
            gas_limit: alloy::primitives::U256::from(200_000u64),
            data: vec![0x7f, 0xf3, 0x6a, 0xb5, 0x00, 0x00, 0x00, 0x00],
            nonce: 0,
            timestamp: Utc::now(),
            block_number: None,
        };
        
        assert!(criteria.matches(&matching_tx));
        
        // 매칭되지 않는 트랜잭션 (가치가 너무 낮음)
        let non_matching_tx = Transaction {
            hash: B256::ZERO,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: alloy::primitives::U256::from(500000000000000000u128), // 0.5 ETH
            gas_price: alloy::primitives::U256::from(20_000_000_000u64),
            gas_limit: alloy::primitives::U256::from(200_000u64),
            data: vec![0x7f, 0xf3, 0x6a, 0xb5, 0x00, 0x00, 0x00, 0x00],
            nonce: 0,
            timestamp: Utc::now(),
            block_number: None,
        };
        
        assert!(!criteria.matches(&non_matching_tx));
    }
} 