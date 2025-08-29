use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn, error};
use alloy::primitives::Address;
use ethers::providers::{Provider, Ws};
use tokio::time::{interval, Duration};
use tokio::sync::RwLock;

use crate::config::Config;
use crate::protocols::MultiProtocolScanner;
use crate::strategies::{LiquidationStrategyV2, LiquidationOpportunityV2, LiquidationStrategyStats};
use crate::mev::{MEVBundleExecutor, BundleExecutionResult, ExecutionStats};

/// 통합 청산 관리자 - 모든 청산 구성요소를 조율
pub struct IntegratedLiquidationManager {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    protocol_scanner: Arc<MultiProtocolScanner>,
    liquidation_strategy: Arc<LiquidationStrategyV2>,
    bundle_executor: Arc<MEVBundleExecutor>,
    
    // 상태 관리
    is_running: Arc<RwLock<bool>>,
    current_opportunities: Arc<RwLock<Vec<LiquidationOpportunityV2>>>,
    execution_history: Arc<RwLock<Vec<BundleExecutionResult>>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
}

#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub total_opportunities_detected: u64,
    pub opportunities_executed: u64,
    pub total_profit_earned: f64,
    pub total_gas_spent: f64,
    pub average_profit_per_execution: f64,
    pub execution_success_rate: f64,
    pub average_detection_time_ms: f64,
    pub uptime_seconds: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct LiquidationSummary {
    pub active_opportunities: usize,
    pub pending_executions: usize,
    pub total_potential_profit: f64,
    pub protocol_breakdown: HashMap<String, u32>,
    pub top_opportunities: Vec<LiquidationOpportunityV2>,
    pub recent_executions: Vec<BundleExecutionResult>,
    pub performance_metrics: PerformanceMetrics,
}

