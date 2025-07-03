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
        if tx.gas_price.as_u64() < min_gas_price * 1_000_000_000 {
            return false;
        }
        
        // 최소 가치 필터
        let min_value = self.config.performance.mempool_filter_min_value.parse::<u64>().unwrap_or(0);
        if tx.value.as_u64() < min_value * 1_000_000_000_000_000_000 {
            return false;
        }
        
        // 데이터 크기 필터 (스왑 트랜잭션 등)
        if tx.data.len() < 4 {
            return false; // 함수 호출이 아닌 단순 ETH 전송
        }
        
        true
    }

    /// 정적 메서드로 트랜잭션 필터링
    fn should_process_transaction_static(tx: &EthersTransaction, config: &Arc<Config>) -> bool {
        // 최소 가스 가격 필터
        let min_gas_price = config.performance.mempool_filter_min_gas_price.parse::<u64>().unwrap_or(10);
        if tx.gas_price.as_u64() < min_gas_price * 1_000_000_000 {
            return false;
        }
        
        // 최소 가치 필터
        let min_value = config.performance.mempool_filter_min_value.parse::<u64>().unwrap_or(0);
        if tx.value.as_u64() < min_value * 1_000_000_000_000_000_000 {
            return false;
        }
        
        // 데이터 크기 필터 (스왑 트랜잭션 등)
        if tx.data.len() < 4 {
            return false; // 함수 호출이 아닌 단순 ETH 전송
        }
        
        true
    }

    /// Ethers 트랜잭션을 내부 Transaction 타입으로 변환합니다
    async fn convert_ethers_transaction(&self, tx: EthersTransaction) -> Result<Transaction> {
        let timestamp = chrono::Utc::now();
        
        Ok(Transaction {
            hash: tx.hash,
            from: tx.from,
            to: tx.to,
            value: tx.value,
            gas_price: tx.gas_price,
            gas_limit: tx.gas_limit,
            data: tx.data.to_vec(),
            nonce: tx.nonce.as_u64(),
            timestamp,
            block_number: tx.block_number.map(|bn| bn.as_u64()),
        })
    }

    /// 정적 메서드로 트랜잭션 변환
    async fn convert_ethers_transaction_static(tx: EthersTransaction) -> Result<Transaction> {
        let timestamp = chrono::Utc::now();
        
        Ok(Transaction {
            hash: tx.hash,
            from: tx.from,
            to: tx.to,
            value: tx.value,
            gas_price: tx.gas_price,
            gas_limit: tx.gas_limit,
            data: tx.data.to_vec(),
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

    /// 멤풀 모니터링을 중지합니다
    pub fn stop(&mut self) {
        self.is_running = false;
        info!("⏹️ 멤풀 모니터링 중지됨");
    }

    /// 멤풀 모니터링 상태를 확인합니다
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

pub mod filters {
    use std::collections::HashSet;
    use ethers::types::{H160, Transaction as EthersTransaction};

    /// 주요 DEX 라우터 주소들을 반환합니다
    pub fn get_dex_routers() -> HashSet<H160> {
        let mut routers = HashSet::new();
        
        // Uniswap V2
        routers.insert("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap());
        
        // SushiSwap
        routers.insert("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap());
        
        // PancakeSwap V2
        routers.insert("0x10ED43C718714eb63d5aA57B78B54704E256024E".parse().unwrap());
        
        routers
    }

    /// 주요 대출 프로토콜 주소들을 반환합니다
    pub fn get_lending_pools() -> HashSet<H160> {
        let mut pools = HashSet::new();
        
        // Aave V2
        pools.insert("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap());
        
        // Compound V3
        pools.insert("0xc3d688B66703497DAA19211EEdff47fB25365b65".parse().unwrap());
        
        pools
    }

    /// 트랜잭션이 DEX 스왑인지 확인합니다
    pub fn is_dex_swap(tx: &EthersTransaction) -> bool {
        if tx.data.len() < 4 {
            return false;
        }
        
        let function_selector = &tx.data[0..4];
        
        // 주요 스왑 함수 시그니처들
        let swap_functions = vec![
            vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            vec![0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            vec![0x18, 0xcb, 0xa5, 0xe5], // swapExactTokensForETH
        ];
        
        swap_functions.contains(function_selector)
    }

    /// 트랜잭션이 청산 호출인지 확인합니다
    pub fn is_liquidation_call(tx: &EthersTransaction) -> bool {
        if tx.data.len() < 4 {
            return false;
        }
        
        let function_selector = &tx.data[0..4];
        
        // 청산 함수 시그니처들
        let liquidation_functions = vec![
            vec![0xe8, 0xed, 0xa9, 0xdf], // Aave liquidationCall
            vec![0x4c, 0x0b, 0x5b, 0x3e], // Compound liquidate
            vec![0x1d, 0x26, 0x3b, 0x3c], // MakerDAO bite
        ];
        
        liquidation_functions.contains(function_selector)
    }

    /// 트랜잭션이 상당한 가치를 가지고 있는지 확인합니다
    pub fn has_significant_value(tx: &EthersTransaction, min_eth: f64) -> bool {
        let min_value_wei = (min_eth * 1_000_000_000_000_000_000.0) as u128;
        tx.value.as_u128() >= min_value_wei
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::{Transaction as EthersTransaction, H160, H256, U256};

    #[test]
    fn test_dex_router_filtering() {
        let routers = filters::get_dex_routers();
        assert!(routers.contains(&"0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()));
        assert!(routers.contains(&"0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap()));
    }

    #[test]
    fn test_dex_swap_detection() {
        let mut tx = EthersTransaction::default();
        
        // swapExactTokensForTokens 함수 시그니처
        tx.data = vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00, 0x00, 0x00].into();
        
        assert!(filters::is_dex_swap(&tx));
        
        // 일반 ETH 전송
        tx.data = vec![].into();
        assert!(!filters::is_dex_swap(&tx));
    }

    #[test]
    fn test_liquidation_call_detection() {
        let mut tx = EthersTransaction::default();
        
        // Aave liquidationCall 함수 시그니처
        tx.data = vec![0xe8, 0xed, 0xa9, 0xdf, 0x00, 0x00, 0x00, 0x00].into();
        
        assert!(filters::is_liquidation_call(&tx));
        
        // 일반 스왑
        tx.data = vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00, 0x00, 0x00].into();
        assert!(!filters::is_liquidation_call(&tx));
    }

    #[test]
    fn test_significant_value_filtering() {
        let mut tx = EthersTransaction::default();
        
        // 1 ETH
        tx.value = U256::from_str_radix("1000000000000000000", 10).unwrap();
        assert!(filters::has_significant_value(&tx, 0.5));
        
        // 0.1 ETH
        tx.value = U256::from_str_radix("100000000000000000", 10).unwrap();
        assert!(!filters::has_significant_value(&tx, 0.5));
    }
}

pub use MempoolMonitor; 