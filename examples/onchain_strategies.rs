/// Example: Using On-Chain Integrated Strategies
/// 
/// This example demonstrates how to use the new on-chain integrated strategies
/// that directly connect to the blockchain and use real contract data.

use std::sync::Arc;
use anyhow::Result;
use tokio::signal;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use xcrack::{
    config::Config,
    blockchain::{BlockchainClient, MultiChainRpcManager},
    strategies::{OnChainSandwichStrategy, OnChainLiquidationStrategy, Strategy},
    types::Transaction,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Starting On-Chain Strategies Example");

    // Load configuration
    let config = Arc::new(Config::load_from_file("config/default.toml").await?);

    // Initialize blockchain client
    let blockchain_client = Arc::new(
        BlockchainClient::new(
            &config.blockchain.primary_network.rpc_url,
            config.blockchain.primary_network.ws_url.as_deref(),
        ).await?
    );

    info!("✅ Blockchain client connected to {}", config.blockchain.primary_network.name);

    // Initialize on-chain strategies
    let onchain_sandwich = Arc::new(
        OnChainSandwichStrategy::new(
            Arc::clone(&config),
            Arc::clone(&blockchain_client)
        ).await?
    );

    let onchain_liquidation = Arc::new(
        OnChainLiquidationStrategy::new(
            Arc::clone(&config),
            Arc::clone(&blockchain_client)
        ).await?
    );

    // Start strategies
    onchain_sandwich.start().await?;
    onchain_liquidation.start().await?;

    info!("🎯 On-chain strategies started successfully!");

    // Example 1: Real-time mempool monitoring
    info!("📡 Starting mempool monitoring...");
    
    tokio::spawn({
        let blockchain_client = Arc::clone(&blockchain_client);
        let sandwich_strategy = Arc::clone(&onchain_sandwich);
        let liquidation_strategy = Arc::clone(&onchain_liquidation);
        
        async move {
            if let Err(e) = monitor_mempool(
                blockchain_client,
                sandwich_strategy,
                liquidation_strategy
            ).await {
                error!("Mempool monitoring error: {}", e);
            }
        }
    });

    // Example 2: Periodic liquidation scanning
    info!("🔍 Starting liquidation scanning...");
    
    tokio::spawn({
        let liquidation_strategy = Arc::clone(&onchain_liquidation);
        
        async move {
            if let Err(e) = periodic_liquidation_scan(liquidation_strategy).await {
                error!("Liquidation scanning error: {}", e);
            }
        }
    });

    // Example 3: On-chain event listening
    info!("👂 Starting event listening...");
    
    tokio::spawn({
        let blockchain_client = Arc::clone(&blockchain_client);
        
        async move {
            if let Err(e) = listen_to_events(blockchain_client).await {
                error!("Event listening error: {}", e);
            }
        }
    });

    // Wait for shutdown signal
    info!("✅ All systems running. Press Ctrl+C to shutdown...");
    signal::ctrl_c().await?;

    // Graceful shutdown
    info!("🛑 Shutting down...");
    onchain_sandwich.stop().await?;
    onchain_liquidation.stop().await?;

    info!("👋 Goodbye!");
    Ok(())
}