impl IntegratedLiquidationManager {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
    ) -> Result<Self> {
        info!("🏭 Initializing Integrated Liquidation Manager...");
        
        // 프로토콜 스캐너 초기화
        let protocol_scanner = Arc::new(
            MultiProtocolScanner::new(Arc::clone(&config), Arc::clone(&provider)).await?
        );
        
        // 청산 전략 초기화
        let liquidation_strategy = Arc::new(
            LiquidationStrategyV2::new(
                Arc::clone(&config),
                Arc::clone(&provider),
                Arc::clone(&protocol_scanner),
            ).await?
        );
        
        // MEV Bundle 실행자 초기화
        let bundle_executor = Arc::new(
            MEVBundleExecutor::new(Arc::clone(&config), Arc::clone(&provider)).await?
        );
        
        info!("✅ Integrated Liquidation Manager initialized");
        
        Ok(Self {
            config,
            provider,
            protocol_scanner,
            liquidation_strategy,
            bundle_executor,
            is_running: Arc::new(RwLock::new(false)),
            current_opportunities: Arc::new(RwLock::new(Vec::new())),
            execution_history: Arc::new(RwLock::new(Vec::new())),
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
        })
    }
    
    /// 자동 청산 봇 시작
    pub async fn start_automated_liquidation(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("⚠️ Automated liquidation already running");
            return Ok(());
        }
        *is_running = true;
        drop(is_running);
        
        info!("🚀 Starting automated liquidation bot...");
        
        // 백그라운드 스캐닝 시작
        self.start_background_scanning().await?;
        
        // 메인 실행 루프 시작
        let manager = Arc::new(self.clone());
        tokio::spawn(async move {
            manager.run_execution_loop().await;
        });
        
        info!("✅ Automated liquidation bot started");
        Ok(())
    }
    
    /// 자동 청산 봇 중지
    pub async fn stop_automated_liquidation(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            warn!("⚠️ Automated liquidation not running");
            return Ok(());
        }
        *is_running = false;
        
        // 프로토콜 스캐너 중지
        self.protocol_scanner.stop_background_scanning().await?;
        
        info!("🛑 Automated liquidation bot stopped");
        Ok(())
    }
    
    /// 백그라운드 스캐닝 시작
    async fn start_background_scanning(&self) -> Result<()> {
        self.protocol_scanner.start_background_scanning().await
    }
    
    /// 메인 실행 루프
    async fn run_execution_loop(&self) {
        let scan_interval = Duration::from_secs(
            self.config.liquidation.scan_interval_seconds.unwrap_or(30)
        );
        let mut interval_timer = interval(scan_interval);
        
        info!("🔄 Starting execution loop with {:.1}s interval", scan_interval.as_secs_f32());
        
        while *self.is_running.read().await {
            interval_timer.tick().await;
            
            let cycle_start = std::time::Instant::now();
            
            // 1. 기회 탐지 및 분석
            match self.detect_and_analyze_opportunities().await {
                Ok(opportunities) => {
                    if !opportunities.is_empty() {
                        // 2. 기회 실행
                        match self.execute_opportunities(opportunities).await {
                            Ok(results) => {
                                self.process_execution_results(results).await;
                            }
                            Err(e) => {
                                error!("❌ Execution failed: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Opportunity detection failed: {}", e);
                }
            }
            
            // 3. 성능 메트릭 업데이트
            self.update_performance_metrics(cycle_start.elapsed()).await;
            
            // 4. 만료된 Bundle 정리
            self.cleanup_expired_data().await;
        }
        
        info!("🏁 Execution loop stopped");
    }
    
    /// 기회 탐지 및 분석
    async fn detect_and_analyze_opportunities(&self) -> Result<Vec<LiquidationOpportunityV2>> {
        debug!("🔍 Detecting liquidation opportunities...");
        
        let opportunities = self.liquidation_strategy.detect_opportunities().await?;
        
        if !opportunities.is_empty() {
            info!("💡 Found {} liquidation opportunities", opportunities.len());
            
            // 현재 기회 업데이트
            *self.current_opportunities.write().await = opportunities.clone();
            
            // 통계 업데이트
            let mut metrics = self.performance_metrics.write().await;
            metrics.total_opportunities_detected += opportunities.len() as u64;
        }
        
        Ok(opportunities)
    }
    
    /// 기회 실행
    async fn execute_opportunities(&self, opportunities: Vec<LiquidationOpportunityV2>) -> Result<Vec<BundleExecutionResult>> {
        info!("⚡ Executing {} liquidation opportunities", opportunities.len());
        
        let current_block = 0u64; // TODO: Fix get_block_number method
        let target_block = current_block + 1;
        
        // 수익성 순으로 정렬하고 동시 실행 제한 적용
        let max_concurrent = self.config.liquidation.max_concurrent_liquidations as usize;
        let top_opportunities = opportunities.into_iter()
            .take(max_concurrent)
            .collect();
        
        // Bundle 실행
        let results = self.bundle_executor
            .execute_liquidation_opportunities(top_opportunities, target_block)
            .await?;
        
        Ok(results)
    }
    
    /// 실행 결과 처리
    async fn process_execution_results(&self, results: Vec<BundleExecutionResult>) {
        debug!("📊 Processing {} execution results", results.len());
        
        let mut total_profit = 0.0;
        let mut successful_executions = 0;
        
        for result in &results {
            if result.success {
                successful_executions += 1;
                if let Some(profit) = result.profit_realized {
                    total_profit += profit;
                }
                
                info!("✅ Liquidation successful: {} (${:.2} profit)", 
                      result.bundle_id, result.profit_realized.unwrap_or(0.0));
            } else {
                warn!("❌ Liquidation failed: {} - {}", 
                      result.bundle_id, result.error_message.as_deref().unwrap_or("Unknown error"));
            }
        }
        
        // 실행 기록 업데이트
        let mut history = self.execution_history.write().await;
        history.extend(results);
        
        // 최근 100개만 유지
        if history.len() > 100 {
            history.drain(0..history.len() - 100);
        }
        
        // 메트릭 업데이트
        let mut metrics = self.performance_metrics.write().await;
        metrics.opportunities_executed += successful_executions;
        metrics.total_profit_earned += total_profit;
        
        if metrics.opportunities_executed > 0 {
            metrics.average_profit_per_execution = metrics.total_profit_earned / metrics.opportunities_executed as f64;
            metrics.execution_success_rate = (metrics.opportunities_executed as f64) / (metrics.total_opportunities_detected as f64);
        }
        
        metrics.last_updated = chrono::Utc::now();
        
        info!("💰 Execution cycle complete: {} successful, ${:.2} total profit", 
              successful_executions, total_profit);
    }
    
    /// 성능 메트릭 업데이트
    async fn update_performance_metrics(&self, cycle_duration: std::time::Duration) {
        let mut metrics = self.performance_metrics.write().await;
        
        // 평균 탐지 시간 업데이트
        let cycle_ms = cycle_duration.as_millis() as f64;
        if metrics.total_opportunities_detected > 0 {
            let total_cycles = metrics.total_opportunities_detected as f64;
            metrics.average_detection_time_ms = 
                (metrics.average_detection_time_ms * (total_cycles - 1.0) + cycle_ms) / total_cycles;
        } else {
            metrics.average_detection_time_ms = cycle_ms;
        }
        
        metrics.uptime_seconds += cycle_duration.as_secs();
    }
    
    /// 만료된 데이터 정리
    async fn cleanup_expired_data(&self) {
        // Bundle 정리
        let cleaned_bundles = self.bundle_executor.cleanup_expired_bundles().await;
        if cleaned_bundles > 0 {
            debug!("🧹 Cleaned up {} expired bundles", cleaned_bundles);
        }
        
        // 기회 정리 (5분 이상 된 것들)
        let mut opportunities = self.current_opportunities.write().await;
        let cutoff_time = chrono::Utc::now() - chrono::Duration::minutes(5);
        let initial_count = opportunities.len();
        
        opportunities.retain(|opp| {
            opp.profitability_analysis.analysis_timestamp > cutoff_time
        });
        
        if opportunities.len() != initial_count {
            debug!("🧹 Cleaned up {} expired opportunities", initial_count - opportunities.len());
        }
    }
    
    /// 특정 사용자 청산 시도
    pub async fn liquidate_user(&self, user_address: Address) -> Result<BundleExecutionResult> {
        info!("🎯 Attempting to liquidate user: {}", user_address);
        
        if let Some(opportunity) = self.liquidation_strategy.analyze_specific_user(user_address).await? {
            info!("💰 Found liquidation opportunity for {}: ${:.2} profit", 
                  user_address, opportunity.strategy.net_profit_usd);
            
            let result = self.bundle_executor.execute_single_opportunity(opportunity).await?;
            
            // 결과를 실행 기록에 추가
            self.execution_history.write().await.push(result.clone());
            
            Ok(result)
        } else {
            Err(anyhow!("No liquidation opportunity found for user {}", user_address))
        }
    }
    
    /// 현재 상태 요약
    pub async fn get_liquidation_summary(&self) -> LiquidationSummary {
        let opportunities = self.current_opportunities.read().await;
        let execution_history = self.execution_history.read().await;
        let metrics = self.performance_metrics.read().await.clone();
        
        let pending_executions = self.bundle_executor.get_pending_bundle_count().await;
        
        let total_potential_profit: f64 = opportunities.iter()
            .map(|opp| opp.strategy.net_profit_usd)
            .sum();
        
        let mut protocol_breakdown = HashMap::new();
        for opp in opportunities.iter() {
            let protocol_name = format!("{:?}", opp.user.protocol);
            *protocol_breakdown.entry(protocol_name).or_insert(0) += 1;
        }
        
        let top_opportunities = opportunities.iter()
            .take(10)
            .cloned()
            .collect();
        
        let recent_executions = execution_history.iter()
            .rev()
            .take(20)
            .cloned()
            .collect();
        
        LiquidationSummary {
            active_opportunities: opportunities.len(),
            pending_executions,
            total_potential_profit,
            protocol_breakdown,
            top_opportunities,
            recent_executions,
            performance_metrics: metrics,
        }
    }
    
    /// 실행 통계 조회
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        self.bundle_executor.get_execution_stats().await
    }
    
    /// 전략 통계 조회
    pub async fn get_strategy_stats(&self) -> Result<LiquidationStrategyStats> {
        self.liquidation_strategy.get_strategy_stats().await
    }
    
    /// 프로토콜 요약 조회
    pub async fn get_protocol_summary(&self) -> Result<crate::protocols::LiquidationSummary> {
        self.protocol_scanner.get_liquidation_summary().await
    }
    
    /// 봇 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

impl Clone for IntegratedLiquidationManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            provider: Arc::clone(&self.provider),
            protocol_scanner: Arc::clone(&self.protocol_scanner),
            liquidation_strategy: Arc::clone(&self.liquidation_strategy),
            bundle_executor: Arc::clone(&self.bundle_executor),
            is_running: Arc::clone(&self.is_running),
            current_opportunities: Arc::clone(&self.current_opportunities),
            execution_history: Arc::clone(&self.execution_history),
            performance_metrics: Arc::clone(&self.performance_metrics),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_liquidation_manager_initialization() {
        // 테스트는 실제 네트워크 연결이 필요하므로 mock 환경에서 실행
        println!("Integrated Liquidation Manager tests require live network connection");
    }
    
    #[test]
    fn test_performance_metrics_calculation() {
        let mut metrics = PerformanceMetrics::default();
        
        // 기회 탐지
        metrics.total_opportunities_detected = 100;
        metrics.opportunities_executed = 85;
        metrics.total_profit_earned = 1250.0;
        
        // 계산
        metrics.average_profit_per_execution = metrics.total_profit_earned / metrics.opportunities_executed as f64;
        metrics.execution_success_rate = (metrics.opportunities_executed as f64) / (metrics.total_opportunities_detected as f64);
        
        assert!((metrics.average_profit_per_execution - 14.71).abs() < 0.01);
        assert!((metrics.execution_success_rate - 0.85).abs() < 0.01);
        
        println!("Performance metrics: {:#?}", metrics);
    }
}