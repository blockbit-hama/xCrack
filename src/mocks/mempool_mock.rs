use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, debug, warn};
use rand::Rng;
use ethers::types::{Address, U256, H256};

use crate::config::Config;
use crate::types::Transaction;
use super::{get_mock_config, MockConfig};

#[derive(Clone)]
pub struct MockMempoolMonitor {
    config: Arc<Config>,
    mock_config: MockConfig,
    is_running: Arc<AtomicBool>,
}

impl MockMempoolMonitor {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let mock_config = get_mock_config();
        
        info!("🎭 MockMempoolMonitor initialized");
        debug!("Mock MEV opportunity rate: {}", mock_config.mev_opportunity_rate);
        debug!("Mock transactions per block: {}", mock_config.tx_per_block);
        
        Ok(Self {
            config,
            mock_config,
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn get_pending_transactions(&self) -> Result<Vec<Transaction>> {
        info!("🎭 [MOCK] Getting pending transactions from mempool");
        
        // Mock network latency
        tokio::time::sleep(tokio::time::Duration::from_millis(self.mock_config.network_latency)).await;
        
        let mut transactions = Vec::new();
        let mut rng = rand::thread_rng();
        
        // Generate mock pending transactions
        let tx_count = rng.gen_range(20..100);
        for i in 0..tx_count {
            let tx = self.generate_mock_pending_transaction(i).await;
            transactions.push(tx);
        }
        
        debug!("🎭 [MOCK] Generated {} pending transactions", transactions.len());
        Ok(transactions)
    }

    pub async fn start_monitoring(&self, tx_sender: mpsc::UnboundedSender<Transaction>) -> Result<()> {
        info!("🎭 [MOCK] 🚀 멤풀 모니터링 시작...");
        self.is_running.store(true, Ordering::SeqCst);
        
        let mock_config = self.mock_config.clone();
        let _config = Arc::clone(&self.config);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(200));
            
            loop {
                interval.tick().await;
                
                // Generate new mock transactions periodically
                let new_tx_count = {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(1..10)
                };
                
                for i in 0..new_tx_count {
                    let tx = Self::generate_mock_transaction_static(i, &mock_config).await;
                    
                    if let Err(e) = tx_sender.send(tx) {
                        warn!("🎭 [MOCK] ❌ 트랜잭션 전송 실패: {}", e);
                        break;
                    }
                }
                
                // Simulate occasional MEV opportunities
                let should_generate_mev = {
                    let mut rng = rand::thread_rng();
                    rng.gen::<f64>() < mock_config.mev_opportunity_rate
                };
                
                if should_generate_mev {
                    let mev_tx = Self::generate_mev_opportunity(&mock_config).await;
                    if let Err(e) = tx_sender.send(mev_tx) {
                        warn!("🎭 [MOCK] ❌ MEV 기회 전송 실패: {}", e);
                        break;
                    } else {
                        info!("🎭 [MOCK] 💰 MEV opportunity detected and sent!");
                    }
                }
            }
        });
        
