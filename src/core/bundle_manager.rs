use std::sync::Arc;
use anyhow::Result;
use tokio::sync::Mutex;
use tracing::{info, debug, error, warn};
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use std::time::{Instant, Duration};

use crate::config::Config;
use crate::types::{Bundle, Opportunity, Priority, StrategyType, U256};
use crate::flashbots::FlashbotsClient;

pub struct BundleManager {
    config: Arc<Config>,
    flashbots_client: Arc<FlashbotsClient>,
    pending_bundles: Arc<Mutex<HashMap<String, Bundle>>>,
    submitted_bundles: Arc<Mutex<HashMap<String, Bundle>>>,
    bundle_stats: Arc<Mutex<BundleStats>>,
}

#[derive(Debug, Clone)]
pub struct BundleStats {
    pub total_created: u64,
    pub total_submitted: u64,
    pub total_included: u64,
    pub total_failed: u64,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub avg_submission_time_ms: f64,
    pub success_rate: f64,
}

impl BundleManager {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let flashbots_client = FlashbotsClient::new(Arc::clone(&config)).await?;
        
        let bundle_stats = BundleStats {
            total_created: 0,
            total_submitted: 0,
            total_included: 0,
            total_failed: 0,
            total_profit: U256::zero(),
            total_gas_spent: U256::zero(),
            avg_submission_time_ms: 0.0,
            success_rate: 0.0,
        };
        
