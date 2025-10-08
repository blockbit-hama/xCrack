use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use anyhow::Result;
use tracing::info;
use ethers::types::{Address, U256};
use ethers::providers::{Provider, Ws};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::protocols::{MultiProtocolScanner, LiquidatableUser, ProtocolType, UserAccountData};
use crate::storage::Database;

/// 청산 상태 인덱서 - 프로토콜 상태 지속적 모니터링
pub struct LiquidationStateIndexer {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    protocol_scanner: Arc<Mutex<MultiProtocolScanner>>,
    database: Option<Arc<Database>>, // Optional for backward compatibility

    // 인덱싱된 상태 (메모리 캐시)
    indexed_positions: Arc<tokio::sync::RwLock<HashMap<Address, UserPosition>>>,
    liquidation_candidates: Arc<tokio::sync::RwLock<Vec<LiquidationCandidate>>>,

    // 인덱싱 상태
    is_indexing: Arc<tokio::sync::RwLock<bool>>,
    last_index_time: Arc<tokio::sync::RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
}

/// 사용자 포지션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPosition {
    pub user: Address,
    pub protocol: ProtocolType,
    pub account_data: UserAccountData,
    pub collateral_positions: Vec<CollateralPosition>,
    pub debt_positions: Vec<DebtPosition>,
    pub health_factor: f64,
    pub liquidation_threshold: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub is_liquidatable: bool,
}

/// 담보 포지션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralPosition {
    pub asset: Address,
    pub amount: U256,
    pub usd_value: f64,
    pub liquidation_threshold: f64,
    pub price_usd: f64,
    pub last_price_update: chrono::DateTime<chrono::Utc>,
}

/// 부채 포지션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtPosition {
    pub asset: Address,
    pub amount: U256,
    pub usd_value: f64,
    pub borrow_rate: f64,
    pub price_usd: f64,
    pub last_price_update: chrono::DateTime<chrono::Utc>,
}

/// 청산 후보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationCandidate {
    pub user: Address,
    pub protocol: ProtocolType,
    pub health_factor: f64,
    pub max_liquidatable_debt: U256,
    pub estimated_profit: U256,
    pub priority_score: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub urgency: LiquidationUrgency,
}

/// 청산 긴급도
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LiquidationUrgency {
    Low,      // 낮은 긴급도
    Medium,   // 중간 긴급도
    High,     // 높은 긴급도
    Critical, // 매우 긴급
}

/// 프로토콜 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub protocol_type: ProtocolType,
    pub enabled: bool,
    pub health_factor_threshold: f64,
    pub max_liquidation_pct: f64,
    pub liquidation_bonus: f64,
    pub close_factor: f64,
    pub scan_interval_seconds: u64,
}

