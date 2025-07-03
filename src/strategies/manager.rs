use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, debug, error, warn};
use futures::future::join_all;
use std::collections::HashMap;
use std::time::Instant;
use ethers::providers::{Provider, Ws};

use crate::config::Config;
use crate::types::{Transaction, Opportunity, StrategyType};
use crate::strategies::Strategy;
use crate::strategies::MempoolArbitrageStrategy;
use crate::strategies::RealTimeSandwichStrategy;
use crate::strategies::CompetitiveLiquidationStrategy;

pub struct StrategyManager {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    strategies: HashMap<StrategyType, Box<dyn Strategy + Send + Sync>>,
    performance_stats: Arc<RwLock<HashMap<StrategyType, StrategyStats>>>,
}

#[derive(Debug, Clone)]
pub struct StrategyStats {
    pub transactions_analyzed: u64,
    pub opportunities_found: u64,
    pub avg_analysis_time_ms: f64,
    pub last_analysis_time: Option<Instant>,
}

impl StrategyManager {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        let mut strategies = HashMap::new();
        let mut performance_stats = HashMap::new();
        
        // 차익거래 전략 초기화
        if config.strategies.arbitrage.enabled {
            info!("🎯 차익거래 전략 초기화 중...");
            match MempoolArbitrageStrategy::new(Arc::clone(&config), Arc::clone(&provider)).await {
                Ok(arbitrage_strategy) => {
                    strategies.insert(StrategyType::Arbitrage, Box::new(arbitrage_strategy));
                    info!("✅ 차익거래 전략 초기화 완료");
                }
                Err(e) => {
                    error!("❌ 차익거래 전략 초기화 실패: {}", e);
                }
            }
            
            performance_stats.insert(StrategyType::Arbitrage, StrategyStats {
                transactions_analyzed: 0,
                opportunities_found: 0,
                avg_analysis_time_ms: 0.0,
                last_analysis_time: None,
            });
        }
        
        // 샌드위치 전략 초기화
        if config.strategies.sandwich.enabled {
            info!("🥪 샌드위치 전략 초기화 중...");
            match RealTimeSandwichStrategy::new(Arc::clone(&config), Arc::clone(&provider)).await {
                Ok(sandwich_strategy) => {
                    strategies.insert(StrategyType::Sandwich, Box::new(sandwich_strategy));
                    info!("✅ 샌드위치 전략 초기화 완료");
                }
                Err(e) => {
                    error!("❌ 샌드위치 전략 초기화 실패: {}", e);
                }
            }
            
            performance_stats.insert(StrategyType::Sandwich, StrategyStats {
                transactions_analyzed: 0,
                opportunities_found: 0,
                avg_analysis_time_ms: 0.0,
                last_analysis_time: None,
            });
        }
        
        // 청산 전략 초기화
        if config.strategies.liquidation.enabled {
            info!("💸 청산 전략 초기화 중...");
            match CompetitiveLiquidationStrategy::new(Arc::clone(&config), Arc::clone(&provider)).await {
                Ok(liquidation_strategy) => {
                    strategies.insert(StrategyType::Liquidation, Box::new(liquidation_strategy));
                    info!("✅ 청산 전략 초기화 완료");
                }
                Err(e) => {
                    error!("❌ 청산 전략 초기화 실패: {}", e);
                }
            }
            
            performance_stats.insert(StrategyType::Liquidation, StrategyStats {
                transactions_analyzed: 0,
                opportunities_found: 0,
                avg_analysis_time_ms: 0.0,
                last_analysis_time: None,
            });
        }
        
        info!("📊 총 {}개 전략 초기화됨", strategies.len());
        