        Ok(Self {
            config,
            flashbots_client: Arc::new(flashbots_client),
            pending_bundles: Arc::new(Mutex::new(HashMap::new())),
            submitted_bundles: Arc::new(Mutex::new(HashMap::new())),
            bundle_stats: Arc::new(Mutex::new(bundle_stats)),
        })
    }

    /// 기회들을 분석하여 최적의 번들 생성
    pub async fn create_optimal_bundle(&self, opportunities: Vec<Opportunity>) -> Result<Option<Bundle>> {
        if opportunities.is_empty() {
            return Ok(None);
        }

        info!("🎯 {}개 기회로 최적 번들 생성 중...", opportunities.len());
        
        // 기회들을 수익성 순으로 정렬
        let mut sorted_opportunities: BinaryHeap<Opportunity> = opportunities.into_iter().collect();
        
        // 번들 크기 제한 확인
        let max_bundle_size = self.config.safety.max_concurrent_bundles;
        let mut selected_opportunities = Vec::new();
        
        while selected_opportunities.len() < max_bundle_size && !sorted_opportunities.is_empty() {
            if let Some(opportunity) = sorted_opportunities.pop() {
                // 기회 검증
                if self.validate_opportunity_for_bundle(&opportunity).await? {
                    selected_opportunities.push(opportunity);
                }
            }
        }
        
        if selected_opportunities.is_empty() {
            return Ok(None);
        }
        
        // 번들 생성
        let bundle = self.create_bundle_from_opportunities(selected_opportunities).await?;
        
        // 번들 통계 업데이트
        self.update_bundle_stats(&bundle, "created").await;
        
        info!("📦 최적 번들 생성됨: {} (기회: {}개, 예상 수익: {} ETH)", 
              bundle.id, bundle.transactions.len(), 
              ethers::utils::format_ether(bundle.expected_profit));
        
        Ok(Some(bundle))
    }

    /// 기회가 번들에 포함될 수 있는지 검증
    async fn validate_opportunity_for_bundle(&self, opportunity: &Opportunity) -> Result<bool> {
        // 최소 수익 임계값 확인
        let min_profit = ethers::utils::parse_ether(&self.config.strategies.arbitrage.min_profit_threshold)?;
        if opportunity.expected_profit < min_profit {
            return Ok(false);
        }
        
        // 가스비 대비 수익성 확인
        let gas_cost = U256::from(opportunity.gas_estimate) * U256::from(20_000_000_000u64); // 20 gwei
        if opportunity.expected_profit <= gas_cost {
            return Ok(false);
        }
        
        // 만료 시간 확인
        if opportunity.is_expired(0) { // 현재 블록 번호는 실제로 가져와야 함
            return Ok(false);
        }
        
        Ok(true)
    }

    /// 기회들로부터 번들 생성
    async fn create_bundle_from_opportunities(&self, opportunities: Vec<Opportunity>) -> Result<Bundle> {
        let mut all_transactions = Vec::new();
        let mut total_profit = U256::zero();
        let mut total_gas = 0u64;
        let mut target_block = 0u64;
        
        for opportunity in &opportunities {
            // 각 기회에 대한 트랜잭션 생성 (실제로는 전략에서 생성)
            // 여기서는 더미 트랜잭션 생성
            let dummy_tx = self.create_dummy_transaction_for_opportunity(opportunity).await?;
            all_transactions.push(dummy_tx);
            
            total_profit += opportunity.expected_profit;
            total_gas += opportunity.gas_estimate;
            
            // 가장 높은 우선순위의 전략 선택
            if opportunity.priority.to_u8() > Priority::High.to_u8() {
                target_block = opportunity.expiry_block;
            }
        }
        
        // 번들 ID 생성
        let bundle_id = format!("bundle_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        
        let bundle = Bundle::new(
            all_transactions,
            target_block,
            total_profit,
            total_gas,
            StrategyType::Arbitrage, // 기본값, 실제로는 혼합 전략
        );
        
        Ok(bundle)
    }

    /// 기회를 위한 더미 트랜잭션 생성 (실제 구현에서는 전략에서 생성)
    async fn create_dummy_transaction_for_opportunity(&self, _opportunity: &Opportunity) -> Result<crate::types::Transaction> {
        // 실제 구현에서는 전략별로 적절한 트랜잭션 생성
        Ok(crate::types::Transaction {
            hash: ethers::types::H256::zero(),
            from: ethers::types::H160::zero(),
            to: Some(ethers::types::H160::zero()),
            value: U256::zero(),
            gas_price: U256::from(20_000_000_000u64),
            gas_limit: U256::from(200_000u64),
            data: vec![],
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: None,
        })
    }

    /// 번들 제출
    pub async fn submit_bundle(&self, bundle: Bundle) -> Result<bool> {
        let submission_start = Instant::now();
        
        info!("📤 번들 제출 중: {}", bundle.id);
        
        // 시뮬레이션 모드 확인
        if self.config.flashbots.simulation_mode {
            info!("🧪 시뮬레이션 모드: 번들 제출 건너뜀");
            self.update_bundle_stats(&bundle, "simulated").await;
            return Ok(true);
        }
        
        // Flashbots에 번들 제출
        match self.flashbots_client.submit_bundle(&bundle).await {
            Ok(success) => {
                let submission_duration = submission_start.elapsed();
                
                if success {
                    info!("✅ 번들 제출 성공: {} (제출 시간: {:.2}ms)", 
                          bundle.id, submission_duration.as_millis());
                    
                    // 제출된 번들 저장
                    let mut submitted = self.submitted_bundles.lock().await;
                    submitted.insert(bundle.id.clone(), bundle.clone());
                    
                    self.update_bundle_stats(&bundle, "submitted").await;
                    Ok(true)
                } else {
                    error!("❌ 번들 제출 실패: {}", bundle.id);
                    self.update_bundle_stats(&bundle, "failed").await;
                    Ok(false)
                }
            }
            Err(e) => {
                error!("❌ 번들 제출 오류: {}", e);
                self.update_bundle_stats(&bundle, "failed").await;
                Err(e)
            }
        }
    }

    /// 번들 상태 업데이트
    pub async fn update_bundle_status(&self, bundle_id: &str, status: &str) -> Result<()> {
        let mut submitted = self.submitted_bundles.lock().await;
        
        if let Some(bundle) = submitted.get_mut(bundle_id) {
            match status {
                "included" => {
                    info!("🎉 번들 포함됨: {}", bundle_id);
                    self.update_bundle_stats(bundle, "included").await;
                }
                "failed" => {
                    warn!("💥 번들 실패: {}", bundle_id);
                    self.update_bundle_stats(bundle, "failed").await;
                }
                _ => {
                    debug!("📊 번들 상태 업데이트: {} -> {}", bundle_id, status);
                }
            }
        }
        
        Ok(())
    }

    /// 번들 통계 업데이트
    async fn update_bundle_stats(&self, bundle: &Bundle, action: &str) {
        let mut stats = self.bundle_stats.lock().await;
        
        match action {
            "created" => {
                stats.total_created += 1;
            }
            "submitted" => {
                stats.total_submitted += 1;
            }
            "included" => {
                stats.total_included += 1;
                stats.total_profit += bundle.expected_profit;
            }
            "failed" => {
                stats.total_failed += 1;
            }
            "simulated" => {
                stats.total_created += 1;
                stats.total_submitted += 1;
            }
            _ => {}
        }
        
        // 성공률 계산
        if stats.total_submitted > 0 {
            stats.success_rate = stats.total_included as f64 / stats.total_submitted as f64;
        }
    }

    /// 번들 통계 조회
    pub async fn get_bundle_stats(&self) -> BundleStats {
        self.bundle_stats.lock().await.clone()
    }

    /// 대기 중인 번들 조회
    pub async fn get_pending_bundles(&self) -> Vec<Bundle> {
        let pending = self.pending_bundles.lock().await;
        pending.values().cloned().collect()
    }

    /// 제출된 번들 조회
    pub async fn get_submitted_bundles(&self) -> Vec<Bundle> {
        let submitted = self.submitted_bundles.lock().await;
        submitted.values().cloned().collect()
    }

    /// 번들 정리 (만료된 번들 제거)
    pub async fn cleanup_expired_bundles(&self) -> Result<()> {
        let mut pending = self.pending_bundles.lock().await;
        let mut submitted = self.submitted_bundles.lock().await;
        
        let current_time = chrono::Utc::now();
        let mut expired_count = 0;
        
        // 대기 중인 번들에서 만료된 것들 제거
        pending.retain(|_, bundle| {
            if bundle.is_expired() {
                expired_count += 1;
                false
            } else {
                true
            }
        });
        
        // 제출된 번들에서 오래된 것들 제거 (24시간 이상)
        let cutoff_time = current_time - chrono::Duration::hours(24);
        submitted.retain(|_, bundle| {
            if bundle.timestamp < cutoff_time {
                expired_count += 1;
                false
            } else {
                true
            }
        });
        
        if expired_count > 0 {
            info!("🧹 {}개 만료된 번들 정리됨", expired_count);
        }
        
        Ok(())
    }

    /// 번들 우선순위 계산
    pub fn calculate_bundle_priority(&self, opportunities: &[Opportunity]) -> Priority {
        if opportunities.is_empty() {
            return Priority::Low;
        }
        
        // 가장 높은 우선순위의 기회 반환
        opportunities.iter()
            .map(|opp| opp.priority)
            .max_by(|a, b| a.to_u8().cmp(&b.to_u8()))
            .unwrap_or(Priority::Low)
    }
}

