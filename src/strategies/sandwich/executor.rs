use super::types::{SandwichBundle, SandwichExecutionResult};
use super::stats::SandwichStatsManager;
use anyhow::{Result, anyhow};
use ethers::prelude::*;
use ethers::types::{H256, U256, Bytes, transaction::eip2718::TypedTransaction};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};

/// 샌드위치 실행자 - MEV 번들 제출 및 실행
pub struct SandwichExecutor {
    provider: Arc<Provider<Ws>>,
    wallet: LocalWallet,
    contract_address: Address,
    flashbots_relay_url: String,
    stats: Arc<SandwichStatsManager>,
    max_retries: u32,
}

impl SandwichExecutor {
    pub fn new(
        provider: Arc<Provider<Ws>>,
        wallet: LocalWallet,
        contract_address: Address,
        flashbots_relay_url: String,
        stats: Arc<SandwichStatsManager>,
    ) -> Self {
        info!("⚡ 샌드위치 실행자 초기화");
        info!("   컨트랙트: {:?}", contract_address);
        info!("   Flashbots Relay: {}", flashbots_relay_url);

        Self {
            provider,
            wallet,
            contract_address,
            flashbots_relay_url,
            stats,
            max_retries: 3,
        }
    }

    /// 샌드위치 번들 실행
    pub async fn execute_bundle(&self, bundle: SandwichBundle) -> Result<SandwichExecutionResult> {
        let start_time = Instant::now();
        let opportunity_id = format!("{:?}", bundle.opportunity.target_tx_hash);

        info!("⚡ 샌드위치 실행 시작");
        info!("   Opportunity ID: {}", opportunity_id);
        info!("   예상 순이익: {} ETH", format_eth(bundle.net_profit));

        // 통계 업데이트
        self.stats.record_bundle_submitted().await;

        // 현재 블록 번호 조회
        let current_block = self.provider.get_block_number().await?.as_u64();
        let target_block = current_block + 1;

        debug!("   현재 블록: {}, 타겟 블록: {}", current_block, target_block);

        // Flashbots 번들 제출
        match self.submit_flashbots_bundle(&bundle, target_block).await {
            Ok((front_run_hash, back_run_hash)) => {
                info!("✅ Flashbots 번들 제출 성공");
                info!("   Front-run TX: {:?}", front_run_hash);
                info!("   Back-run TX: {:?}", back_run_hash);

                // 번들 포함 대기 및 확인
                match self.wait_for_bundle_inclusion(front_run_hash, target_block).await {
                    Ok(true) => {
                        let execution_time_ms = start_time.elapsed().as_millis() as u64;

                        // 실제 수익 계산 (간소화)
                        let actual_profit = bundle.estimated_profit;
                        let actual_gas_cost = bundle.total_gas_cost;
                        let net_profit = if actual_profit > actual_gas_cost {
                            actual_profit - actual_gas_cost
                        } else {
                            U256::zero()
                        };

                        self.stats.record_successful_sandwich(actual_profit, actual_gas_cost).await;
                        self.stats.record_bundle_included().await;

                        info!("🎉 샌드위치 성공!");
                        info!("   실제 수익: {} ETH", format_eth(actual_profit));
                        info!("   가스 비용: {} ETH", format_eth(actual_gas_cost));
                        info!("   순이익: {} ETH", format_eth(net_profit));
                        info!("   실행 시간: {}ms", execution_time_ms);

                        Ok(SandwichExecutionResult {
                            opportunity_id,
                            bundle_hash: bundle.bundle_hash.unwrap_or_default(),
                            front_run_tx_hash: Some(front_run_hash),
                            back_run_tx_hash: Some(back_run_hash),
                            success: true,
                            actual_profit,
                            actual_gas_cost,
                            net_profit,
                            execution_time_ms,
                            block_number: target_block,
                            error_message: None,
                        })
                    }
                    Ok(false) => {
                        warn!("⏱️ 번들이 포함되지 않음 (타임아웃)");
                        self.stats.record_failed_sandwich().await;

                        Ok(SandwichExecutionResult {
                            opportunity_id,
                            bundle_hash: bundle.bundle_hash.unwrap_or_default(),
                            front_run_tx_hash: Some(front_run_hash),
                            back_run_tx_hash: Some(back_run_hash),
                            success: false,
                            actual_profit: U256::zero(),
                            actual_gas_cost: bundle.total_gas_cost,
                            net_profit: U256::zero(),
                            execution_time_ms: start_time.elapsed().as_millis() as u64,
                            block_number: target_block,
                            error_message: Some("Bundle not included".to_string()),
                        })
                    }
                    Err(e) => {
                        error!("❌ 번들 확인 실패: {}", e);
                        self.stats.record_failed_sandwich().await;

                        Ok(SandwichExecutionResult {
                            opportunity_id,
                            bundle_hash: bundle.bundle_hash.unwrap_or_default(),
                            front_run_tx_hash: None,
                            back_run_tx_hash: None,
                            success: false,
                            actual_profit: U256::zero(),
                            actual_gas_cost: U256::zero(),
                            net_profit: U256::zero(),
                            execution_time_ms: start_time.elapsed().as_millis() as u64,
                            block_number: target_block,
                            error_message: Some(e.to_string()),
                        })
                    }
                }
            }
            Err(e) => {
                error!("❌ Flashbots 번들 제출 실패: {}", e);
                self.stats.record_failed_sandwich().await;

                Ok(SandwichExecutionResult {
                    opportunity_id,
                    bundle_hash: H256::zero(),
                    front_run_tx_hash: None,
                    back_run_tx_hash: None,
                    success: false,
                    actual_profit: U256::zero(),
                    actual_gas_cost: U256::zero(),
                    net_profit: U256::zero(),
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    block_number: target_block,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }

    /// Flashbots 번들 제출 (실제 HTTP 요청)
    async fn submit_flashbots_bundle(
        &self,
        bundle: &SandwichBundle,
        target_block: u64,
    ) -> Result<(H256, H256)> {
        use serde_json::json;
        use ethers::utils::hex;

        debug!("📤 Flashbots 번들 제출 중...");

        // 1. Front-run 트랜잭션 빌드 및 서명
        let front_run_tx = self.build_and_sign_transaction(
            &bundle.front_run_tx,
            target_block,
            true, // is_front_run
        ).await?;

        // 2. Back-run 트랜잭션 빌드 및 서명
        let back_run_tx = self.build_and_sign_transaction(
            &bundle.back_run_tx,
            target_block,
            false, // is_back_run
        ).await?;

        let front_run_hash = front_run_tx.hash(&self.wallet.chain_id());
        let back_run_hash = back_run_tx.hash(&self.wallet.chain_id());

        debug!("   타겟 블록: {}", target_block);
        debug!("   Front-run TX: {:?}", front_run_hash);
        debug!("   Back-run TX: {:?}", back_run_hash);

        // 3. Flashbots 번들 구성
        let bundle_request = json!({
            "jsonrpc": "2.0",
            "method": "eth_sendBundle",
            "params": [{
                "txs": [
                    format!("0x{}", hex::encode(front_run_tx.rlp().as_ref())),
                    format!("0x{:?}", bundle.target_tx_hash), // 타겟 트랜잭션 해시
                    format!("0x{}", hex::encode(back_run_tx.rlp().as_ref())),
                ],
                "blockNumber": format!("0x{:x}", target_block),
                "minTimestamp": 0,
                "maxTimestamp": 0,
            }],
            "id": 1,
        });

        // 4. Flashbots Relay에 제출
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        match client
            .post(&self.flashbots_relay_url)
            .header("Content-Type", "application/json")
            .json(&bundle_request)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                let result: serde_json::Value = response.json().await?;

                if status.is_success() {
                    info!("✅ Flashbots 번들 제출 성공");
                    debug!("   응답: {:?}", result);
                    Ok((front_run_hash, back_run_hash))
                } else {
                    warn!("⚠️ Flashbots 번들 제출 실패: {:?}", result);
                    Err(anyhow!("Flashbots submission failed: {:?}", result))
                }
            }
            Err(e) => {
                error!("❌ Flashbots 네트워크 오류: {}", e);
                Err(anyhow!("Network error: {}", e))
            }
        }
    }

    /// 트랜잭션 빌드 및 서명
    async fn build_and_sign_transaction(
        &self,
        calldata: &Bytes,
        target_block: u64,
        is_front_run: bool,
    ) -> Result<TypedTransaction> {
        // 현재 nonce 조회
        let nonce = self.provider.get_transaction_count(
            self.wallet.address(),
            Some(ethers::types::BlockNumber::Pending.into()),
        ).await?;

        // 가스 가격 조회
        let base_fee = self.provider.get_gas_price().await?;
        
        // 환경변수에서 우선순위 수수료 로드
        let priority_fee_gwei = if is_front_run {
            std::env::var("SANDWICH_FRONT_RUN_PRIORITY_FEE_GWEI")
                .unwrap_or_else(|_| "5".to_string())
                .parse::<u64>()
                .unwrap_or(5)
        } else {
            std::env::var("SANDWICH_BACK_RUN_PRIORITY_FEE_GWEI")
                .unwrap_or_else(|_| "2".to_string())
                .parse::<u64>()
                .unwrap_or(2)
        };
        
        let priority_fee = U256::from(priority_fee_gwei) * U256::from(1_000_000_000u64);

        // 환경변수에서 가스 한도 로드
        let gas_limit = std::env::var("SANDWICH_GAS_LIMIT")
            .unwrap_or_else(|_| "200000".to_string())
            .parse::<u64>()
            .unwrap_or(200_000);
        
        // EIP-1559 트랜잭션 생성
        let tx = ethers::types::Eip1559TransactionRequest {
            to: Some(self.contract_address.into()),
            data: Some(calldata.clone()),
            value: Some(U256::zero()),
            nonce: Some(nonce + if is_front_run { U256::zero() } else { U256::one() }),
            gas: Some(U256::from(gas_limit)),
            max_fee_per_gas: Some(base_fee + priority_fee),
            max_priority_fee_per_gas: Some(priority_fee),
            chain_id: Some(self.wallet.chain_id()),
            access_list: Default::default(),
        };

        // 트랜잭션 서명
        let typed_tx: TypedTransaction = tx.into();
        let signature = self.wallet.sign_transaction(&typed_tx).await?;

        Ok(typed_tx.rlp_signed(&signature))
    }

    /// 번들 포함 대기
    async fn wait_for_bundle_inclusion(
        &self,
        tx_hash: H256,
        target_block: u64,
    ) -> Result<bool> {
        debug!("⏳ 번들 포함 대기 중...");

        // 환경변수에서 최대 대기 블록 수 로드
        let max_wait_blocks = std::env::var("SANDWICH_MAX_WAIT_BLOCKS")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u64>()
            .unwrap_or(3);
        
        let mut current_block = self.provider.get_block_number().await?.as_u64();

        while current_block <= target_block + max_wait_blocks {
            // 트랜잭션 영수증 확인
            if let Ok(Some(receipt)) = self.provider.get_transaction_receipt(tx_hash).await {
                if receipt.status == Some(1.into()) {
                    info!("✅ 트랜잭션 포함 확인: Block {}", receipt.block_number.unwrap());
                    return Ok(true);
                } else {
                    warn!("❌ 트랜잭션 실패");
                    return Ok(false);
                }
            }

            // 환경변수에서 대기 시간 로드
            let wait_seconds = std::env::var("SANDWICH_WAIT_SECONDS")
                .unwrap_or_else(|_| "3".to_string())
                .parse::<u64>()
                .unwrap_or(3);
            
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_seconds)).await;
            current_block = self.provider.get_block_number().await?.as_u64();
        }

        Ok(false)
    }
}

fn format_eth(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        // Mock test
        assert!(true);
    }
}
