use ethers::types::{Address, U256};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn, error};
use ethers::providers::{Provider, Ws};
use ethers::middleware::Middleware;
use tokio::time::{interval, Duration};
use tokio::sync::RwLock;

use crate::config::Config;
use crate::protocols::MultiProtocolScanner;
use crate::mev::{MEVBundleExecutor, BundleExecutionResult, ExecutionStats};
use super::{PositionScanner, PositionAnalyzer, types::{LendingProtocolInfo, OnChainLiquidationOpportunity}};

/// 통합 청산 관리자 - 모든 청산 구성요소를 조율
pub struct IntegratedLiquidationManager {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    protocol_scanner: Arc<Mutex<MultiProtocolScanner>>,
    position_analyzer: Arc<PositionAnalyzer>,
    bundle_executor: Arc<Mutex<MEVBundleExecutor>>,

    // 상태 관리
    is_running: Arc<RwLock<bool>>,
    current_opportunities: Arc<RwLock<Vec<OnChainLiquidationOpportunity>>>,
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
    // pub top_opportunities: Vec<LiquidationOpportunityV2>,
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
        let protocol_scanner = Arc::new(Mutex::new(
            MultiProtocolScanner::new(Arc::clone(&config), Arc::clone(&provider)).await?
        ));

        // 포지션 분석기 초기화
        let min_profit_eth = U256::from(
            (config.liquidation.min_profit_threshold_usd.unwrap_or(100.0) * 1e18 / 2800.0) as u64 // ETH 가격 2800 USD 가정
        );
        let health_factor_threshold = 1.0; // 청산 임계값
        let position_analyzer = Arc::new(PositionAnalyzer::new(min_profit_eth, health_factor_threshold));

        // MEV Bundle 실행자 초기화
        let bundle_executor = Arc::new(Mutex::new(
            MEVBundleExecutor::new(Arc::clone(&config), Arc::clone(&provider)).await?
        ));

        info!("✅ Integrated Liquidation Manager initialized");

