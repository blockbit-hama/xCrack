use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug, warn};
use alloy::primitives::U256;
use ethers::providers::{Provider, Ws};
use ethers::types::{H256, Address as EthersAddress};
use tokio::time::{sleep, Duration};
use chrono::Utc;

use crate::config::Config;
use crate::mev::{FlashbotsClient, BundleStatus};
use crate::strategies::liquidation::bundle_builder::LiquidationBundle;
use crate::blockchain::BlockchainClient;

/// 청산 실행 엔진 - MEV 번들 제출 및 실행 관리
pub struct LiquidationExecutionEngine {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    flashbots_client: FlashbotsClient,
    
    // 실행 통계
    execution_stats: Arc<tokio::sync::RwLock<ExecutionStats>>,
}

/// 제출 결과
#[derive(Debug, Clone)]
pub struct SubmissionResult {
    pub bundle_hash: String,
    pub status: BundleStatus,
    pub submission_time: chrono::DateTime<chrono::Utc>,
    pub inclusion_time: Option<chrono::DateTime<chrono::Utc>>,
    pub profit_realized: Option<U256>,
    pub gas_used: Option<u64>,
    pub error_message: Option<String>,
}

/// 실행 통계
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub total_submissions: u64,
    pub successful_inclusions: u64,
    pub failed_submissions: u64,
    pub total_profit: U256,
    pub avg_inclusion_time_ms: u64,
    pub success_rate: f64,
    pub total_gas_used: u64,
    pub avg_gas_price: U256,
}

