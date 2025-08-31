pub mod flashbots_mock;
pub mod rpc_mock;
pub mod mempool_mock;
pub mod provider_mock;
pub mod exchange_clients;
pub mod arbitrage_simulator;

pub use flashbots_mock::MockFlashbotsClient;
pub use mempool_mock::MockMempoolMonitor;
pub use provider_mock::create_mock_ws_provider;

use std::env;

/// Check if mock mode is enabled
pub fn is_mock_mode() -> bool {
    env::var("API_MODE").unwrap_or_default() == "mock"
}

/// Get mock configuration values
pub fn get_mock_config() -> MockConfig {
    MockConfig {
        chain_id: env::var("MOCK_CHAIN_ID")
            .unwrap_or_else(|_| "1337".to_string())
            .parse()
            .unwrap_or(1337),
        block_time: env::var("MOCK_BLOCK_TIME")
            .unwrap_or_else(|_| "12".to_string())
            .parse()
            .unwrap_or(12),
        gas_price: env::var("MOCK_GAS_PRICE")
            .unwrap_or_else(|_| "20000000000".to_string())
            .parse()
            .unwrap_or(20_000_000_000u64),
        base_fee: env::var("MOCK_BASE_FEE")
            .unwrap_or_else(|_| "15000000000".to_string())
            .parse()
            .unwrap_or(15_000_000_000u64),
        tx_per_block: env::var("MOCK_TX_PER_BLOCK")
            .unwrap_or_else(|_| "150".to_string())
            .parse()
            .unwrap_or(150),
        mev_opportunity_rate: env::var("MOCK_MEV_OPPORTUNITY_RATE")
            .unwrap_or_else(|_| "0.05".to_string())
            .parse()
            .unwrap_or(0.05),
        network_latency: env::var("MOCK_NETWORK_LATENCY")
            .unwrap_or_else(|_| "50".to_string())
            .parse()
            .unwrap_or(50),
        bundle_success_rate: env::var("MOCK_BUNDLE_SUCCESS_RATE")
            .unwrap_or_else(|_| "0.85".to_string())
            .parse()
            .unwrap_or(0.85),
        simulation_success_rate: env::var("MOCK_SIMULATION_SUCCESS_RATE")
            .unwrap_or_else(|_| "0.95".to_string())
            .parse()
            .unwrap_or(0.95),
        
        // 마이크로아비트래지 Mock 설정
        arbitrage_opportunity_rate: env::var("MOCK_ARBITRAGE_OPPORTUNITY_RATE")
            .unwrap_or_else(|_| "0.15".to_string())
            .parse()
            .unwrap_or(0.15),
        exchange_latency_ms: env::var("MOCK_EXCHANGE_LATENCY_MS")
            .unwrap_or_else(|_| "25".to_string())
            .parse()
            .unwrap_or(25),
        order_execution_success_rate: env::var("MOCK_ORDER_EXECUTION_SUCCESS_RATE")
            .unwrap_or_else(|_| "0.92".to_string())
            .parse()
            .unwrap_or(0.92),
        min_profit_usd: env::var("MOCK_MIN_PROFIT_USD")
            .unwrap_or_else(|_| "5.0".to_string())
            .parse()
            .unwrap_or(5.0),
        max_profit_usd: env::var("MOCK_MAX_PROFIT_USD")
            .unwrap_or_else(|_| "250.0".to_string())
            .parse()
            .unwrap_or(250.0),
    }
}

#[derive(Debug, Clone)]
pub struct MockConfig {
    pub chain_id: u64,
    pub block_time: u64,
    pub gas_price: u64,
    pub base_fee: u64,
    pub tx_per_block: usize,
    pub mev_opportunity_rate: f64,
    pub network_latency: u64,
    pub bundle_success_rate: f64,
    pub simulation_success_rate: f64,
    
    // 마이크로아비트래지 설정
    pub arbitrage_opportunity_rate: f64,
    pub exchange_latency_ms: u64,
    pub order_execution_success_rate: f64,
    pub min_profit_usd: f64,
    pub max_profit_usd: f64,
}