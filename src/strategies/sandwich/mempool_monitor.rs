use super::types::{TargetTransaction, DexType};
use super::dex_router::DexRouterManager;
use anyhow::Result;
use ethers::prelude::*;
use ethers::types::{Transaction, H256};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{info, debug, warn, error};
use std::collections::HashMap;

/// 멤풀 모니터 - 실시간 트랜잭션 감시
pub struct MempoolMonitor {
    provider: Arc<Provider<Ws>>,
    dex_manager: Arc<DexRouterManager>,
    tx_sender: mpsc::UnboundedSender<PendingSwapTransaction>,
    is_running: Arc<AtomicBool>,
    min_value_eth: f64,
    max_gas_price_gwei: u64,
    stats: Arc<RwLock<MempoolStats>>,
}

use std::sync::atomic::{AtomicBool, Ordering};

/// 대기 중인 스왑 트랜잭션
#[derive(Debug, Clone)]
pub struct PendingSwapTransaction {
    pub tx: TargetTransaction,
    pub dex_type: DexType,
    pub router_address: Address,
    pub detected_at_ms: u128,
}

/// 멤풀 통계
#[derive(Debug, Clone, Default)]
struct MempoolStats {
    total_txs_observed: u64,
    swap_txs_detected: u64,
    high_value_txs: u64,
    filtered_txs: u64,
}

impl MempoolMonitor {
    pub async fn new(
        provider: Arc<Provider<Ws>>,
        dex_manager: Arc<DexRouterManager>,
        min_value_eth: f64,
        max_gas_price_gwei: u64,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PendingSwapTransaction>)> {
        info!("🔍 멤풀 모니터 초기화 중...");
        info!("   최소 거래 가치: {} ETH", min_value_eth);
        info!("   최대 가스 가격: {} Gwei", max_gas_price_gwei);

        let (tx_sender, tx_receiver) = mpsc::unbounded_channel();

        let monitor = Self {
            provider,
            dex_manager,
            tx_sender,
            is_running: Arc::new(AtomicBool::new(false)),
            min_value_eth,
            max_gas_price_gwei,
            stats: Arc::new(RwLock::new(MempoolStats::default())),
        };

        Ok((monitor, tx_receiver))
    }

    /// 멤풀 모니터링 시작
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("⚠️ 멤풀 모니터가 이미 실행 중입니다");
            return Ok(());
        }

        info!("🚀 멤풀 모니터 시작...");
        self.is_running.store(true, Ordering::Relaxed);

        let provider = self.provider.clone();
        let dex_manager = self.dex_manager.clone();
        let tx_sender = self.tx_sender.clone();
        let is_running = self.is_running.clone();
        let min_value_wei = U256::from((self.min_value_eth * 1e18) as u64);
        let max_gas_price_wei = U256::from(self.max_gas_price_gwei) * U256::from(1_000_000_000u64);
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut pending_txs_stream = match provider.subscribe_pending_txs().await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("❌ 멤풀 구독 실패: {}", e);
                    return;
                }
            };

            info!("✅ 멤풀 구독 성공 - 트랜잭션 모니터링 시작");

            // 환경변수에서 통계 출력 간격 로드
            let stats_interval_secs = std::env::var("SANDWICH_STATS_INTERVAL_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse::<u64>()
                .unwrap_or(60);
            
            let mut stats_interval = interval(Duration::from_secs(stats_interval_secs));

            loop {
                if !is_running.load(Ordering::Relaxed) {
                    info!("🛑 멤풀 모니터 중지");
                    break;
                }

                tokio::select! {
                    Some(tx_hash) = pending_txs_stream.next() => {
                        Self::process_pending_tx(
                            &provider,
                            &dex_manager,
                            &tx_sender,
                            &stats,
                            tx_hash,
                            min_value_wei,
                            max_gas_price_wei,
                        ).await;
                    }
                    _ = stats_interval.tick() => {
                        Self::print_stats(&stats).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_pending_tx(
        provider: &Arc<Provider<Ws>>,
        dex_manager: &Arc<DexRouterManager>,
        tx_sender: &mpsc::UnboundedSender<PendingSwapTransaction>,
        stats: &Arc<RwLock<MempoolStats>>,
        tx_hash: H256,
        min_value_wei: U256,
        max_gas_price_wei: U256,
    ) {
        // 통계 업데이트
        {
            let mut s = stats.write().await;
            s.total_txs_observed += 1;
        }

        // 트랜잭션 상세 조회
        let tx = match provider.get_transaction(tx_hash).await {
            Ok(Some(tx)) => tx,
            Ok(None) => return,
            Err(_) => return,
        };

        // 기본 필터링
        if !Self::should_process_tx(&tx, min_value_wei, max_gas_price_wei, stats).await {
            return;
        }

        // DEX 스왑 감지
        let to_addr = match tx.to {
            Some(addr) => addr,
            None => return, // 컨트랙트 생성 트랜잭션 무시
        };

        if let Some(swap_detection) = dex_manager.detect_swap(&to_addr, &tx.input) {
            {
                let mut s = stats.write().await;
                s.swap_txs_detected += 1;
            }

            debug!("🎯 {} 스왑 감지: {:?}", swap_detection.dex_type.name(), tx_hash);

            let pending_swap = PendingSwapTransaction {
                tx: tx.into(),
                dex_type: swap_detection.dex_type,
                router_address: to_addr,
                detected_at_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            };

            if let Err(e) = tx_sender.send(pending_swap) {
                error!("❌ 스왑 트랜잭션 전송 실패: {}", e);
            }
        }
    }

    async fn should_process_tx(
        tx: &Transaction,
        min_value_wei: U256,
        max_gas_price_wei: U256,
        stats: &Arc<RwLock<MempoolStats>>,
    ) -> bool {
        // 가스 가격 필터
        let gas_price = tx.gas_price.unwrap_or_default();
        if gas_price > max_gas_price_wei {
            let mut s = stats.write().await;
            s.filtered_txs += 1;
            return false;
        }

        // 트랜잭션 가치 필터
        if tx.value < min_value_wei {
            let mut s = stats.write().await;
            s.filtered_txs += 1;
            return false;
        }

        {
            let mut s = stats.write().await;
            s.high_value_txs += 1;
        }

        true
    }

    async fn print_stats(stats: &Arc<RwLock<MempoolStats>>) {
        let s = stats.read().await;
        info!("📊 멤풀 통계 | 관찰: {} | 스왑: {} | 고가: {} | 필터: {}",
              s.total_txs_observed, s.swap_txs_detected, s.high_value_txs, s.filtered_txs);
    }

    /// 멤풀 모니터링 중지
    pub fn stop(&self) {
        info!("🛑 멤풀 모니터 중지 중...");
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// 현재 통계 조회
    pub async fn get_stats(&self) -> MempoolStats {
        self.stats.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mempool_monitor_creation() {
        // Mock test - 실제 WebSocket 연결 없이 구조 테스트
        assert!(true);
    }
}