        Ok(Self {
            config,
            provider,
            strategies,
            performance_stats: Arc::new(RwLock::new(performance_stats)),
        })
    }

    /// 모든 활성 전략으로 트랜잭션을 병렬 분석
    pub async fn analyze_transaction(&self, tx: &Transaction) -> Vec<Opportunity> {
        let start_time = Instant::now();
        let mut all_opportunities = Vec::new();
        
        let mut analysis_futures = Vec::new();
        
        // 각 전략에 대해 병렬 분석 실행
        for (strategy_type, strategy) in &self.strategies {
            if strategy.is_enabled() {
                let strategy_clone = strategy.clone();
                let tx_clone = tx.clone();
                let strategy_type_clone = *strategy_type;
                
                let future = async move {
                    let analysis_start = Instant::now();
                    let result = strategy_clone.analyze(&tx_clone).await;
                    let analysis_duration = analysis_start.elapsed();
                    
                    (strategy_type_clone, result, analysis_duration)
                };
                
                analysis_futures.push(future);
            }
        }
        
        // 모든 분석 완료 대기
        let results = join_all(analysis_futures).await;
        
        // 결과 수집 및 성능 통계 업데이트
        for (strategy_type, result, analysis_duration) in results {
            match result {
                Ok(opportunities) => {
                    debug!("📊 {} 전략에서 {}개 기회 발견", strategy_type, opportunities.len());
                    all_opportunities.extend(opportunities);
                    
                    // 성능 통계 업데이트
                    self.update_strategy_stats(strategy_type, analysis_duration, opportunities.len()).await;
                }
                Err(e) => {
                    error!("❌ {} 전략 분석 실패: {}", strategy_type, e);
                }
            }
        }
        
        let total_duration = start_time.elapsed();
        debug!("⏱️ 전체 분석 시간: {:.2}ms, 발견된 기회: {}", 
               total_duration.as_millis(), all_opportunities.len());
        
        all_opportunities
    }

    /// 기회 검증
    pub async fn validate_opportunities(&self, opportunities: Vec<Opportunity>) -> Vec<Opportunity> {
        let mut valid_opportunities = Vec::new();
        
        for opportunity in opportunities {
            if let Some(strategy) = self.strategies.get(&opportunity.strategy) {
                match strategy.validate_opportunity(&opportunity).await {
                    Ok(is_valid) => {
                        if is_valid {
                            valid_opportunities.push(opportunity);
                        } else {
                            debug!("❌ 기회 검증 실패: {}", opportunity.id);
                        }
                    }
                    Err(e) => {
                        error!("❌ 기회 검증 오류: {}", e);
                    }
                }
            }
        }
        
        info!("✅ {}개 기회 중 {}개 검증 통과", opportunities.len(), valid_opportunities.len());
        valid_opportunities
    }

    /// 전략별 번들 생성
    pub async fn create_bundles(&self, opportunities: Vec<Opportunity>) -> Vec<crate::types::Bundle> {
        let mut bundles = Vec::new();
        
        for opportunity in opportunities {
            if let Some(strategy) = self.strategies.get(&opportunity.strategy) {
                match strategy.create_bundle(&opportunity).await {
                    Ok(bundle) => {
                        info!("📦 번들 생성됨: {} (전략: {})", bundle.id, opportunity.strategy);
                        bundles.push(bundle);
                    }
                    Err(e) => {
                        error!("❌ 번들 생성 실패: {} (전략: {})", e, opportunity.strategy);
                    }
                }
            }
        }
        
        info!("📦 총 {}개 번들 생성됨", bundles.len());
        bundles
    }

    /// 전략 성능 통계 업데이트
    async fn update_strategy_stats(&self, strategy_type: StrategyType, duration: std::time::Duration, opportunities_found: usize) {
        let mut stats = self.performance_stats.write().await;
        if let Some(stat) = stats.get_mut(&strategy_type) {
            stat.transactions_analyzed += 1;
            stat.opportunities_found += opportunities_found as u64;
            stat.last_analysis_time = Some(Instant::now());
            
            // 평균 분석 시간 업데이트
            let duration_ms = duration.as_millis() as f64;
            stat.avg_analysis_time_ms = (stat.avg_analysis_time_ms * (stat.transactions_analyzed - 1) as f64 + duration_ms) / stat.transactions_analyzed as f64;
        }
    }

    /// 전략별 성능 통계 조회
    pub async fn get_strategy_stats(&self) -> HashMap<StrategyType, StrategyStats> {
        self.performance_stats.read().await.clone()
    }

    /// 전략 활성화/비활성화 (이제 직접 전략에 접근)
    pub async fn set_strategy_enabled(&self, strategy_type: StrategyType, enabled: bool) -> Result<()> {
        if let Some(strategy) = self.strategies.get(&strategy_type) {
            if enabled {
                strategy.start().await?;
            } else {
                strategy.stop().await?;
            }
            info!("{} 전략 {}됨", strategy_type, if enabled { "활성화" } else { "비활성화" });
        } else {
            warn!("전략을 찾을 수 없음: {}", strategy_type);
        }
        Ok(())
    }

    /// 모든 전략 시작
    pub async fn start_all_strategies(&self) -> Result<()> {
        info!("🚀 모든 전략 시작 중...");
        
        for (strategy_type, strategy) in &self.strategies {
            match strategy.start().await {
                Ok(_) => {
                    info!("✅ {} 전략 시작됨", strategy_type);
                }
                Err(e) => {
                    error!("❌ {} 전략 시작 실패: {}", strategy_type, e);
                }
            }
        }
        
        info!("🎯 모든 전략이 시작되었습니다");
        Ok(())
    }

    /// 모든 전략 중지
    pub async fn stop_all_strategies(&self) -> Result<()> {
        info!("⏹️ 모든 전략 중지 중...");
        
        for (strategy_type, strategy) in &self.strategies {
            match strategy.stop().await {
                Ok(_) => {
                    info!("✅ {} 전략 중지됨", strategy_type);
                }
                Err(e) => {
                    error!("❌ {} 전략 중지 실패: {}", strategy_type, e);
                }
            }
        }
        
        info!("🛑 모든 전략이 중지되었습니다");
        Ok(())
    }

    /// 활성 전략 수 조회
    pub fn get_active_strategy_count(&self) -> usize {
        self.strategies.values().filter(|s| s.is_enabled()).count()
    }
}

impl std::fmt::Debug for StrategyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategyManager")
            .field("strategy_count", &self.strategies.len())
            .field("active_strategies", &self.get_active_strategy_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_strategy_manager_creation() {
        let config = Arc::new(Config::default());
        let provider = Arc::new(Provider::new(ethers::providers::Ws::connect("wss://dummy").await.unwrap()));
        
        let manager = StrategyManager::new(config, provider).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_strategy_stats_update() {
        let config = Arc::new(Config::default());
        let provider = Arc::new(Provider::new(ethers::providers::Ws::connect("wss://dummy").await.unwrap()));
        
        let manager = StrategyManager::new(config, provider).await.unwrap();
        let stats = manager.get_strategy_stats().await;
        assert!(!stats.is_empty());
    }
} 