/// Monitor mempool for arbitrage opportunities
async fn monitor_mempool(
    blockchain_client: Arc<BlockchainClient>,
    sandwich_strategy: Arc<OnChainSandwichStrategy>,
    liquidation_strategy: Arc<OnChainLiquidationStrategy>,
) -> Result<()> {
    info!("🔍 Mempool monitoring started");
    
    loop {
        // Get pending transactions
        match blockchain_client.get_pending_transactions().await {
            Ok(transactions) => {
                for tx in transactions {
                    // Convert ethers Transaction to our Transaction type
                    let our_tx = convert_transaction(tx);
                    
                    // Analyze with sandwich strategy
                    if let Ok(sandwich_opportunities) = sandwich_strategy.analyze(&our_tx).await {
                        if !sandwich_opportunities.is_empty() {
                            info!("🥪 Found {} sandwich opportunities", sandwich_opportunities.len());
                            
                            for opportunity in sandwich_opportunities {
                                if sandwich_strategy.validate_opportunity(&opportunity).await.unwrap_or(false) {
                                    info!("✅ Valid sandwich opportunity: {} ETH profit", 
                                        format_eth_amount(opportunity.expected_profit));
                                    
                                    // Create and potentially execute bundle
                                    if let Ok(bundle) = sandwich_strategy.create_bundle(&opportunity).await {
                                        info!("📦 Bundle created for execution");
                                        // In real implementation, submit to Flashbots or similar
                                    }
                                }
                            }
                        }
                    }
                    
                    // Analyze with liquidation strategy
                    if let Ok(liquidation_opportunities) = liquidation_strategy.analyze(&our_tx).await {
                        if !liquidation_opportunities.is_empty() {
                            info!("💸 Found {} liquidation opportunities", liquidation_opportunities.len());
                            
                            for opportunity in liquidation_opportunities {
                                if liquidation_strategy.validate_opportunity(&opportunity).await.unwrap_or(false) {
                                    info!("✅ Valid liquidation opportunity: {} ETH profit", 
                                        format_eth_amount(opportunity.expected_profit));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get pending transactions: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
        
        // Small delay to avoid overwhelming the RPC
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

/// Periodic liquidation opportunity scanning
async fn periodic_liquidation_scan(
    liquidation_strategy: Arc<OnChainLiquidationStrategy>,
) -> Result<()> {
    info!("🔍 Periodic liquidation scanning started");
    
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        info!("🔍 Scanning for liquidation opportunities...");
        
        match liquidation_strategy.scan_liquidatable_positions().await {
            Ok(opportunities) => {
                if !opportunities.is_empty() {
                    info!("💸 Found {} liquidation opportunities", opportunities.len());
                    
                    for opportunity in opportunities.iter().take(3) { // Top 3
                        info!("  👤 User: {}", opportunity.target_user);
                        info!("  💰 Liquidation: {} ETH", format_eth_amount(opportunity.liquidation_amount));
                        info!("  📊 Profit: {} ETH", format_eth_amount(opportunity.net_profit));
                        info!("  🏥 Health Factor: {:.3}", opportunity.position.health_factor);
                        info!("  🎲 Success Probability: {:.1}%", opportunity.success_probability * 100.0);
                    }
                } else {
                    info!("ℹ️ No liquidation opportunities found");
                }
            }
            Err(e) => {
                error!("Failed to scan liquidation opportunities: {}", e);
            }
        }
    }
}

/// Listen to on-chain events
async fn listen_to_events(blockchain_client: Arc<BlockchainClient>) -> Result<()> {
    info!("👂 Event listening started");
    
    // Subscribe to new blocks
    if let Some(ws_provider) = blockchain_client.get_ws_provider() {
        blockchain_client.subscribe_blocks(|block| {
            info!("📦 New block: {} with {} transactions", 
                block.number.unwrap_or_default(), 
                block.transactions.len()
            );
        }).await?;
        
        // Subscribe to pending transactions
        blockchain_client.subscribe_pending_transactions(|tx_hash| {
            info!("📤 New pending transaction: {:?}", tx_hash);
        }).await?;
        
        info!("✅ Subscribed to blocks and pending transactions");
    } else {
        warn!("⚠️ No WebSocket connection available for event listening");
    }
    
    // Keep the event listener alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        info!("👂 Event listener still active...");
    }
}

/// Convert ethers Transaction to our Transaction type
fn convert_transaction(ethers_tx: ethers::types::Transaction) -> Transaction {
    Transaction {
        hash: alloy::primitives::B256::from_slice(ethers_tx.hash.as_bytes()),
        from: alloy::primitives::Address::from_slice(ethers_tx.from.as_bytes()),
        to: ethers_tx.to.map(|addr| alloy::primitives::Address::from_slice(addr.as_bytes())),
        value: alloy::primitives::U256::from_limbs_slice(&ethers_tx.value.0),
        gas_price: alloy::primitives::U256::from_limbs_slice(&ethers_tx.gas_price.unwrap_or_default().0),
        gas_limit: alloy::primitives::U256::from_limbs_slice(&ethers_tx.gas.0),
        data: ethers_tx.input.to_vec(),
        nonce: ethers_tx.nonce.as_u64(),
        timestamp: chrono::Utc::now(),
        block_number: ethers_tx.block_number.map(|n| n.as_u64()),
    }
}

/// Format ETH amount for display
fn format_eth_amount(wei: alloy::primitives::U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6}", eth)
}

/// Multi-chain example
async fn multi_chain_example() -> Result<()> {
    info!("🌐 Multi-chain RPC manager example");
    
    let mut rpc_manager = MultiChainRpcManager::new();
    
    // Add multiple chains
    rpc_manager.add_chain(
        1, // Ethereum
        "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY",
        Some("wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY")
    ).await?;
    
    rpc_manager.add_chain(
        137, // Polygon
        "https://polygon-mainnet.g.alchemy.com/v2/YOUR_API_KEY",
        Some("wss://polygon-mainnet.g.alchemy.com/v2/YOUR_API_KEY")
    ).await?;
    
    rpc_manager.add_chain(
        56, // BSC
        "https://bsc-dataseed.binance.org/",
        None
    ).await?;
    
    // Use different chains
    for &chain_id in &[1u64, 137, 56] {
        if let Some(client) = rpc_manager.get_client(chain_id) {
            match client.get_current_block().await {
                Ok(block_number) => {
                    info!("Chain {}: Current block {}", chain_id, block_number);
                }
                Err(e) => {
                    error!("Chain {}: Failed to get block - {}", chain_id, e);
                }
            }
        }
    }
    
    Ok(())
}

/// Price oracle integration example
async fn price_oracle_example(blockchain_client: Arc<BlockchainClient>) -> Result<()> {
    info!("💱 Price oracle integration example");
    
    // Chainlink ETH/USD price feed
    let eth_usd_feed = alloy::primitives::Address::from_slice(
        &hex::decode("5f4ec3df9cbd43714fe2740f5e3616155c5b8419")?
    );
    
    // Get ETH price from Chainlink
    // This would require the Chainlink ABI and proper contract interaction
    // For now, we'll just show the concept
    
    info!("📊 Current ETH/USD price: $2,800 (example)");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_blockchain_connection() {
        // Test with a public RPC endpoint
        let result = BlockchainClient::new(
            "https://ethereum-rpc.publicnode.com",
            None
        ).await;
        
        assert!(result.is_ok(), "Failed to connect to blockchain");
        
        let client = result.unwrap();
        
        // Test basic functionality
        assert!(client.is_connected().await, "Client should be connected");
        
        let block_number = client.get_current_block().await;
        assert!(block_number.is_ok(), "Should be able to get current block");
        
        println!("✅ Connected to Ethereum, current block: {}", block_number.unwrap());
    }
}