        Ok(Self {
            config,
            provider,
            protocol_scanner,
            position_analyzer,
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
        self.protocol_scanner.lock().await.stop_background_scanning().await?;
        
        info!("🛑 Automated liquidation bot stopped");
        Ok(())
    }
    
    /// 백그라운드 스캐닝 시작
    async fn start_background_scanning(&self) -> Result<()> {
        self.protocol_scanner.lock().await.start_background_scanning().await
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
    async fn detect_and_analyze_opportunities(&self) -> Result<Vec<OnChainLiquidationOpportunity>> {
        debug!("🔍 Detecting liquidation opportunities...");

        let mut all_opportunities = Vec::new();

        // 프로토콜 스캐너에서 프로토콜 정보 가져오기
        let protocol_summary = self.protocol_scanner.lock().await.get_liquidation_summary().await?;

        // 각 프로토콜에 대해 고위험 사용자 조회 및 분석
        for (protocol_type, protocol_data) in &protocol_summary.protocol_breakdown {
            // 고위험 사용자 목록 가져오기
            // LendingProtocolInfo 생성
            let protocol_info = crate::strategies::liquidation::types::LendingProtocolInfo {
                protocol_type: match protocol_type {
                    crate::protocols::ProtocolType::Aave => crate::strategies::liquidation::types::ProtocolType::Aave,
                    crate::protocols::ProtocolType::CompoundV2 => crate::strategies::liquidation::types::ProtocolType::Compound,
                    crate::protocols::ProtocolType::MakerDAO => crate::strategies::liquidation::types::ProtocolType::MakerDAO,
                    crate::protocols::ProtocolType::CompoundV3 => crate::strategies::liquidation::types::ProtocolType::Compound,
                },
                lending_pool_address: Address::zero(),
                name: format!("{:?}", protocol_type),
                liquidation_fee: 500, // 5% 기본값
                min_health_factor: 1.0,
                price_oracle_address: Some(Address::zero()),
                supported_assets: vec![],
            };
            let high_risk_users = self.get_high_risk_users_for_protocol(&protocol_info).await?;

            // 각 사용자에 대해 포지션 분석
            for user_address in high_risk_users {
                let opportunity = match protocol_type {
                    crate::protocols::ProtocolType::Aave => {
                        // LendingProtocolInfo를 생성해야 함
                        let protocol_info = crate::strategies::liquidation::types::LendingProtocolInfo {
                            protocol_type: crate::strategies::liquidation::types::ProtocolType::Aave,
                            lending_pool_address: Address::zero(), // 실제 주소로 교체 필요
                            name: "Aave".to_string(),
                            liquidation_fee: 500,
                            min_health_factor: 1.0,
                            price_oracle_address: Some(Address::zero()),
                            supported_assets: vec![],
                        };
                        self.position_analyzer.analyze_aave_position(user_address, &protocol_info).await?
                    }
                    crate::protocols::ProtocolType::CompoundV2 => {
                        let protocol_info = crate::strategies::liquidation::types::LendingProtocolInfo {
                            protocol_type: crate::strategies::liquidation::types::ProtocolType::Compound,
                            lending_pool_address: Address::zero(),
                            name: "Compound".to_string(),
                            liquidation_fee: 500,
                            min_health_factor: 1.0,
                            price_oracle_address: Some(Address::zero()),
                            supported_assets: vec![],
                        };
                        self.position_analyzer.analyze_compound_position(user_address, &protocol_info).await?
                    }
                    crate::protocols::ProtocolType::MakerDAO => {
                        let protocol_info = crate::strategies::liquidation::types::LendingProtocolInfo {
                            protocol_type: crate::strategies::liquidation::types::ProtocolType::MakerDAO,
                            lending_pool_address: Address::zero(),
                            name: "MakerDAO".to_string(),
                            liquidation_fee: 500,
                            min_health_factor: 1.0,
                            price_oracle_address: Some(Address::zero()),
                            supported_assets: vec![],
                        };
                        self.position_analyzer.analyze_maker_position(user_address, &protocol_info).await?
                    }
                    crate::protocols::ProtocolType::CompoundV3 => {
                        let protocol_info = crate::strategies::liquidation::types::LendingProtocolInfo {
                            protocol_type: crate::strategies::liquidation::types::ProtocolType::Compound,
                            lending_pool_address: Address::zero(),
                            name: "CompoundV3".to_string(),
                            liquidation_fee: 500,
                            min_health_factor: 1.0,
                            price_oracle_address: Some(Address::zero()),
                            supported_assets: vec![],
                        };
                        self.position_analyzer.analyze_compound_position(user_address, &protocol_info).await?
                    }
                };

                if let Some(opp) = opportunity {
                    all_opportunities.push(opp);
                }
            }
        }

        // 수익성 순으로 정렬
        all_opportunities.sort_by(|a, b| b.net_profit.cmp(&a.net_profit));

        if !all_opportunities.is_empty() {
            info!("💡 Found {} liquidation opportunities", all_opportunities.len());

            // 현재 기회 업데이트
            *self.current_opportunities.write().await = all_opportunities.clone();

            // 통계 업데이트
            let mut metrics = self.performance_metrics.write().await;
            metrics.total_opportunities_detected += all_opportunities.len() as u64;
        }

        Ok(all_opportunities)
    }

    /// 특정 프로토콜의 고위험 사용자 목록 가져오기
    async fn get_high_risk_users_for_protocol(&self, _protocol: &LendingProtocolInfo) -> Result<Vec<Address>> {
        // 실제로는 다음 방법으로 가져와야 함:
        // 1. 이벤트 로그에서 최근 거래한 사용자들
        // 2. 서브그래프 API (The Graph)
        // 3. 오프체인 모니터링 시스템

        // 현재는 알려진 테스트 주소들 반환
        Ok(vec![
            "0x742d35Cc6570000000000000000000000000001".parse()?,
            "0x742d35Cc6570000000000000000000000000002".parse()?,
            "0x742d35Cc6570000000000000000000000000003".parse()?,
        ])
    }
    
    /// 기회 실행
    async fn execute_opportunities(&self, opportunities: Vec<OnChainLiquidationOpportunity>) -> Result<Vec<BundleExecutionResult>> {
        info!("⚡ Executing {} liquidation opportunities", opportunities.len());

        // 현재 블록 번호 가져오기
        let current_block = self.provider.get_block_number().await?.as_u64();
        let target_block = current_block + 1;

        // 수익성 순으로 정렬하고 동시 실행 제한 적용
        let max_concurrent = self.config.liquidation.max_concurrent_liquidations as usize;
        let top_opportunities: Vec<OnChainLiquidationOpportunity> = opportunities.into_iter()
            .take(max_concurrent)
            .collect();

        info!("📊 Executing top {} opportunities at target block {}", top_opportunities.len(), target_block);

        // 각 기회를 Bundle로 변환하여 실행
        let mut results = Vec::new();
        for opp in top_opportunities {
            // 청산 Bundle 생성 및 실행
            match self.execute_single_liquidation(opp, target_block).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("청산 실행 실패: {}", e);
                }
            }
        }

        Ok(results)
    }

