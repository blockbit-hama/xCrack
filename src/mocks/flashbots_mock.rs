use std::sync::Arc;
use anyhow::Result;
use ethers::prelude::*;
use tracing::{info, debug, warn};

use crate::config::Config;
use crate::types::{Bundle, BundleStatus, SimulationResult};
use super::{get_mock_config, MockConfig};

pub struct MockFlashbotsClient {
    config: Arc<Config>,
    mock_config: MockConfig,
}

impl MockFlashbotsClient {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let mock_config = get_mock_config();
        
        info!("🎭 MockFlashbotsClient initialized with mock configuration");
        debug!("Mock bundle success rate: {}", mock_config.bundle_success_rate);
        debug!("Mock simulation success rate: {}", mock_config.simulation_success_rate);
        
        Ok(Self {
            config,
            mock_config,
        })
    }

    pub async fn submit_bundle(&self, bundle: &Bundle) -> Result<bool> {
        info!("🎭 [MOCK] 번들 제출 중: {}", bundle.id);
        
        // Mock network latency
        tokio::time::sleep(tokio::time::Duration::from_millis(self.mock_config.network_latency)).await;
        
        // Simulate bundle submission process
        match self.simulate_bundle(bundle).await {
            Ok(simulation) => {
                if !simulation.success {
                    warn!("🎭 [MOCK] 번들 시뮬레이션 실패: {:?}", simulation.error_message);
                    return Ok(false);
                }
                
                debug!("🎭 [MOCK] 번들 시뮬레이션 성공: 수익 {} ETH", 
                       ethers::utils::format_ether({
                           let ethers_profit = ethers::types::U256::from_big_endian(&simulation.net_profit.to_be_bytes::<32>());
                           ethers_profit
                       }));
            }
            Err(e) => {
                warn!("🎭 [MOCK] 번들 시뮬레이션 오류: {}", e);
                return Ok(false);
            }
        }
        
        // Mock bundle submission
        match self.send_bundle(bundle).await {
            Ok(_bundle_hash) => {
                info!("🎭 [MOCK] ✅ 번들 제출 성공: {}", bundle.id);
                Ok(true)
            }
            Err(e) => {
                warn!("🎭 [MOCK] ❌ 번들 제출 실패: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn send_bundle(&self, bundle: &Bundle) -> Result<H256> {
        info!("🎭 [MOCK] 📤 Submitting bundle {} to Flashbots", bundle.id);
        
        // Mock network latency
        tokio::time::sleep(tokio::time::Duration::from_millis(self.mock_config.network_latency)).await;
        
        // Simulate success/failure based on configured rate
        let success = rand::random::<f64>() < self.mock_config.bundle_success_rate;
        
        if success {
            let bundle_hash = H256::random();
            info!("🎭 [MOCK] ✅ Bundle {} submitted successfully (hash: {:?})", bundle.id, bundle_hash);
            Ok(bundle_hash)
        } else {
            Err(anyhow::anyhow!("Mock bundle submission failed"))
        }
    }

    pub async fn simulate_bundle(&self, bundle: &Bundle) -> Result<SimulationResult> {
        info!("🎭 [MOCK] 🔬 Simulating bundle {}", bundle.id);
        
        // Mock network latency
        tokio::time::sleep(tokio::time::Duration::from_millis(self.mock_config.network_latency / 2)).await;
        
        let success = rand::random::<f64>() < self.mock_config.simulation_success_rate;
        
        if success {
            // Generate realistic mock values
            let gas_cost_ethers = U256::from(bundle.gas_estimate) * U256::from(self.mock_config.gas_price);
            let gas_cost = {
                let mut bytes = [0u8; 32];
                gas_cost_ethers.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            };
            let net_profit = if bundle.expected_profit > gas_cost {
                bundle.expected_profit - gas_cost
            } else {
                alloy::primitives::U256::ZERO
            };
            
            Ok(SimulationResult {
                success: true,
                profit: bundle.expected_profit,
                gas_used: bundle.gas_estimate,
                gas_cost,
                net_profit,
                price_impact: rand::random::<f64>() * 0.049 + 0.001, // 0.1% to 5%
                error_message: None,
                traces: Some(vec![
                    format!("🎭 [MOCK] Bundle {} simulation trace", bundle.id),
                    "🎭 [MOCK] Transaction 1: DEX swap successful".to_string(),
                    "🎭 [MOCK] Transaction 2: Arbitrage execution successful".to_string(),
                ]),
            })
        } else {
            let error_messages = vec![
                "Mock simulation: insufficient liquidity",
                "Mock simulation: transaction would revert",
                "Mock simulation: gas estimation failed",
                "Mock simulation: slippage too high",
            ];
            let error_msg = error_messages[rand::random::<usize>() % error_messages.len()].to_string();
            
            Ok(SimulationResult {
                success: false,
                profit: alloy::primitives::U256::ZERO,
                gas_used: 0,
                gas_cost: alloy::primitives::U256::ZERO,
                net_profit: alloy::primitives::U256::ZERO,
                price_impact: 0.0,
                error_message: Some(error_msg),
                traces: Some(vec!["🎭 [MOCK] Simulation failed".to_string()]),
            })
        }
    }

    pub async fn get_bundle_status(&self, bundle_hash: &H256) -> Result<BundleStatus> {
        info!("🎭 [MOCK] 📊 Checking bundle status for hash: {:?}", bundle_hash);
        
        // Mock network latency
        tokio::time::sleep(tokio::time::Duration::from_millis(self.mock_config.network_latency / 4)).await;
        
        let status_rand = rand::random::<f64>();
        
        let status = if status_rand < 0.4 {
            BundleStatus::Pending
        } else if status_rand < 0.7 {
            BundleStatus::Included
        } else if status_rand < 0.9 {
            BundleStatus::Failed
        } else {
            BundleStatus::Failed
        };
        
        debug!("🎭 [MOCK] Bundle status: {:?}", status);
        Ok(status)
    }
}