impl LiquidationExecutionEngine {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        flashbots_client: FlashbotsClient,
    ) -> Result<Self> {
        info!("⚡ Initializing Liquidation Execution Engine...");
        
        let execution_stats = Arc::new(tokio::sync::RwLock::new(ExecutionStats::default()));
        
        Ok(Self {
            config,
            provider,
            flashbots_client,
            execution_stats,
        })
    }
    
    /// 청산 번들 실행
    pub async fn execute_liquidation_bundle(&self, bundle: LiquidationBundle) -> Result<SubmissionResult> {
        let start_time = std::time::Instant::now();
        let submission_time = chrono::Utc::now();
        
        info!("🚀 Executing liquidation bundle with estimated profit: {} ETH", 
              format_eth_amount(U256::from_limbs(bundle.estimated_profit.0)));
        
        // 1. 번들 시뮬레이션
        let simulation_result = self.simulate_bundle(&bundle).await?;
        if !simulation_result.success {
            return Ok(SubmissionResult {
                bundle_hash: "".to_string(),
                status: BundleStatus::Rejected(simulation_result.error_message.clone().unwrap_or("Simulation failed".to_string())),
                submission_time,
                inclusion_time: None,
                profit_realized: None,
                gas_used: None,
                error_message: simulation_result.error_message,
            });
        }
        
        // 2. MEV 번들 제출
        let bundle_hash = self.submit_to_flashbots(&bundle).await?;
        
        // 3. 제출 결과 모니터링
        let result = self.monitor_bundle_inclusion(bundle_hash, submission_time, &bundle).await?;
        
        // 4. 통계 업데이트
        self.update_execution_stats(&result, start_time.elapsed()).await;
        
        Ok(result)
    }
    
    /// 번들 시뮬레이션
    async fn simulate_bundle(&self, bundle: &LiquidationBundle) -> Result<SimulationResult> {
        info!("🔍 Simulating liquidation bundle...");

        // 1. 기본 검증
        if bundle.estimated_profit <= ethers::types::U256::from(0) {
            return Ok(SimulationResult {
                success: false,
                gas_used: 0,
                error_message: Some("No profit expected".to_string()),
            });
        }

        // 2. 가스 비용 검증
        let gas_cost = bundle.scenario.max_gas_price * U256::from(bundle.scenario.estimated_gas);
        if gas_cost > bundle.estimated_profit {
            return Ok(SimulationResult {
                success: false,
                gas_used: 0,
                error_message: Some("Gas cost exceeds profit".to_string()),
            });
        }

        // 3. 청산 가능 여부 확인 (health factor)
        if bundle.scenario.user.account_data.health_factor >= 1.0 {
            return Ok(SimulationResult {
                success: false,
                gas_used: 0,
                error_message: Some("Position is not liquidatable (HF >= 1.0)".to_string()),
            });
        }

        // 4. 프로토콜 상태 확인 (실제로는 on-chain 호출)
        // 여기서는 시뮬레이션으로 성공 확률 기반 판단
        let success = bundle.success_probability > 0.5;

        // 5. 예상 가스 사용량 계산
        let estimated_gas = if bundle.scenario.requires_flash_loan {
            800_000 // 플래시론 사용 시 더 많은 가스
        } else {
            500_000 // 직접 청산
        };

        debug!("Simulation result: success={}, gas={}, profit={:.6} ETH",
               success, estimated_gas,
               (bundle.estimated_profit.low_u128() as f64) / 1e18);

        Ok(SimulationResult {
            success,
            gas_used: estimated_gas,
            error_message: if success { None } else { Some("Simulation failed - low success probability".to_string()) },
        })
    }
    
    /// Flashbots에 번들 제출
    async fn submit_to_flashbots(&self, bundle: &LiquidationBundle) -> Result<String> {
        info!("📤 Submitting bundle to Flashbots...");

        // 1. Flashbots RPC 엔드포인트
        let flashbots_rpc = "https://relay.flashbots.net";

        // 2. 번들 구성
        let target_block = bundle.target_block_number;
        let bundle_transactions = vec![bundle.transactions.clone()];

        // 3. 번들 해시 생성
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(bundle.transactions.as_ref());
        hasher.update(target_block.to_be_bytes());
        let hash_result = hasher.finalize();
        let bundle_hash = format!("0x{}", hex::encode(hash_result));

        // 4. Flashbots 번들 제출 (실제로는 HTTP POST)
        // POST /relay/v1/bundle
        // {
        //   "jsonrpc": "2.0",
        //   "method": "eth_sendBundle",
        //   "params": [{
        //     "txs": [bundleTx],
        //     "blockNumber": targetBlock,
        //     "minTimestamp": 0,
        //     "maxTimestamp": 0
        //   }],
        //   "id": 1
        // }

        info!("📡 Bundle submitted to Flashbots: {}", bundle_hash);
        debug!("Target block: {}, Priority fee: {:.4} ETH",
               target_block,
               (bundle.priority_fee_eth.low_u128() as f64) / 1e18);

        Ok(bundle_hash)
    }
    
    /// 번들 포함 모니터링
    async fn monitor_bundle_inclusion(
        &self,
        bundle_hash: String,
        submission_time: chrono::DateTime<chrono::Utc>,
        bundle: &LiquidationBundle,
    ) -> Result<SubmissionResult> {
        info!("👀 Monitoring bundle inclusion: {}", bundle_hash);
        
        let mut attempts = 0;
        let max_attempts = 20; // 20블록 (약 4분) 대기

        while attempts < max_attempts {
            // 1. 현재 블록 번호 확인
            let current_block = self.get_current_block_number().await?;

            // 2. Flashbots API로 번들 상태 조회
            // GET /relay/v1/bundle?bundleHash={bundle_hash}
            let bundle_status = self.check_flashbots_bundle_status(&bundle_hash).await?;

            match bundle_status {
                BundleCheckStatus::Included(block_hash, tx_hash) => {
                    let inclusion_time = chrono::Utc::now();
                    let inclusion_duration = inclusion_time - submission_time;

                    info!("🎉 Bundle included in block {:?}! Duration: {:?}", block_hash, inclusion_duration);

                    return Ok(SubmissionResult {
                        bundle_hash,
                        status: BundleStatus::Included(block_hash),
                        submission_time,
                        inclusion_time: Some(inclusion_time),
                        profit_realized: Some(U256::from_limbs(bundle.estimated_profit.0)),
                        gas_used: Some(bundle.scenario.estimated_gas),
                        error_message: None,
                    });
                }
                BundleCheckStatus::Pending => {
                    debug!("Bundle still pending at block {}", current_block);
                }
                BundleCheckStatus::Failed(reason) => {
                    warn!("❌ Bundle rejected: {}", reason);
                    return Ok(SubmissionResult {
                        bundle_hash,
                        status: BundleStatus::Rejected(reason.clone()),
                        submission_time,
                        inclusion_time: None,
                        profit_realized: None,
                        gas_used: None,
                        error_message: Some(reason),
                    });
                }
            }

            attempts += 1;
            sleep(Duration::from_secs(12)).await; // 1블록 대기
        }
        
        warn!("⏰ Bundle not included within timeout period");
        
        Ok(SubmissionResult {
            bundle_hash,
            status: BundleStatus::Rejected("Timeout - bundle not included".to_string()),
            submission_time,
            inclusion_time: None,
            profit_realized: None,
            gas_used: None,
            error_message: Some("Timeout - bundle not included".to_string()),
        })
    }
    
    /// 실행 통계 업데이트
    async fn update_execution_stats(&self, result: &SubmissionResult, _execution_time: Duration) {
        let mut stats = self.execution_stats.write().await;
        
        stats.total_submissions += 1;
        
        match result.status {
            BundleStatus::Included(_) => {
                stats.successful_inclusions += 1;
                if let Some(profit) = result.profit_realized {
                    stats.total_profit += profit;
                }
                if let Some(gas_used) = result.gas_used {
                    stats.total_gas_used += gas_used;
                }
            },
            BundleStatus::Rejected(_) => {
                stats.failed_submissions += 1;
            },
            BundleStatus::Pending => {
                // 아직 처리 중
            },
            BundleStatus::Timeout | BundleStatus::Replaced => {
                stats.failed_submissions += 1;
            },
        }
        
        // 평균 계산
        stats.success_rate = stats.successful_inclusions as f64 / stats.total_submissions as f64;
        
        if let Some(inclusion_time) = result.inclusion_time {
            let inclusion_duration = inclusion_time - result.submission_time;
            stats.avg_inclusion_time_ms = inclusion_duration.num_milliseconds() as u64;
        }
        
        debug!("Updated execution stats: {} submissions, {:.2}% success rate",
               stats.total_submissions, stats.success_rate * 100.0);
    }

    /// 현재 블록 번호 조회
    async fn get_current_block_number(&self) -> Result<u64> {
        // 실제로는 provider.get_block_number() 호출
        // 현재는 임의 값 반환
        Ok(18000000)
    }

    /// Flashbots 번들 상태 확인
    async fn check_flashbots_bundle_status(&self, _bundle_hash: &str) -> Result<BundleCheckStatus> {
        // 실제로는 Flashbots API 호출
        // GET https://relay.flashbots.net/relay/v1/bundle?bundleHash={hash}

        // 현재는 랜덤 시뮬레이션
        let random: f64 = rand::random();

        if random < 0.3 {
            // 30% 확률로 포함됨
            Ok(BundleCheckStatus::Included(
                H256::zero(),
                H256::zero(),
            ))
        } else if random < 0.9 {
            // 60% 확률로 대기 중
            Ok(BundleCheckStatus::Pending)
        } else {
            // 10% 확률로 실패
            Ok(BundleCheckStatus::Failed("Competition won".to_string()))
        }
    }

    /// 실행 통계 조회
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        self.execution_stats.read().await.clone()
    }
    
    /// 다중 번들 제출
    pub async fn submit_multiple_bundles(&self, bundles: Vec<LiquidationBundle>) -> Result<Vec<SubmissionResult>> {
        info!("📦 Submitting {} liquidation bundles", bundles.len());
        
        let mut results = Vec::new();
        
        for bundle in bundles {
            let result = self.execute_liquidation_bundle(bundle).await?;
            results.push(result);
            
            // 제출 간격 조절
            sleep(Duration::from_millis(100)).await;
        }
        
        info!("✅ All {} bundles submitted", results.len());
        
        Ok(results)
    }

    /// 경쟁 분석 및 가스 가격 조정
    pub async fn analyze_competition_and_adjust_gas(&self, bundle: &mut LiquidationBundle) -> Result<()> {
        info!("🔍 실시간 경쟁 분석 시작");
        
        // 1. 멤풀에서 유사한 청산 트랜잭션 스캔
        let competitor_analysis = self.scan_competitor_transactions(bundle).await?;
        
        // 2. 경쟁자들의 가스 가격 분석
        let gas_analysis = self.analyze_competitor_gas_prices(&competitor_analysis).await?;
        
        // 3. 동적 가스 가격 조정
        let adjusted_gas_price = self.calculate_competitive_gas_price(
            &gas_analysis, 
            &competitor_analysis,
            bundle
        ).await?;
        
        // 4. Bundle 업데이트
        bundle.scenario.max_gas_price = adjusted_gas_price;
        bundle.competition_level = self.determine_competition_level(&competitor_analysis);
        
        info!("⚡ 경쟁 분석 완료: 경쟁자={}명, 가스조정={:.1}x, 레벨={:?}", 
               competitor_analysis.competitor_count,
               gas_analysis.multiplier,
               bundle.competition_level);
        
        Ok(())
    }
    
    /// 경쟁자 트랜잭션 스캔
    async fn scan_competitor_transactions(&self, bundle: &LiquidationBundle) -> Result<CompetitorAnalysis> {
        let mut competitors = Vec::new();
        let mut total_gas_used = 0u64;
        
        // 최근 5개 블록에서 청산 관련 트랜잭션 스캔
        let current_block = self.get_current_block_number().await?;
        
        for block_offset in 1..=5 {
            let block_number = current_block - block_offset;
            if let Some(block) = self.blockchain_client.get_block(block_number).await? {
                if let Some(transactions) = block.transactions {
                    for tx in transactions {
                        if self.is_liquidation_transaction(&tx).await? {
                            let competitor = CompetitorInfo {
                                address: tx.from,
                                gas_price: tx.gas_price.unwrap_or_default(),
                                gas_used: tx.gas.unwrap_or_default().as_u64(),
                                block_number,
                                timestamp: chrono::Utc::now(),
                            };
                            competitors.push(competitor);
                            total_gas_used += tx.gas.unwrap_or_default().as_u64();
                        }
                    }
                }
            }
        }
        
        // 경쟁자 통계 계산
        let avg_gas_price = if !competitors.is_empty() {
            let total_gas_price: u64 = competitors.iter()
                .map(|c| c.gas_price.as_u64())
                .sum();
            total_gas_price / competitors.len() as u64
        } else {
            20_000_000_000 // 20 gwei 기본값
        };
        
        Ok(CompetitorAnalysis {
            competitors,
            competitor_count: competitors.len(),
            avg_gas_price: ethers::types::U256::from(avg_gas_price),
            total_gas_used,
            analysis_timestamp: chrono::Utc::now(),
        })
    }
    
    /// 청산 트랜잭션 여부 확인
    async fn is_liquidation_transaction(&self, tx: &ethers::types::Transaction) -> Result<bool> {
        // 청산 관련 함수 시그니처들
        let liquidation_signatures = [
            "0x4e71d92d", // liquidationCall (Aave)
            "0xf5e3c462", // liquidateBorrow (Compound)
            "0x2a55205a", // liquidate (MakerDAO)
        ];
        
        if let Some(data) = &tx.input {
            let function_selector = &data.0[..4];
            return Ok(liquidation_signatures.contains(&function_selector));
        }
        
        Ok(false)
    }
    
    /// 경쟁자 가스 가격 분석
    async fn analyze_competitor_gas_prices(&self, analysis: &CompetitorAnalysis) -> Result<GasAnalysis> {
        if analysis.competitors.is_empty() {
            return Ok(GasAnalysis {
                multiplier: 1.0,
                is_high_gas: false,
                trend: GasTrend::Stable,
                recommended_priority_fee: 2_000_000_000, // 2 gwei
            });
        }
        
        // 가스 가격 분포 분석
        let mut gas_prices: Vec<u64> = analysis.competitors
            .iter()
            .map(|c| c.gas_price.as_u64())
            .collect();
        gas_prices.sort();
        
        let median_gas_price = gas_prices[gas_prices.len() / 2];
        let p75_gas_price = gas_prices[(gas_prices.len() * 3) / 4];
        let p90_gas_price = gas_prices[(gas_prices.len() * 9) / 10];
        
        // 가스 가격 트렌드 분석
        let trend = self.analyze_gas_trend(&analysis.competitors).await?;
        
        // 경쟁 수준에 따른 승수 계산
        let multiplier = if p90_gas_price > median_gas_price * 2 {
            2.0 // 매우 높은 경쟁
        } else if p75_gas_price > median_gas_price * 1.5 {
            1.5 // 높은 경쟁
        } else if p75_gas_price > median_gas_price * 1.2 {
            1.2 // 보통 경쟁
        } else {
            1.0 // 낮은 경쟁
        };
        
        // 권장 Priority Fee 계산 (P75 + 10%)
        let recommended_priority_fee = (p75_gas_price * 110) / 100;
        
        Ok(GasAnalysis {
            multiplier,
            is_high_gas: p75_gas_price > 50_000_000_000, // 50 gwei 이상
            trend,
            recommended_priority_fee,
        })
    }
    
    /// 가스 가격 트렌드 분석
    async fn analyze_gas_trend(&self, competitors: &[CompetitorInfo]) -> Result<GasTrend> {
        if competitors.len() < 3 {
            return Ok(GasTrend::Stable);
        }
        
        // 시간순으로 정렬
        let mut sorted_competitors = competitors.to_vec();
        sorted_competitors.sort_by_key(|c| c.timestamp);
        
        // 최근 3개와 이전 3개 비교
        let recent_count = (sorted_competitors.len() / 2).min(3);
        let older_count = sorted_competitors.len() - recent_count;
        
        if older_count == 0 {
            return Ok(GasTrend::Stable);
        }
        
        let recent_avg: u64 = sorted_competitors[older_count..]
            .iter()
            .map(|c| c.gas_price.as_u64())
            .sum::<u64>() / recent_count as u64;
            
        let older_avg: u64 = sorted_competitors[..older_count]
            .iter()
            .map(|c| c.gas_price.as_u64())
            .sum::<u64>() / older_count as u64;
        
        let change_percentage = if older_avg > 0 {
            ((recent_avg as f64 - older_avg as f64) / older_avg as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(if change_percentage > 20.0 {
            GasTrend::Rising
        } else if change_percentage < -20.0 {
            GasTrend::Falling
        } else {
            GasTrend::Stable
        })
    }
    
    /// 경쟁적 가스 가격 계산
    async fn calculate_competitive_gas_price(
        &self,
        gas_analysis: &GasAnalysis,
        competitor_analysis: &CompetitorAnalysis,
        bundle: &LiquidationBundle,
    ) -> Result<ethers::types::U256> {
        // 기본 가스 가격 조회
        let (base_fee, _) = self.blockchain_client.get_gas_price().await?;
        
        // 경쟁 분석 기반 Priority Fee 계산
        let base_priority_fee = gas_analysis.recommended_priority_fee;
        let competition_multiplier = gas_analysis.multiplier;
        
        // 트렌드에 따른 추가 조정
        let trend_multiplier = match gas_analysis.trend {
            GasTrend::Rising => 1.2,  // 상승 중이면 20% 추가
            GasTrend::Falling => 0.9, // 하락 중이면 10% 감소
            GasTrend::Stable => 1.0,  // 안정적이면 그대로
        };
        
        // 최종 Priority Fee 계산
        let final_priority_fee = (base_priority_fee as f64 * competition_multiplier * trend_multiplier) as u64;
        
        // 최소/최대 제한
        let min_priority_fee = 1_000_000_000; // 1 gwei
        let max_priority_fee = 100_000_000_000; // 100 gwei
        
        let clamped_priority_fee = final_priority_fee.max(min_priority_fee).min(max_priority_fee);
        
        let total_gas_price = base_fee + ethers::types::U256::from(clamped_priority_fee);
        
        debug!("💰 가스 가격 계산: base={} gwei, priority={} gwei, total={} gwei",
               base_fee.as_u128() / 1_000_000_000,
               clamped_priority_fee / 1_000_000_000,
               total_gas_price.as_u128() / 1_000_000_000);
        
        Ok(total_gas_price)
    }
    
    /// 경쟁 수준 결정
    fn determine_competition_level(&self, analysis: &CompetitorAnalysis) -> crate::strategies::liquidation::bundle_builder::CompetitionLevel {
        match analysis.competitor_count {
            0..=1 => crate::strategies::liquidation::bundle_builder::CompetitionLevel::Low,
            2..=4 => crate::strategies::liquidation::bundle_builder::CompetitionLevel::Medium,
            5..=9 => crate::strategies::liquidation::bundle_builder::CompetitionLevel::High,
            _ => crate::strategies::liquidation::bundle_builder::CompetitionLevel::Critical,
        }
    }
}

/// 시뮬레이션 결과
#[derive(Debug, Clone)]
struct SimulationResult {
    success: bool,
    gas_used: u64,
    error_message: Option<String>,
}

/// 경쟁자 분석 결과
#[derive(Debug, Clone)]
struct CompetitorAnalysis {
    competitors: Vec<CompetitorInfo>,
    competitor_count: usize,
    avg_gas_price: ethers::types::U256,
    total_gas_used: u64,
    analysis_timestamp: chrono::DateTime<chrono::Utc>,
}

/// 경쟁자 정보
#[derive(Debug, Clone)]
struct CompetitorInfo {
    address: EthersAddress,
    gas_price: ethers::types::U256,
    gas_used: u64,
    block_number: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// 가스 분석 결과
#[derive(Debug, Clone)]
struct GasAnalysis {
    multiplier: f64,
    is_high_gas: bool,
    trend: GasTrend,
    recommended_priority_fee: u64,
}

/// 가스 가격 트렌드
#[derive(Debug, Clone, PartialEq)]
enum GasTrend {
    Rising,
    Falling,
    Stable,
}

/// 번들 체크 상태
enum BundleCheckStatus {
    Included(H256, H256), // (block_hash, tx_hash)
    Pending,
    Failed(String),
}

/// ETH 금액 포맷팅 헬퍼
fn format_eth_amount(amount: U256) -> String {
    let eth_amount = amount.to::<u128>() as f64 / 1e18;
    format!("{:.6}", eth_amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_execution_engine_creation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_bundle_simulation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_multiple_bundle_submission() {
        // TODO: 테스트 구현
        assert!(true);
    }
}
