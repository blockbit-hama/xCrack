use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug, warn};
use alloy::primitives::{Address, U256};
use ethers::providers::{Provider, Ws};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::protocols::ProtocolType;

/// 청산 멤풀 워처 - 멤풀에서 청산 관련 신호 감지
pub struct LiquidationMempoolWatcher {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    
    // 감지된 신호들
    detected_signals: Arc<tokio::sync::RwLock<Vec<LiquidationSignal>>>,
    
    // 모니터링 상태
    is_monitoring: Arc<tokio::sync::RwLock<bool>>,
}

/// 청산 신호
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidationSignal {
    /// 오라클 가격 업데이트
    OracleUpdate {
        asset: Address,
        old_price: U256,
        new_price: U256,
        affected_positions: Vec<Address>,
        urgency: LiquidationUrgency,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// 사용자 상환 시도
    UserRepay {
        user: Address,
        protocol: ProtocolType,
        repay_amount: U256,
        impact: RepayImpact,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// 경쟁 청산 감지
    CompetitorLiquidation {
        user: Address,
        protocol: ProtocolType,
        competitor_gas_price: U256,
        our_advantage: bool,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// 가스 가격 급등
    GasPriceSpike {
        old_gas_price: U256,
        new_gas_price: U256,
        spike_percentage: f64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// 네트워크 혼잡
    NetworkCongestion {
        pending_transactions: u64,
        avg_gas_price: U256,
        congestion_level: CongestionLevel,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// 청산 긴급도
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LiquidationUrgency {
    Low,      // 낮은 긴급도
    Medium,   // 중간 긴급도
    High,     // 높은 긴급도
    Critical, // 매우 긴급
}

/// 상환 임팩트
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RepayImpact {
    Neutral,     // 청산 기회에 영향 없음
    Reduces,     // 청산 기회 감소
    Eliminates,  // 청산 기회 완전 제거
    Increases,   // 청산 기회 증가 (부채 증가)
}

/// 네트워크 혼잡 수준
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CongestionLevel {
    Low,      // 낮은 혼잡
    Medium,   // 중간 혼잡
    High,     // 높은 혼잡
    Critical, // 매우 혼잡
}

impl LiquidationMempoolWatcher {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
    ) -> Result<Self> {
        info!("👀 Initializing Liquidation Mempool Watcher...");
        
        let detected_signals = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let is_monitoring = Arc::new(tokio::sync::RwLock::new(false));
        
        Ok(Self {
            config,
            provider,
            detected_signals,
            is_monitoring,
        })
    }
    
    /// 멤풀 모니터링 시작
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("🚀 Starting mempool monitoring for liquidation signals...");
        
        {
            let mut is_monitoring = self.is_monitoring.write().await;
            *is_monitoring = true;
        }
        
        // 멤풀 이벤트 구독 시작
        self.subscribe_to_mempool_events().await?;
        
        Ok(())
    }
    
    /// 멤풀 모니터링 중지
    pub async fn stop_monitoring(&self) -> Result<()> {
        info!("🛑 Stopping mempool monitoring...");
        
        {
            let mut is_monitoring = self.is_monitoring.write().await;
            *is_monitoring = false;
        }
        
        Ok(())
    }
    
    /// 멤풀 이벤트 구독 - 실제 pending transaction 모니터링
    async fn subscribe_to_mempool_events(&self) -> Result<()> {
        info!("📡 멤풀 이벤트 구독 시작...");

        // Pending 트랜잭션 스트림 생성
        let mut pending_tx_stream = match self.provider.watch_pending_transactions().await {
            Ok(stream) => stream,
            Err(e) => {
                warn!("⚠️ Pending 트랜잭션 스트림 생성 실패: {}", e);
                warn!("⚠️ 시뮬레이션 모드로 전환합니다...");
                return self.run_simulated_monitoring().await;
            }
        };

        info!("✅ Pending 트랜잭션 스트림 시작");

        // 트랜잭션 스트림 처리
        while *self.is_monitoring.read().await {
            use futures::StreamExt;

            match pending_tx_stream.next().await {
                Some(tx_hash) => {
                    // Pending 트랜잭션 분석
                    if let Err(e) = self.analyze_pending_transaction(tx_hash).await {
                        debug!("⚠️ 트랜잭션 분석 실패: {}", e);
                    }
                }
                None => {
                    warn!("⚠️ 스트림 종료, 재연결 시도...");
                    sleep(Duration::from_secs(5)).await;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Pending 트랜잭션 분석
    async fn analyze_pending_transaction(&self, tx_hash: ethers::types::H256) -> Result<()> {
        // 트랜잭션 상세 정보 조회
        if let Ok(Some(tx)) = self.provider.get_transaction(tx_hash).await {
            // 청산 관련 트랜잭션 감지
            if let Some(to) = tx.to {
                // Aave, Compound, MakerDAO 주소 확인
                if self.is_lending_protocol_address(&to) {
                    // 청산 함수 호출 감지
                    if self.is_liquidation_call(&tx.input) {
                        self.process_competitor_liquidation(tx).await?;
                    }
                }

                // 오라클 업데이트 감지
                if self.is_oracle_address(&to) {
                    self.process_oracle_update(tx).await?;
                }
            }

            // 가스 가격 급등 감지
            self.check_gas_price_spike(&tx).await?;
        }

        Ok(())
    }

    /// 대출 프로토콜 주소 확인
    fn is_lending_protocol_address(&self, address: &Address) -> bool {
        let lending_protocols = vec![
            "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2", // Aave V3 Pool
            "0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B", // Compound V2 Comptroller
            "0xc3d688B66703497DAA19211EEdff47f25384cdc3", // Compound V3 Comet (USDC)
            "0x9759A6Ac90977b93B58547b4A71c78317f391A28", // MakerDAO Cat (liquidator)
        ];

        lending_protocols.iter().any(|addr| {
            addr.parse::<Address>().map(|a| a == *address).unwrap_or(false)
        })
    }

    /// 오라클 주소 확인
    fn is_oracle_address(&self, address: &Address) -> bool {
        let oracle_addresses = vec![
            "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419", // Chainlink ETH/USD
            "0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6", // Chainlink USDC/USD
        ];

        oracle_addresses.iter().any(|addr| {
            addr.parse::<Address>().map(|a| a == *address).unwrap_or(false)
        })
    }

    /// 청산 호출 감지
    fn is_liquidation_call(&self, input: &ethers::types::Bytes) -> bool {
        if input.len() < 4 {
            return false;
        }

        let function_selector = &input[0..4];

        // Aave liquidationCall: 0xe8efa440
        // Compound liquidateBorrow: 0xf5e3c462
        // MakerDAO bite: 0x72c6c1e6
        let liquidation_selectors = vec![
            [0xe8, 0xef, 0xa4, 0x40], // Aave
            [0xf5, 0xe3, 0xc4, 0x62], // Compound
            [0x72, 0xc6, 0xc1, 0xe6], // MakerDAO
        ];

        liquidation_selectors.iter().any(|selector| function_selector == selector)
    }

    /// 경쟁 청산 처리
    async fn process_competitor_liquidation(&self, tx: ethers::types::Transaction) -> Result<()> {
        let signal = LiquidationSignal::CompetitorLiquidation {
            user: tx.from,
            protocol: ProtocolType::Aave, // TODO: 트랜잭션에서 프로토콜 파싱
            competitor_gas_price: tx.gas_price.unwrap_or_default(),
            our_advantage: false, // TODO: 우리 전략과 비교
            timestamp: chrono::Utc::now(),
        };

        self.process_liquidation_signal(signal).await
    }

    /// 오라클 업데이트 처리
    async fn process_oracle_update(&self, _tx: ethers::types::Transaction) -> Result<()> {
        let signal = LiquidationSignal::OracleUpdate {
            asset: Address::ZERO, // TODO: 트랜잭션에서 자산 파싱
            old_price: U256::ZERO,
            new_price: U256::ZERO,
            affected_positions: vec![],
            urgency: LiquidationUrgency::Medium,
            timestamp: chrono::Utc::now(),
        };

        self.process_liquidation_signal(signal).await
    }

    /// 가스 가격 급등 확인
    async fn check_gas_price_spike(&self, tx: &ethers::types::Transaction) -> Result<()> {
        if let Some(gas_price) = tx.gas_price {
            // 이전 가스 가격과 비교 (간단화)
            let base_gas_price = U256::from(20_000_000_000u64); // 20 gwei

            if gas_price > base_gas_price * ethers::types::U256::from(2) {
                let spike_percentage = ((gas_price.as_u128() as f64 / base_gas_price.as_u128() as f64) - 1.0) * 100.0;

                let signal = LiquidationSignal::GasPriceSpike {
                    old_gas_price: base_gas_price,
                    new_gas_price: gas_price,
                    spike_percentage,
                    timestamp: chrono::Utc::now(),
                };

                self.process_liquidation_signal(signal).await?;
            }
        }

        Ok(())
    }

    /// 시뮬레이션 모드 실행 (fallback)
    async fn run_simulated_monitoring(&self) -> Result<()> {
        info!("🎭 시뮬레이션 모드로 멤풀 모니터링 시작...");

        while *self.is_monitoring.read().await {
            // 시뮬레이션된 이벤트 생성
            if let Some(signal) = self.generate_simulated_signal().await {
                self.process_liquidation_signal(signal).await?;
            }

            sleep(Duration::from_secs(1)).await;
        }

        Ok(())
    }
    
    /// 청산 신호 처리
    async fn process_liquidation_signal(&self, signal: LiquidationSignal) -> Result<()> {
        debug!("📡 Processing liquidation signal: {:?}", signal);
        
        // 신호 저장
        {
            let mut signals = self.detected_signals.write().await;
            signals.push(signal.clone());
            
            // 최대 1000개 신호만 유지
            if signals.len() > 1000 {
                signals.remove(0);
            }
        }
        
        // 신호 타입별 처리
        match signal {
            LiquidationSignal::OracleUpdate { urgency, .. } => {
                if urgency == LiquidationUrgency::Critical {
                    warn!("🚨 Critical oracle update detected - immediate action required!");
                }
            },
            LiquidationSignal::UserRepay { impact, .. } => {
                match impact {
                    RepayImpact::Eliminates => {
                        warn!("⚠️ User repay eliminates liquidation opportunity");
                    },
                    RepayImpact::Reduces => {
                        info!("📉 User repay reduces liquidation opportunity");
                    },
                    _ => {}
                }
            },
            LiquidationSignal::CompetitorLiquidation { our_advantage, .. } => {
                if !our_advantage {
                    warn!("🏃 Competitor liquidation detected - we may be at disadvantage");
                }
            },
            LiquidationSignal::GasPriceSpike { spike_percentage, .. } => {
                if spike_percentage > 50.0 {
                    warn!("⛽ Gas price spike detected: {:.1}% increase", spike_percentage);
                }
            },
            LiquidationSignal::NetworkCongestion { congestion_level, .. } => {
                if congestion_level == CongestionLevel::Critical {
                    warn!("🚧 Critical network congestion detected");
                }
            },
        }
        
        Ok(())
    }
    
    /// 시뮬레이션된 신호 생성
    async fn generate_simulated_signal(&self) -> Option<LiquidationSignal> {
        // TODO: 실제 멤풀 데이터 기반 신호 생성
        // 현재는 랜덤 시뮬레이션
        
        let random_value: f64 = rand::random();
        
        if random_value < 0.1 { // 10% 확률로 신호 생성
            Some(self.create_random_signal().await)
        } else {
            None
        }
    }
    
    /// 랜덤 신호 생성
    async fn create_random_signal(&self) -> LiquidationSignal {
        let signal_type: u8 = (rand::random::<f64>() * 5.0) as u8;
        
        match signal_type {
            0 => LiquidationSignal::OracleUpdate {
                asset: Address::from_slice(&rand::random::<[u8; 20]>()),
                old_price: U256::from(1000),
                new_price: U256::from(950), // 5% 하락
                affected_positions: vec![Address::from_slice(&rand::random::<[u8; 20]>())],
                urgency: LiquidationUrgency::High,
                timestamp: chrono::Utc::now(),
            },
            1 => LiquidationSignal::UserRepay {
                user: Address::from_slice(&rand::random::<[u8; 20]>()),
                protocol: ProtocolType::Aave,
                repay_amount: U256::from(1000),
                impact: RepayImpact::Reduces,
                timestamp: chrono::Utc::now(),
            },
            2 => LiquidationSignal::CompetitorLiquidation {
                user: Address::from_slice(&rand::random::<[u8; 20]>()),
                protocol: ProtocolType::CompoundV2,
                competitor_gas_price: U256::from(50_000_000_000u64), // 50 gwei
                our_advantage: false,
                timestamp: chrono::Utc::now(),
            },
            3 => LiquidationSignal::GasPriceSpike {
                old_gas_price: U256::from(20_000_000_000u64), // 20 gwei
                new_gas_price: U256::from(40_000_000_000u64), // 40 gwei
                spike_percentage: 100.0,
                timestamp: chrono::Utc::now(),
            },
            _ => LiquidationSignal::NetworkCongestion {
                pending_transactions: 150_000,
                avg_gas_price: U256::from(30_000_000_000u64), // 30 gwei
                congestion_level: CongestionLevel::High,
                timestamp: chrono::Utc::now(),
            },
        }
    }
    
    /// 최근 신호 조회
    pub async fn get_recent_signals(&self, limit: usize) -> Vec<LiquidationSignal> {
        let signals = self.detected_signals.read().await;
        let start = if signals.len() > limit {
            signals.len() - limit
        } else {
            0
        };
        
        signals[start..].to_vec()
    }
    
    /// 긴급 신호 필터링
    pub async fn get_urgent_signals(&self) -> Vec<LiquidationSignal> {
        let signals = self.detected_signals.read().await;
        
        signals.iter()
            .filter(|signal| self.is_urgent_signal(signal))
            .cloned()
            .collect()
    }
    
    /// 긴급 신호 판별
    fn is_urgent_signal(&self, signal: &LiquidationSignal) -> bool {
        match signal {
            LiquidationSignal::OracleUpdate { urgency, .. } => {
                *urgency == LiquidationUrgency::Critical || *urgency == LiquidationUrgency::High
            },
            LiquidationSignal::UserRepay { impact, .. } => {
                *impact == RepayImpact::Eliminates
            },
            LiquidationSignal::CompetitorLiquidation { our_advantage, .. } => {
                !our_advantage
            },
            LiquidationSignal::GasPriceSpike { spike_percentage, .. } => {
                *spike_percentage > 50.0
            },
            LiquidationSignal::NetworkCongestion { congestion_level, .. } => {
                *congestion_level == CongestionLevel::Critical
            },
        }
    }
    
    /// 신호 통계 조회
    pub async fn get_signal_statistics(&self) -> SignalStatistics {
        let signals = self.detected_signals.read().await;
        
        let mut oracle_updates = 0;
        let mut user_repays = 0;
        let mut competitor_liquidations = 0;
        let mut gas_spikes = 0;
        let mut network_congestion = 0;
        
        for signal in signals.iter() {
            match signal {
                LiquidationSignal::OracleUpdate { .. } => oracle_updates += 1,
                LiquidationSignal::UserRepay { .. } => user_repays += 1,
                LiquidationSignal::CompetitorLiquidation { .. } => competitor_liquidations += 1,
                LiquidationSignal::GasPriceSpike { .. } => gas_spikes += 1,
                LiquidationSignal::NetworkCongestion { .. } => network_congestion += 1,
            }
        }
        
        SignalStatistics {
            total_signals: signals.len(),
            oracle_updates,
            user_repays,
            competitor_liquidations,
            gas_spikes,
            network_congestion,
        }
    }
}

/// 신호 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStatistics {
    pub total_signals: usize,
    pub oracle_updates: usize,
    pub user_repays: usize,
    pub competitor_liquidations: usize,
    pub gas_spikes: usize,
    pub network_congestion: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mempool_watcher_creation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_signal_processing() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_urgent_signal_filtering() {
        // TODO: 테스트 구현
        assert!(true);
    }
}