impl std::fmt::Debug for BundleManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BundleManager")
            .field("config", &"Arc<Config>")
            .field("flashbots_client", &"Arc<FlashbotsClient>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Opportunity, OpportunityType, StrategyType, Priority, U256};
    use chrono::Utc;

    #[tokio::test]
    async fn test_bundle_manager_creation() {
        let config = Arc::new(Config::default());
        let manager = BundleManager::new(config).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_bundle_priority_calculation() {
        let config = Arc::new(Config::default());
        let manager = BundleManager::new(config).await.unwrap();
        
        // 테스트 기회들 생성
        let opportunities = vec![
            Opportunity::new(
                OpportunityType::Arbitrage,
                StrategyType::Arbitrage,
                U256::from(1000000000000000000u128), // 1 ETH
                0.8,
                150_000,
                1000,
                crate::types::OpportunityDetails::Arbitrage(crate::types::ArbitrageDetails {
                    token_in: ethers::types::H160::zero(),
                    token_out: ethers::types::H160::zero(),
                    amount_in: U256::zero(),
                    amount_out: U256::zero(),
                    dex_path: vec![],
                    price_impact: 0.0,
                }),
            ),
        ];
        
        let priority = manager.calculate_bundle_priority(&opportunities);
        assert_eq!(priority, Priority::Low); // 기본 우선순위
    }
} 