use std::sync::Arc;
use anyhow::Result;
use ethers::prelude::*;
use ethers::types::{Block, Transaction as EthersTransaction, BlockNumber, TxHash};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, debug};

use super::{get_mock_config, MockConfig};

#[derive(Clone)]
pub struct MockRpcProvider {
    mock_config: MockConfig,
    current_block: Arc<RwLock<u64>>,
    blocks: Arc<RwLock<HashMap<u64, Block<EthersTransaction>>>>,
    transactions: Arc<RwLock<HashMap<TxHash, EthersTransaction>>>,
}

impl MockRpcProvider {
    pub async fn new() -> Result<Self> {
        let mock_config = get_mock_config();
        
        info!("🎭 MockRpcProvider initialized");
        debug!("Mock chain ID: {}", mock_config.chain_id);
        debug!("Mock block time: {}s", mock_config.block_time);
        
        let provider = Self {
            mock_config,
            current_block: Arc::new(RwLock::new(18_000_000)), // Start from a realistic block number
            blocks: Arc::new(RwLock::new(HashMap::new())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Start block generation
        let provider_clone = provider.clone();
        provider_clone.start_block_generation().await;
        
        Ok(provider)
    }
    
    async fn start_block_generation(self) {
        let provider = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(provider.mock_config.block_time)
            );
            
            loop {
                interval.tick().await;
                if let Err(e) = provider.generate_new_block().await {
                    tracing::error!("🎭 [MOCK] Failed to generate new block: {}", e);
                }
            }
        });
    }
    
    async fn generate_new_block(&self) -> Result<()> {
        let mut current_block = self.current_block.write().await;
        *current_block += 1;
        let block_number = *current_block;
        
        let timestamp = chrono::Utc::now().timestamp() as u64;
        
        // Generate mock transactions for this block
        let mut transactions = Vec::new();
        let tx_count = rand::random::<usize>() % (self.mock_config.tx_per_block - 50 + 1) + 50;
        
        for i in 0..tx_count {
            let tx = self.generate_mock_transaction(block_number, i as u64, timestamp).await;
            transactions.push(tx.clone());
            
            // Store transaction
            self.transactions.write().await.insert(tx.hash, tx);
        }
        
        // Create mock block
        let block = Block {
            hash: Some(H256::random()),
            parent_hash: H256::random(),
            uncles_hash: H256::random(),
            author: Some(Address::random()),
            state_root: H256::random(),
            transactions_root: H256::random(),
            receipts_root: H256::random(),
            number: Some(U64::from(block_number)),
            gas_used: U256::from(rand::random::<u32>() % 7_000_000 + 8_000_000),
            gas_limit: U256::from(15_000_000u64),
            extra_data: Default::default(),
            logs_bloom: Some(Default::default()),
            timestamp: U256::from(timestamp),
            difficulty: U256::from(rand::random::<u32>() % 9_000_000 + 1_000_000),
            total_difficulty: Some(U256::from(rand::random::<u64>() % 10_000_000_000_000 + 10_000_000_000_000)),
            seal_fields: vec![],
            uncles: vec![],
            transactions,
            size: Some(U256::from(rand::random::<u32>() % 150_000 + 50_000)),
            mix_hash: Some(H256::random()),
            nonce: Some(H64::random()),
            base_fee_per_gas: Some(U256::from(self.mock_config.base_fee)),
            withdrawals_root: None,
            withdrawals: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            other: Default::default(),
        };
        
        // Store block
        self.blocks.write().await.insert(block_number, block.clone());
        
        debug!("🎭 [MOCK] Generated block {} with {} transactions", block_number, tx_count);
        
        // Keep only recent blocks to avoid memory issues
        let mut blocks = self.blocks.write().await;
        if blocks.len() > 100 {
            let min_block = blocks.keys().min().copied();
            if let Some(min_block) = min_block {
                blocks.remove(&min_block);
            }
        }
        
        Ok(())
    }
    
    async fn generate_mock_transaction(&self, block_number: u64, tx_index: u64, _timestamp: u64) -> EthersTransaction {
        // Generate different types of transactions
        let tx_type = rand::random::<u32>() % 4;
        let (to, value, data, gas_limit) = match tx_type {
            0 => {
                // Simple ETH transfer
                (Some(Address::random()), U256::from(rand::random::<u64>() % 9_900_000_000_000_000_000 + 100_000_000_000_000_000), vec![], 21_000)
            },
            1 => {
                // DEX swap (Uniswap V2 Router)
                let router = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap();
                let swap_data = generate_swap_data();
                (Some(router), U256::from(rand::random::<u64>() % 900_000_000_000_000_000 + 100_000_000_000_000_000), swap_data, rand::random::<u32>() % 150_000 + 150_000)
            },
            2 => {
                // Contract interaction
                (Some(Address::random()), U256::zero(), generate_random_data((rand::random::<u32>() % 196 + 4) as usize), rand::random::<u32>() % 450_000 + 50_000)
            },
            _ => {
                // MEV opportunity (large swap or arbitrage)
                let router = "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap();
                let arb_data = generate_arbitrage_data();
                (Some(router), U256::from(rand::random::<u128>() % 49_000_000_000_000_000_000u128 + 1_000_000_000_000_000_000u128), arb_data, rand::random::<u32>() % 600_000 + 200_000)
            }
        };
        
        EthersTransaction {
            hash: H256::random(),
            nonce: U256::from(rand::random::<u32>() % 1000),
            block_hash: Some(H256::random()),
            block_number: Some(U64::from(block_number)),
            transaction_index: Some(U64::from(tx_index)),
            from: Address::random(),
            to,
            value,
            gas_price: Some(U256::from(rand::random::<u64>() % self.mock_config.gas_price + self.mock_config.gas_price / 2)),
            gas: U256::from(gas_limit),
            input: data.into(),
            v: U64::from(rand::random::<u32>() % 2 + 27),
            r: U256::from(rand::random::<u64>()),
            s: U256::from(rand::random::<u64>()),
            transaction_type: Some(U64::from(2)), // EIP-1559
            access_list: Some(Default::default()),
            max_priority_fee_per_gas: Some(U256::from(rand::random::<u32>() % 4_000_000_000 + 1_000_000_000)),
            max_fee_per_gas: Some(U256::from(self.mock_config.gas_price)),
            chain_id: Some(U256::from(self.mock_config.chain_id)),
            other: Default::default(),
        }
    }
    
    pub async fn get_block_number(&self) -> Result<U64> {
        // Add small latency to simulate network call
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let block_number = *self.current_block.read().await;
        Ok(U64::from(block_number))
    }
    
    pub async fn get_block_with_txs(&self, block_number: BlockNumber) -> Result<Option<Block<EthersTransaction>>> {
        // Add small latency to simulate network call
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        
        let block_num = match block_number {
            BlockNumber::Number(n) => n.as_u64(),
            BlockNumber::Latest => *self.current_block.read().await,
            BlockNumber::Earliest => 1,
            BlockNumber::Pending => *self.current_block.read().await + 1,
            _ => return Ok(None),
        };
        
        let blocks = self.blocks.read().await;
        Ok(blocks.get(&block_num).cloned())
    }
    
    pub async fn get_transaction(&self, hash: TxHash) -> Result<Option<EthersTransaction>> {
        // Add small latency to simulate network call
        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        
        let transactions = self.transactions.read().await;
        Ok(transactions.get(&hash).cloned())
    }
}

fn generate_swap_data() -> Vec<u8> {
    // Mock Uniswap V2 swapExactTokensForTokens function signature + random data
    let mut data = vec![0x38, 0xed, 0x17, 0x39]; // Function selector
    data.extend_from_slice(&[0u8; 128]); // Mock parameters
    data
}

fn generate_arbitrage_data() -> Vec<u8> {
    // Mock arbitrage function call
    let mut data = vec![0x12, 0x34, 0x56, 0x78]; // Function selector
    data.extend_from_slice(&[0u8; 256]); // Mock parameters
    data
}

fn generate_random_data(size: usize) -> Vec<u8> {
    (0..size).map(|_| rand::random::<u8>()).collect()
}