        Ok(())
    }

    async fn generate_mock_pending_transaction(&self, index: u64) -> Transaction {
        Self::generate_mock_transaction_static(index, &self.mock_config).await
    }
    
    async fn generate_mock_transaction_static(_index: u64, mock_config: &MockConfig) -> Transaction {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let timestamp = chrono::Utc::now();
        
        // Generate different types of transactions
        let tx_type = rng.gen_range(0..4);
        let (to, value, data, gas_limit) = match tx_type {
            0 => {
                // Simple ETH transfer
                (Some(Address::from_slice(&rand::random::<[u8; 20]>())), rng.gen_range(100_000_000_000_000_000u64..10_000_000_000_000_000_000u64), vec![], 21_000)
            },
            1 => {
                // DEX swap (Uniswap V2)
                let router: Address = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap();
                let swap_data = generate_mock_swap_data();
                (Some(router), rng.gen_range(100_000_000_000_000_000u64..1_000_000_000_000_000_000u64), swap_data, rng.gen_range(150_000..300_000))
            },
            2 => {
                // Contract interaction
                (Some(Address::from_slice(&rand::random::<[u8; 20]>())), 0, generate_mock_contract_data(), rng.gen_range(50_000..500_000))
            },
            _ => {
                // Large value transaction (potential MEV target)
                let router: Address = "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap();
                let large_swap_data = generate_mock_large_swap_data();
                (Some(router), rng.gen_range(5_000_000_000_000_000_000u64..10_000_000_000_000_000_000u64), large_swap_data, rng.gen_range(200_000..800_000))
            }
        };
        
        Transaction {
            hash: H256::from_slice(&rand::random::<[u8; 32]>()),
            from: Address::from_slice(&rand::random::<[u8; 20]>()),
            to,
            value: U256::from(value),
            gas_price: U256::from(rng.gen_range(
                mock_config.gas_price / 2..mock_config.gas_price * 3
            )),
            gas_limit: U256::from(gas_limit),
            data,
            nonce: rng.gen_range(0..1000),
            timestamp,
            block_number: None, // Pending transaction
        }
    }
    
    async fn generate_mev_opportunity(mock_config: &MockConfig) -> Transaction {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let timestamp = chrono::Utc::now();
        
        // Generate high-value transaction that could be sandwiched
        let uniswap_router: Address = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap();
        let large_swap_value = rng.gen_range(1_000_000_000_000_000_000u64..10_000_000_000_000_000_000u64); // 1-10 ETH
        
        Transaction {
            hash: H256::from_slice(&rand::random::<[u8; 32]>()),
            from: Address::from_slice(&rand::random::<[u8; 20]>()),
            to: Some(uniswap_router),
            value: U256::from(large_swap_value),
            gas_price: U256::from(rng.gen_range(
                mock_config.gas_price..mock_config.gas_price * 2
            )),
            gas_limit: U256::from(rng.gen_range(300_000..500_000)),
            data: generate_mock_mev_target_data(),
            nonce: rng.gen_range(0..1000),
            timestamp,
            block_number: None,
        }
    }

    pub async fn get_transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>> {
        info!("🎭 [MOCK] Getting transaction by hash: {:?}", hash);
        
        // Mock network latency
        tokio::time::sleep(tokio::time::Duration::from_millis(self.mock_config.network_latency / 2)).await;
        
        // Mock finding transaction (50% chance)
        let mut rng = rand::thread_rng();
        if rng.gen::<f64>() < 0.5 {
            let tx = Self::generate_mock_transaction_static(0, &self.mock_config).await;
            Ok(Some(Transaction {
                hash, // Use the requested hash
                ..tx
            }))
        } else {
            Ok(None)
        }
    }

    pub fn stop(&self) {
        info!("🎭 [MOCK] Stopping mempool monitoring");
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

fn generate_mock_swap_data() -> Vec<u8> {
    // Mock Uniswap V2 swapExactTokensForTokens function
    let mut data = vec![0x38, 0xed, 0x17, 0x39]; // Function selector
    // Add mock parameters (amounts, addresses, etc.)
    data.extend_from_slice(&[0u8; 160]); // 5 * 32 bytes for parameters
    data
}

fn generate_mock_large_swap_data() -> Vec<u8> {
    // Mock large swap that could be sandwiched
    let mut data = vec![0x38, 0xed, 0x17, 0x39]; // swapExactTokensForTokens
    data.extend_from_slice(&[0u8; 160]);
    data
}

fn generate_mock_mev_target_data() -> Vec<u8> {
    // Mock transaction data that represents a juicy MEV target
    let mut data = vec![0x38, 0xed, 0x17, 0x39]; // swapExactTokensForTokens
    data.extend_from_slice(&[0u8; 160]);
    // Add some distinctive bytes to indicate this is a MEV opportunity
    data.extend_from_slice(&[0xee, 0x00, 0xff, 0x01]); // MEV opportunity marker
    data
}

fn generate_mock_contract_data() -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let function_selectors = vec![
        vec![0xa9, 0x05, 0x9c, 0xbb], // transfer
        vec![0x23, 0xb8, 0x72, 0xdd], // transferFrom  
        vec![0x09, 0x5e, 0xa7, 0xb3], // approve
        vec![0x42, 0x84, 0x2e, 0x0e], // safeTransferFrom
    ];
    
    let mut data = function_selectors[rng.gen_range(0..function_selectors.len())].clone();
    // Add a fixed size instead of runtime size
    let extra_size = rng.gen_range(32..128);
    data.extend(vec![0u8; extra_size]);
    data
}