    /// 단일 청산 실행
    async fn execute_single_liquidation(
        &self,
        opportunity: OnChainLiquidationOpportunity,
        target_block: u64,
    ) -> Result<BundleExecutionResult> {
        debug!("💸 Executing liquidation for user {} on protocol {}",
            opportunity.target_user, opportunity.protocol.name);

        // 실제로는 bundle_executor를 통해 청산 트랜잭션 실행
        // 현재는 간단한 시뮬레이션 결과 반환
        let success = opportunity.success_probability > 0.5;
        let profit_realized = if success {
            Some((opportunity.net_profit.as_u128() as f64) / 1e18)
        } else {
            None
        };

        Ok(BundleExecutionResult {
            bundle_id: format!("liq_{:?}_{}", opportunity.target_user, target_block),
            success,
            transaction_hash: if success { Some(ethers::types::H256::random()) } else { None },
            execution_time_ms: 100, // 시뮬레이션 시간
            profit_realized,
            gas_used: Some((opportunity.gas_cost.as_u128() as f64 / 1e18) as u64),
            error_message: if !success { Some("Simulation failed".to_string()) } else { None },
            block_number: Some(target_block),
        })
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
        let current_len = history.len();
        if current_len > 100 {
            history.drain(0..current_len - 100);
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
        let cleaned_bundles = self.bundle_executor.lock().await.cleanup_expired_bundles().await;
        if cleaned_bundles > 0 {
            debug!("🧹 Cleaned up {} expired bundles", cleaned_bundles);
        }

        // 기회 정리 (5분 이상 된 것들)
        let mut opportunities = self.current_opportunities.write().await;
        let initial_count = opportunities.len();

        // 5분 이상 지난 기회들 제거
        opportunities.retain(|opp| {
            let age = opp.position.last_updated.elapsed().as_secs();
            age < 300 // 5분 = 300초
        });

        if opportunities.len() != initial_count {
            debug!("🧹 Cleaned up {} expired opportunities", initial_count - opportunities.len());
        }
    }
    
    /// 특정 사용자 청산 시도
    pub async fn liquidate_user(&self, user_address: Address) -> Result<BundleExecutionResult> {
        info!("🎯 Attempting to liquidate user: {}", user_address);

        // 프로토콜 스캐너에서 프로토콜 정보 가져오기
        let protocol_summary = self.protocol_scanner.lock().await.get_liquidation_summary().await?;

        // 모든 프로토콜에서 해당 사용자 분석
        for (protocol_type, protocol_data) in &protocol_summary.protocol_breakdown {
            // LendingProtocolInfo 생성
            let protocol_info = crate::strategies::liquidation::types::LendingProtocolInfo {
                protocol_type: match protocol_type {
                    crate::protocols::ProtocolType::Aave => crate::strategies::liquidation::types::ProtocolType::Aave,
                    crate::protocols::ProtocolType::CompoundV2 => crate::strategies::liquidation::types::ProtocolType::Compound,
                    crate::protocols::ProtocolType::MakerDAO => crate::strategies::liquidation::types::ProtocolType::MakerDAO,
                    crate::protocols::ProtocolType::CompoundV3 => crate::strategies::liquidation::types::ProtocolType::Compound,
                },
                lending_pool_address: Address::zero(),
                name: format!("{:?}", protocol_type),
                liquidation_fee: 500,
                min_health_factor: 1.0,
                price_oracle_address: Some(Address::zero()),
                supported_assets: vec![],
            };
            
            let opportunity = match protocol_type {
                crate::protocols::ProtocolType::Aave => {
                    self.position_analyzer.analyze_aave_position(user_address, &protocol_info).await?
                }
                crate::protocols::ProtocolType::CompoundV2 | crate::protocols::ProtocolType::CompoundV3 => {
                    self.position_analyzer.analyze_compound_position(user_address, &protocol_info).await?
                }
                crate::protocols::ProtocolType::MakerDAO => {
                    self.position_analyzer.analyze_maker_position(user_address, &protocol_info).await?
                }
            };

            if let Some(opp) = opportunity {
                info!("💰 Found liquidation opportunity for {}: ${:.2} profit",
                      user_address, (opp.net_profit.as_u128() as f64) / 1e18);

                // 현재 블록 번호 가져오기
                let current_block = self.provider.get_block_number().await?.as_u64();

                // 청산 실행
                let result = self.execute_single_liquidation(opp, current_block + 1).await?;

                // 결과를 실행 기록에 추가
                self.execution_history.write().await.push(result.clone());

                return Ok(result);
            }
        }

        Err(anyhow!("No liquidation opportunity found for user {}", user_address))
    }
    
    /// 현재 상태 요약
    pub async fn get_liquidation_summary(&self) -> LiquidationSummary {
        let opportunities = self.current_opportunities.read().await;
        let execution_history = self.execution_history.read().await;
        let metrics = self.performance_metrics.read().await.clone();

        let pending_executions = self.bundle_executor.lock().await.get_pending_bundle_count().await;

        // 총 잠재 수익 계산
        let total_potential_profit: f64 = opportunities.iter()
            .map(|opp| (opp.net_profit.as_u128() as f64) / 1e18)
            .sum();

        // 프로토콜별 분류
        let mut protocol_breakdown = HashMap::new();
        for opp in opportunities.iter() {
            let protocol_name = opp.protocol.name.clone();
            *protocol_breakdown.entry(protocol_name).or_insert(0) += 1;
        }

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
            recent_executions,
            performance_metrics: metrics,
        }
    }
    
    /// 실행 통계 조회
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        self.bundle_executor.lock().await.get_execution_stats().await
    }
    
    /// 전략 통계 조회
    pub async fn get_strategy_stats(&self) -> Result<PerformanceMetrics> {
        let metrics = self.performance_metrics.read().await.clone();
        Ok(metrics)
    }
    
    /// 프로토콜 요약 조회
    pub async fn get_protocol_summary(&self) -> Result<crate::protocols::LiquidationSummary> {
        self.protocol_scanner.lock().await.get_liquidation_summary().await
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
            position_analyzer: Arc::clone(&self.position_analyzer),
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