impl LiquidationStateIndexer {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        protocol_scanner: Arc<Mutex<MultiProtocolScanner>>,
    ) -> Result<Self> {
        info!("📊 Initializing Liquidation State Indexer...");

        // PostgreSQL 데이터베이스 연결 시도
        let database = match Database::from_env().await {
            Ok(db) => {
                info!("✅ PostgreSQL database connected");
                Some(Arc::new(db))
            }
            Err(e) => {
                info!("⚠️  PostgreSQL not available: {}, using memory-only mode", e);
                None
            }
        };

        let indexed_positions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let liquidation_candidates = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let is_indexing = Arc::new(tokio::sync::RwLock::new(false));
        let last_index_time = Arc::new(tokio::sync::RwLock::new(None));

        Ok(Self {
            config,
            provider,
            protocol_scanner,
            database,
            indexed_positions,
            liquidation_candidates,
            is_indexing,
            last_index_time,
        })
    }
    
    /// 상태 인덱싱 시작
    pub async fn start_indexing(&self) -> Result<()> {
        info!("🚀 Starting liquidation state indexing...");
        
        {
            let mut is_indexing = self.is_indexing.write().await;
            *is_indexing = true;
        }
        
        // 주기적 인덱싱 루프
        self.indexing_loop().await?;
        
        Ok(())
    }
    
    /// 상태 인덱싱 중지
    pub async fn stop_indexing(&self) -> Result<()> {
        info!("🛑 Stopping liquidation state indexing...");
        
        {
            let mut is_indexing = self.is_indexing.write().await;
            *is_indexing = false;
        }
        
        Ok(())
    }
    
    /// 인덱싱 루프
    async fn indexing_loop(&self) -> Result<()> {
        while *self.is_indexing.read().await {
            let start_time = std::time::Instant::now();
            
            // 1. 모든 프로토콜에서 사용자 포지션 스캔
            self.scan_all_protocols().await?;
            
            // 2. 청산 후보 업데이트
            self.update_liquidation_candidates().await?;
            
            // 3. 인덱싱 시간 업데이트
            {
                let mut last_index_time = self.last_index_time.write().await;
                *last_index_time = Some(chrono::Utc::now());
            }
            
            let duration = start_time.elapsed();
            info!("✅ State indexing completed in {:?}", duration);
            
            // 다음 인덱싱까지 대기 (30초)
            sleep(Duration::from_secs(30)).await;
        }
        
        Ok(())
    }
    
    /// 모든 프로토콜 스캔
    async fn scan_all_protocols(&self) -> Result<()> {
        info!("🔍 Scanning all protocols for user positions...");
        
        // 모든 프로토콜에서 청산 가능한 사용자 스캔
        let liquidatable_users = self.protocol_scanner.lock().await.scan_all_protocols().await?;
        
        let mut total_users = 0;
        let mut liquidatable_count = 0;
        
        // 각 프로토콜의 사용자 포지션 인덱싱
        for (_protocol_type, users) in liquidatable_users {
            for user in users {
                let position = self.build_user_position(user.clone()).await?;

                // 메모리 캐시에 저장
                {
                    let mut positions = self.indexed_positions.write().await;
                    positions.insert(position.user, position.clone());
                }

                // PostgreSQL 데이터베이스에 저장
                if let Some(db) = &self.database {
                    if let Err(e) = db.upsert_user(&user).await {
                        tracing::warn!("Failed to save user to database: {}", e);
                    }
                }

                total_users += 1;
                if position.is_liquidatable {
                    liquidatable_count += 1;
                }
            }
        }

        info!("📊 Indexed {} total users, {} liquidatable", total_users, liquidatable_count);

        Ok(())
    }
    
    /// 사용자 포지션 빌드
    async fn build_user_position(&self, user: LiquidatableUser) -> Result<UserPosition> {
        let health_factor = user.account_data.health_factor;
        let is_liquidatable = health_factor < 1.0;
        
        // 담보 포지션 빌드
        let collateral_positions = user.collateral_positions.into_iter()
            .map(|pos| CollateralPosition {
                asset: pos.asset,
                amount: pos.amount,
                usd_value: pos.usd_value,
                liquidation_threshold: pos.liquidation_threshold,
                price_usd: pos.price_usd,
                last_price_update: chrono::Utc::now(),
            })
            .collect();
        
        // 부채 포지션 빌드
        let debt_positions = user.debt_positions.into_iter()
            .map(|pos| DebtPosition {
                asset: pos.asset,
                amount: pos.amount,
                usd_value: pos.usd_value,
                borrow_rate: pos.borrow_rate,
                price_usd: pos.price_usd,
                last_price_update: chrono::Utc::now(),
            })
            .collect();
        
        Ok(UserPosition {
            user: user.address,
            protocol: user.protocol.clone(),
            account_data: user.account_data,
            collateral_positions,
            debt_positions,
            health_factor,
            liquidation_threshold: self.get_protocol_liquidation_threshold(&user.protocol),
            last_updated: chrono::Utc::now(),
            is_liquidatable,
        })
    }
    
    /// 청산 후보 업데이트
    async fn update_liquidation_candidates(&self) -> Result<()> {
        let positions = self.indexed_positions.read().await;
        let mut candidates = Vec::new();
        
        for (_user, position) in positions.iter() {
            if position.is_liquidatable {
                let candidate = self.build_liquidation_candidate(position).await?;
                candidates.push(candidate);
            }
        }
        
        // 우선순위별 정렬
        candidates.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());
        
        // 후보 업데이트
        {
            let mut liquidation_candidates = self.liquidation_candidates.write().await;
            *liquidation_candidates = candidates;
        }
        
        info!("🎯 Updated {} liquidation candidates", self.liquidation_candidates.read().await.len());
        
        Ok(())
    }
    
    /// 청산 후보 빌드
    async fn build_liquidation_candidate(&self, position: &UserPosition) -> Result<LiquidationCandidate> {
        let health_factor = position.health_factor;
        let urgency = self.determine_urgency(health_factor);
        
        // 최대 청산 가능 부채 계산
        let max_liquidatable_debt = self.calculate_max_liquidatable_debt(position).await?;
        
        // 예상 수익 계산
        let estimated_profit = self.calculate_estimated_profit(position, max_liquidatable_debt).await?;
        
        // 우선순위 점수 계산
        let priority_score = self.calculate_priority_score(position, estimated_profit);
        
        Ok(LiquidationCandidate {
            user: position.user,
            protocol: position.protocol.clone(),
            health_factor,
            max_liquidatable_debt,
            estimated_profit,
            priority_score,
            last_updated: chrono::Utc::now(),
            urgency,
        })
    }
    
    /// 긴급도 결정
    fn determine_urgency(&self, health_factor: f64) -> LiquidationUrgency {
        if health_factor < 0.95 {
            LiquidationUrgency::Critical
        } else if health_factor < 0.98 {
            LiquidationUrgency::High
        } else if health_factor < 0.99 {
            LiquidationUrgency::Medium
        } else {
            LiquidationUrgency::Low
        }
    }
    
    /// 최대 청산 가능 부채 계산 - 프로토콜별 close factor 적용
    async fn calculate_max_liquidatable_debt(&self, position: &UserPosition) -> Result<U256> {
        let close_factor = self.get_protocol_close_factor(&position.protocol);
        let total_debt_usd = position.account_data.total_debt_usd;
        let max_liquidatable_usd = total_debt_usd * close_factor;

        // USD를 토큰 단위로 변환 (간단화)
        let max_liquidatable = U256::from((max_liquidatable_usd * 1e18) as u64);

        Ok(max_liquidatable)
    }

    /// 프로토콜별 청산 임계값 조회
    fn get_protocol_liquidation_threshold(&self, protocol_type: &ProtocolType) -> f64 {
        match protocol_type {
            ProtocolType::Aave => 0.825,      // Aave V3: 평균 82.5%
            ProtocolType::CompoundV2 => 0.80, // Compound V2: 80%
            ProtocolType::CompoundV3 => 0.83, // Compound V3: 83%
            ProtocolType::MakerDAO => 0.85,   // MakerDAO: 85%
            _ => 0.80, // 기본값: 80%
        }
    }

    /// 프로토콜별 close factor 조회
    fn get_protocol_close_factor(&self, protocol_type: &ProtocolType) -> f64 {
        match protocol_type {
            ProtocolType::Aave => 0.50,      // Aave V3: 50% (최대 청산 가능)
            ProtocolType::CompoundV2 => 0.50, // Compound V2: 50%
            ProtocolType::CompoundV3 => 1.00, // Compound V3: 100% (전체 청산 가능)
            ProtocolType::MakerDAO => 1.00,   // MakerDAO: 100% (전체 청산 가능)
            _ => 0.50, // 기본값: 50%
        }
    }
    
    /// 예상 수익 계산 - 프로토콜별 청산 보너스 적용
    async fn calculate_estimated_profit(&self, position: &UserPosition, liquidation_amount: U256) -> Result<U256> {
        let liquidation_bonus = self.get_protocol_liquidation_bonus(&position.protocol);

        // 청산 시 받을 담보 계산
        let bonus_multiplier = U256::from((liquidation_bonus * 100.0) as u64);
        let estimated_collateral_received = liquidation_amount + (liquidation_amount * bonus_multiplier / U256::from(100));

        // 가스 비용 및 슬리피지 차감
        let gas_cost = U256::from(500_000 * 20_000_000_000u64); // 500k gas * 20 gwei
        let slippage_cost = estimated_collateral_received * U256::from(2) / U256::from(100); // 2% 슬리피지

        let estimated_profit = if estimated_collateral_received > liquidation_amount + gas_cost + slippage_cost {
            estimated_collateral_received - liquidation_amount - gas_cost - slippage_cost
        } else {
            U256::from(0)
        };

        Ok(estimated_profit)
    }

    /// 프로토콜별 청산 보너스 조회
    fn get_protocol_liquidation_bonus(&self, protocol_type: &ProtocolType) -> f64 {
        match protocol_type {
            ProtocolType::Aave => 0.05,      // Aave V3: 5% 보너스
            ProtocolType::CompoundV2 => 0.08, // Compound V2: 8% 보너스
            ProtocolType::CompoundV3 => 0.05, // Compound V3: 5% 보너스
            ProtocolType::MakerDAO => 0.13,   // MakerDAO: 13% 보너스
            _ => 0.05, // 기본값: 5%
        }
    }
    
    /// 우선순위 점수 계산
    fn calculate_priority_score(&self, position: &UserPosition, estimated_profit: U256) -> f64 {
        let profit_score = estimated_profit.as_u128() as f64 / 1e18;
        let urgency_score = match self.determine_urgency(position.health_factor) {
            LiquidationUrgency::Critical => 1.0,
            LiquidationUrgency::High => 0.8,
            LiquidationUrgency::Medium => 0.6,
            LiquidationUrgency::Low => 0.4,
        };
        let size_score = position.account_data.total_debt_usd / 1_000_000.0; // 100만 달러 기준
        
        profit_score * 0.5 + urgency_score * 0.3 + size_score * 0.2
    }
    
    /// 청산 후보 조회
    pub async fn get_liquidation_candidates(&self, limit: Option<usize>) -> Vec<LiquidationCandidate> {
        let candidates = self.liquidation_candidates.read().await;
        
        if let Some(limit) = limit {
            candidates.iter().take(limit).cloned().collect()
        } else {
            candidates.clone()
        }
    }
    
    /// 특정 사용자 포지션 조회
    pub async fn get_user_position(&self, user: Address) -> Option<UserPosition> {
        let positions = self.indexed_positions.read().await;
        positions.get(&user).cloned()
    }
    
    /// 인덱싱 통계 조회
    pub async fn get_indexing_statistics(&self) -> IndexingStatistics {
        let positions = self.indexed_positions.read().await;
        let candidates = self.liquidation_candidates.read().await;
        let last_index_time = self.last_index_time.read().await;
        
        let total_positions = positions.len();
        let liquidatable_positions = candidates.len();
        
        IndexingStatistics {
            total_positions,
            liquidatable_positions,
            last_index_time: *last_index_time,
            is_indexing: *self.is_indexing.read().await,
        }
    }
}

/// 인덱싱 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatistics {
    pub total_positions: usize,
    pub liquidatable_positions: usize,
    pub last_index_time: Option<chrono::DateTime<chrono::Utc>>,
    pub is_indexing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_state_indexer_creation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_position_indexing() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_candidate_building() {
        // TODO: 테스트 구현
        assert!(true);
    }
}
