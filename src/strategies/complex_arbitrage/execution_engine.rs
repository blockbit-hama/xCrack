//! Multi-Asset Arbitrage 실행 엔진
//!
//! 탐지된 다중자산 아비트리지 기회를 실제로 실행합니다.

use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::types::{H256, U256};
use ethers::providers::{Provider, Ws};
use tracing::{info, debug, error};
use tokio::sync::Mutex;
use std::collections::HashMap;

use crate::config::Config;
use crate::adapters::AdapterSelector;
use super::types::*;
use super::flashloan_executor::AaveFlashLoanExecutor;

pub struct ExecutionEngine {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    adapter_selector: Arc<AdapterSelector>,
    flashloan_executor: Arc<AaveFlashLoanExecutor>,
    execution_history: Arc<Mutex<Vec<ExecutionRecord>>>,
}

#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub opportunity_id: String,
    pub tx_hash: H256,
    pub success: bool,
    pub actual_profit: U256,
    pub gas_used: u64,
    pub execution_time_ms: u64,
}

impl ExecutionEngine {
    pub fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        adapter_selector: Arc<AdapterSelector>,
        flashloan_executor: Arc<AaveFlashLoanExecutor>,
    ) -> Self {
        Self {
            config,
            provider,
            adapter_selector,
            flashloan_executor,
            execution_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 다중자산 아비트리지 실행
    pub async fn execute(&self, opportunity: &MultiAssetArbitrageOpportunity) -> Result<bool> {
        info!("🚀 다중자산 아비트리지 실행: {}", opportunity.id);

        // 유동성 검증
        if !self.flashloan_executor.validate_liquidity(opportunity).await? {
            error!("❌ 유동성 부족");
            return Ok(false);
        }

        // FlashLoan 실행
        match self.flashloan_executor.execute_multi_asset_flashloan(opportunity).await {
            Ok(tx_hash) => {
                info!("✅ FlashLoan 성공: {:?}", tx_hash);

                // 실행 기록 저장
                let record = ExecutionRecord {
                    opportunity_id: opportunity.id.clone(),
                    tx_hash,
                    success: true,
                    actual_profit: opportunity.expected_profit,
                    gas_used: opportunity.total_gas_estimate,
                    execution_time_ms: 0,
                };

                self.execution_history.lock().await.push(record);

                Ok(true)
            }
            Err(e) => {
                error!("❌ FlashLoan 실패: {}", e);
                Ok(false)
            }
        }
    }

    /// 실행 기록 조회
    pub async fn get_execution_history(&self) -> Vec<ExecutionRecord> {
        self.execution_history.lock().await.clone()
    }

    /// 성공률 조회
    pub async fn get_success_rate(&self) -> f64 {
        let history = self.execution_history.lock().await;
        if history.is_empty() {
            return 0.0;
        }

        let successful = history.iter().filter(|r| r.success).count();
        (successful as f64 / history.len() as f64) * 100.0
    }
}
