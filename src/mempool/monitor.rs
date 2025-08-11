// Mempool monitoring functionality

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, debug, error, warn};
use ethers::providers::{Provider, Ws, Middleware};
use ethers::types::{Transaction as EthersTransaction, BlockNumber, TxHash, H256};
use futures::StreamExt;

use crate::config::Config;
use crate::types::Transaction;

#[derive(Clone)]
pub struct MempoolMonitor {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    is_running: bool,
}

impl MempoolMonitor {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        Ok(Self {
            config,
            provider,
            is_running: false,
        })
    }

    /// 멤풀에서 대기 중인 트랜잭션들을 가져옵니다
    pub async fn get_pending_transactions(&self) -> Result<Vec<Transaction>> {
        let mut transactions = Vec::new();
        
        // 현재 블록 번호 가져오기
        let current_block = self.provider.get_block_number().await?.as_u64();
        
        // 최근 블록들의 트랜잭션들을 가져와서 멤풀 상태 추정
        for block_offset in 0..5 {
            let block_number = current_block - block_offset;
            if let Ok(Some(block)) = self.provider.get_block_with_txs(BlockNumber::Number(block_number.into())).await {
                for tx in block.transactions {
                    if self.should_process_transaction(&tx) {
                        if let Ok(converted_tx) = self.convert_ethers_transaction(tx).await {
                            transactions.push(converted_tx);
                        }
                    }
                }
            }
        }
        
        debug!("Found {} pending transactions", transactions.len());
        Ok(transactions)
    }

    /// 멤풀 모니터링을 시작합니다
    pub async fn start_monitoring(&mut self, tx_sender: mpsc::UnboundedSender<Transaction>) -> Result<()> {
        info!("🚀 멤풀 모니터링 시작...");
        self.is_running = true;
        
        let provider = Arc::clone(&self.provider);
        let config = Arc::clone(&self.config);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            
            loop {
                interval.tick().await;
                
                // 새로운 트랜잭션들을 가져와서 처리
                match Self::get_new_transactions(&provider, &config).await {
                    Ok(new_transactions) => {
                        for tx in new_transactions {
                            if let Err(e) = tx_sender.send(tx) {
                                error!("❌ 트랜잭션 전송 실패: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 트랜잭션 가져오기 실패: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }

    /// 새로운 트랜잭션들을 가져옵니다
    async fn get_new_transactions(provider: &Arc<Provider<Ws>>, config: &Arc<Config>) -> Result<Vec<Transaction>> {
        let mut transactions = Vec::new();
        
        // 최근 블록에서 트랜잭션 가져오기
        let current_block = provider.get_block_number().await?.as_u64();
        
        // 최근 3개 블록 확인
        for block_offset in 0..3 {
            let block_number = current_block - block_offset;
            if let Ok(Some(block)) = provider.get_block_with_txs(BlockNumber::Number(block_number.into())).await {
                for tx in block.transactions {
                    // 필터링 조건 적용
                    if Self::should_process_transaction_static(&tx, config) {
                        if let Ok(converted_tx) = Self::convert_ethers_transaction_static(tx).await {
                            transactions.push(converted_tx);
                        }
                    }
                }
            }
        }
        
        Ok(transactions)
    }

    /// 트랜잭션이 처리 대상인지 확인합니다
    fn should_process_transaction(&self, tx: &EthersTransaction) -> bool {
        // 최소 가스 가격 필터
        let min_gas_price = self.config.performance.mempool_filter_min_gas_price.parse::<u64>().unwrap_or(10);
        if tx.gas_price.unwrap_or_default().as_u64() < min_gas_price * 1_000_000_000 {
            return false;
        }
        
        // 최소 가치 필터
        let min_value = self.config.performance.mempool_filter_min_value.parse::<u64>().unwrap_or(0);
        if tx.value.as_u64() < min_value * 1_000_000_000_000_000_000 {
            return false;
        }
        
        // 데이터 크기 필터 (스왑 트랜잭션 등)
        if tx.input.len() < 4 {
            return false; // 함수 호출이 아닌 단순 ETH 전송
        }
        
        true
    }

    /// 정적 메서드로 트랜잭션 필터링
    fn should_process_transaction_static(tx: &EthersTransaction, config: &Arc<Config>) -> bool {
        // 최소 가스 가격 필터
        let min_gas_price = config.performance.mempool_filter_min_gas_price.parse::<u64>().unwrap_or(10);
        if tx.gas_price.unwrap_or_default().as_u64() < min_gas_price * 1_000_000_000 {
            return false;
        }
        
        // 최소 가치 필터
        let min_value = config.performance.mempool_filter_min_value.parse::<u64>().unwrap_or(0);
        if tx.value.as_u64() < min_value * 1_000_000_000_000_000_000 {
            return false;
        }
        
        // 데이터 크기 필터 (스왑 트랜잭션 등)
        if tx.input.len() < 4 {
            return false; // 함수 호출이 아닌 단순 ETH 전송
        }
        
        true
    }

    /// Ethers 트랜잭션을 내부 Transaction 타입으로 변환합니다
    async fn convert_ethers_transaction(&self, tx: EthersTransaction) -> Result<Transaction> {
        let timestamp = chrono::Utc::now();
        
        Ok(Transaction {
            hash: alloy::primitives::B256::from_slice(&tx.hash.0),
            from: alloy::primitives::Address::from_slice(&tx.from.0),
            to: tx.to.map(|addr| alloy::primitives::Address::from_slice(&addr.0)),
            value: {
                let mut bytes = [0u8; 32];
                tx.value.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            gas_price: {
                let gas_price = tx.gas_price.unwrap_or_default();
                let mut bytes = [0u8; 32];
                gas_price.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            gas_limit: {
                let gas = tx.gas;
                let mut bytes = [0u8; 32];
                gas.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            data: tx.input.to_vec(),
            nonce: tx.nonce.as_u64(),
            timestamp,
            block_number: tx.block_number.map(|bn| bn.as_u64()),
        })
    }

    /// 정적 메서드로 트랜잭션 변환
    async fn convert_ethers_transaction_static(tx: EthersTransaction) -> Result<Transaction> {
        let timestamp = chrono::Utc::now();
        
        Ok(Transaction {
            hash: alloy::primitives::B256::from_slice(&tx.hash.0),
            from: alloy::primitives::Address::from_slice(&tx.from.0),
            to: tx.to.map(|addr| alloy::primitives::Address::from_slice(&addr.0)),
            value: {
                let mut bytes = [0u8; 32];
                tx.value.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            gas_price: {
                let gas_price = tx.gas_price.unwrap_or_default();
                let mut bytes = [0u8; 32];
                gas_price.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            gas_limit: {
                let gas = tx.gas;
                let mut bytes = [0u8; 32];
                gas.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            data: tx.input.to_vec(),
            nonce: tx.nonce.as_u64(),
            timestamp,
            block_number: tx.block_number.map(|bn| bn.as_u64()),
        })
    }

    /// 특정 트랜잭션 해시로 트랜잭션을 가져옵니다
    pub async fn get_transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>> {
        if let Ok(Some(tx)) = self.provider.get_transaction(hash).await {
            if self.should_process_transaction(&tx) {
                return Ok(Some(self.convert_ethers_transaction(tx).await?));
            }
        }
        Ok(None)
    }

    /// 모니터링을 중지합니다
    pub fn stop(&mut self) {
        self.is_running = false;
    }

    /// 모니터링 상태를 확인합니다
    pub fn is_running(&self) -> bool {
        self.is_running
    }
} 