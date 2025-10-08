use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, debug};
use ethers::types::U256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Opportunity, OpportunityType, StrategyType};
use crate::config::Config;
use super::priority_queue::{OpportunityQueue, OpportunityPriority};
use super::scoring::OpportunityScorer;

/// 기회 관리자
pub struct OpportunityManager {
    /// 설정
    config: Arc<Config>,
    
    /// 전략별 우선순위 큐
    strategy_queues: HashMap<StrategyType, Arc<OpportunityQueue>>,
    
    /// 전체 우선순위 큐
    global_queue: Arc<OpportunityQueue>,
    
    /// 점수 계산기
    scorer: Arc<RwLock<OpportunityScorer>>,
    
    /// 실행 중인 기회 추적
    executing: Arc<RwLock<HashMap<String, OpportunityPriority>>>,
    
    /// 완료된 기회 히스토리
    history: Arc<RwLock<Vec<ExecutionRecord>>>,
    
    /// 통계
    stats: Arc<RwLock<ManagerStats>>,
}

/// 실행 기록
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub opportunity_id: String,
    pub opportunity_type: OpportunityType,
    pub strategy: StrategyType,
    pub expected_profit: U256,
    pub actual_profit: Option<U256>,
    pub gas_used: U256,
    pub success: bool,
    pub error_message: Option<String>,
    pub executed_at: u64,
    pub execution_time_ms: u64,
}

/// 관리자 통계
#[derive(Debug, Clone, Default)]
pub struct ManagerStats {
    pub total_opportunities: u64,
    pub total_executed: u64,
    pub total_successful: u64,
    pub total_failed: u64,
    pub total_expired: u64,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub strategy_stats: HashMap<StrategyType, StrategyStats>,
}

/// 전략별 통계
#[derive(Debug, Clone, Default)]
pub struct StrategyStats {
    pub total_opportunities: u64,
    pub total_executed: u64,
    pub total_successful: u64,
    pub total_profit: U256,
    pub avg_profit: U256,
    pub success_rate: f64,
}

impl OpportunityManager {
    /// 새로운 기회 관리자 생성
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let min_profit = U256::from_str_radix(
            &config.strategies.sandwich.min_profit_eth,
            10
        ).unwrap_or(U256::from(10).pow(U256::from(17))); // 0.1 ETH
        
        let scorer = Arc::new(RwLock::new(
            OpportunityScorer::new(min_profit, 0.7)
        ));
        
        // 전략별 큐 생성
        let mut strategy_queues = HashMap::new();
        
        // 샌드위치 전략 큐
        strategy_queues.insert(
            StrategyType::Sandwich,
            Arc::new(OpportunityQueue::new(100, 60))  // 최대 100개, 60초 TTL
        );
        
        // 아비트라지 전략 큐
        strategy_queues.insert(
            StrategyType::MicroArbitrage,
            Arc::new(OpportunityQueue::new(50, 30))  // 최대 50개, 30초 TTL
        );
        
        // 청산 전략 큐
        strategy_queues.insert(
            StrategyType::Liquidation,
            Arc::new(OpportunityQueue::new(30, 120))  // 최대 30개, 120초 TTL
        );
        
        // 전체 우선순위 큐
        let global_queue = Arc::new(OpportunityQueue::new(200, 60));
        
        info!("🎯 OpportunityManager 초기화 완료");
        
        Ok(Self {
            config,
            strategy_queues,
            global_queue,
            scorer,
            executing: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(ManagerStats::default())),
        })
    }
    
    /// 기회 추가
    pub async fn add_opportunity(&self, opportunity: Opportunity) -> Result<bool> {
        let start = SystemTime::now();
        
        // 점수 계산
        let scorer = self.scorer.read().await;
        let ttl = self.get_ttl_for_strategy(&opportunity.strategy);
        let priority = scorer.score_opportunity(&opportunity, ttl);
        
        // ID 생성
        let opportunity_id = self.generate_opportunity_id(&opportunity);
        debug!("Adding opportunity: id={}, type={:?}, profit={}, score={:.3}",
            opportunity_id,
            opportunity.opportunity_type,
            opportunity.expected_profit,
            priority.priority_score
        );
        
        // 전략별 큐에 추가
        let added_to_strategy = if let Some(queue) = self.strategy_queues.get(&opportunity.strategy) {
            queue.push(priority.clone()).await?
        } else {
            false
        };
        
        // 전체 큐에 추가
        let added_to_global = self.global_queue.push(priority).await?;
        
        // 통계 업데이트
        if added_to_strategy || added_to_global {
            let mut stats = self.stats.write().await;
            stats.total_opportunities += 1;
            
            let strategy_stats = stats.strategy_stats
                .entry(opportunity.strategy)
                .or_insert_with(StrategyStats::default);
            strategy_stats.total_opportunities += 1;
        }
        
        let elapsed = start.elapsed().unwrap().as_millis();
        debug!("Opportunity processing took {}ms", elapsed);
        
        Ok(added_to_strategy || added_to_global)
    }
    
    /// 배치로 기회 추가
    pub async fn add_opportunities_batch(&self, opportunities: Vec<Opportunity>) -> Result<usize> {
        let mut added_count = 0;
        
        for opportunity in opportunities {
            if self.add_opportunity(opportunity).await? {
                added_count += 1;
            }
        }
        
        info!("Added {} opportunities to queue", added_count);
        Ok(added_count)
    }
    
    /// 다음 실행할 기회 가져오기
    pub async fn get_next_opportunity(&self) -> Option<OpportunityPriority> {
        // 전체 큐에서 먼저 확인
        if let Some(opp) = self.global_queue.pop().await {
            self.mark_as_executing(opp.clone()).await;
            return Some(opp);
        }
        
        // 전략별 큐 확인 (우선순위 순)
        let strategies = vec![
            StrategyType::Sandwich,    // 가장 높은 우선순위
            StrategyType::MicroArbitrage,    // 중간 우선순위
            StrategyType::Liquidation,  // 낮은 우선순위
        ];
        
        for strategy in strategies {
            if let Some(queue) = self.strategy_queues.get(&strategy) {
                if let Some(opp) = queue.pop().await {
                    self.mark_as_executing(opp.clone()).await;
                    return Some(opp);
                }
            }
        }
        
        None
    }
    
    /// 특정 전략의 다음 기회 가져오기
    pub async fn get_next_opportunity_for_strategy(&self, strategy: StrategyType) -> Option<OpportunityPriority> {
        if let Some(queue) = self.strategy_queues.get(&strategy) {
            if let Some(opp) = queue.pop().await {
                self.mark_as_executing(opp.clone()).await;
                return Some(opp);
            }
        }
        
        // 전체 큐에서 해당 전략 찾기
        if let Some(opp) = self.global_queue.pop_by_strategy(strategy).await {
            self.mark_as_executing(opp.clone()).await;
            return Some(opp);
        }
        
        None
    }
    
    /// 여러 개의 기회 가져오기
    pub async fn get_opportunities_batch(&self, count: usize) -> Vec<OpportunityPriority> {
        let mut opportunities = Vec::new();
        
        // 전체 큐에서 가져오기
        let global_batch = self.global_queue.pop_batch(count).await;
        for opp in global_batch {
            self.mark_as_executing(opp.clone()).await;
            opportunities.push(opp);
        }
        
        // 부족한 경우 전략별 큐에서 추가
        if opportunities.len() < count {
            let remaining = count - opportunities.len();
            for (_, queue) in &self.strategy_queues {
                let strategy_batch = queue.pop_batch(remaining - opportunities.len()).await;
                for opp in strategy_batch {
                    self.mark_as_executing(opp.clone()).await;
                    opportunities.push(opp);
                    
                    if opportunities.len() >= count {
                        break;
                    }
                }
                
                if opportunities.len() >= count {
                    break;
                }
            }
        }
        
        opportunities
    }
    
    /// 실행 중으로 표시
    async fn mark_as_executing(&self, opportunity: OpportunityPriority) {
        let id = self.generate_opportunity_id(&opportunity.opportunity);
        let mut executing = self.executing.write().await;
        executing.insert(id, opportunity);
    }
    
    /// 실행 완료 기록
    pub async fn record_execution(
        &self,
        opportunity_id: String,
        success: bool,
        actual_profit: Option<U256>,
        gas_used: U256,
        error_message: Option<String>,
        execution_time_ms: u64,
    ) -> Result<()> {
        // 실행 중 목록에서 제거
        let opportunity = {
            let mut executing = self.executing.write().await;
            executing.remove(&opportunity_id)
        };
        
        if let Some(opp) = opportunity {
            // 실행 기록 생성
            let record = ExecutionRecord {
                opportunity_id: opportunity_id.clone(),
                opportunity_type: opp.opportunity.opportunity_type,
                strategy: opp.opportunity.strategy,
                expected_profit: opp.opportunity.expected_profit,
                actual_profit,
                gas_used,
                success,
                error_message,
                executed_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                execution_time_ms,
            };
            
            // 히스토리에 추가
            {
                let mut history = self.history.write().await;
                history.push(record.clone());
                // 최대 1000개만 유지
                let len = history.len();
                if len > 1000 {
                    let remove_count = len - 1000;
                    history.drain(0..remove_count);
                }
            }
            
            // 통계 업데이트
            self.update_stats(record).await;
            
            info!(
                "Execution recorded: id={}, success={}, profit={:?}, gas={}",
                opportunity_id,
                success,
                actual_profit,
                gas_used
            );
        }
        
        Ok(())
    }
    
    /// 통계 업데이트
    async fn update_stats(&self, record: ExecutionRecord) {
        let mut stats = self.stats.write().await;
        
        stats.total_executed += 1;
        if record.success {
            stats.total_successful += 1;
            if let Some(profit) = record.actual_profit {
                stats.total_profit += profit;
            }
        } else {
            stats.total_failed += 1;
        }
        
        stats.total_gas_spent += record.gas_used;
        
        // 평균 실행 시간 업데이트
        if stats.total_executed == 1 {
            stats.avg_execution_time_ms = record.execution_time_ms as f64;
        } else {
            stats.avg_execution_time_ms = 
                (stats.avg_execution_time_ms * (stats.total_executed - 1) as f64 
                 + record.execution_time_ms as f64) / stats.total_executed as f64;
        }
        
        // 성공률 계산
        stats.success_rate = if stats.total_executed > 0 {
            stats.total_successful as f64 / stats.total_executed as f64
        } else {
            0.0
        };
        
        // 전략별 통계 업데이트
        let strategy_stats = stats.strategy_stats
            .entry(record.strategy)
            .or_insert_with(StrategyStats::default);
        
        strategy_stats.total_executed += 1;
        if record.success {
            strategy_stats.total_successful += 1;
            if let Some(profit) = record.actual_profit {
                strategy_stats.total_profit += profit;
            }
        }
        
        strategy_stats.success_rate = if strategy_stats.total_executed > 0 {
            strategy_stats.total_successful as f64 / strategy_stats.total_executed as f64
        } else {
            0.0
        };
        
        if strategy_stats.total_successful > 0 {
            strategy_stats.avg_profit = strategy_stats.total_profit / U256::from(strategy_stats.total_successful);
        }
    }
    
    /// 네트워크 상태 업데이트
    pub async fn update_network_state(&self, congestion: f64, competitors: u32) {
        let mut scorer = self.scorer.write().await;
        scorer.update_network_state(congestion, competitors);
    }
    
    /// 큐 상태 가져오기
    pub async fn get_queue_status(&self) -> HashMap<String, QueueStatus> {
        let mut status = HashMap::new();
        
        // 전체 큐 상태
        status.insert(
            "global".to_string(),
            QueueStatus {
                size: self.global_queue.size().await,
                stats: self.global_queue.get_stats().await.into(),
            }
        );
        
        // 전략별 큐 상태
        for (strategy, queue) in &self.strategy_queues {
            status.insert(
                format!("{:?}", strategy),
                QueueStatus {
                    size: queue.size().await,
                    stats: queue.get_stats().await.into(),
                }
            );
        }
        
        status
    }
    
    /// 통계 가져오기
    pub async fn get_stats(&self) -> ManagerStats {
        let stats = self.stats.read().await;
        stats.clone()
    }
    
    /// 실행 히스토리 가져오기
    pub async fn get_history(&self, limit: usize) -> Vec<ExecutionRecord> {
        let history = self.history.read().await;
        let start = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };
        
        history[start..].to_vec()
    }
    
    /// 모든 큐 비우기
    pub async fn clear_all_queues(&self) {
        self.global_queue.clear().await;
        for queue in self.strategy_queues.values() {
            queue.clear().await;
        }
        
        info!("All opportunity queues cleared");
    }
    
    /// 기회 ID 생성
    fn generate_opportunity_id(&self, opportunity: &Opportunity) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        
        format!(
            "{:?}_{:?}_{}_{}", 
            opportunity.opportunity_type,
            opportunity.strategy,
            opportunity.expected_profit,
            timestamp
        )
    }
    
    /// 전략별 TTL 가져오기
    fn get_ttl_for_strategy(&self, strategy: &StrategyType) -> u64 {
        match strategy {
            StrategyType::Sandwich => 60,      // 60초
            StrategyType::MicroArbitrage => 30,     // 30초
            StrategyType::Liquidation => 120,  // 120초
            _ => 60,
        }
    }
}

/// 큐 상태 정보
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub size: usize,
    pub stats: QueueStatsInfo,
}

/// 큐 통계 정보
#[derive(Debug, Clone)]
pub struct QueueStatsInfo {
    pub total_added: u64,
    pub total_executed: u64,
    pub total_expired: u64,
    pub total_rejected: u64,
    pub avg_priority_score: f64,
    pub max_priority_score: f64,
}

impl From<super::priority_queue::QueueStats> for QueueStatsInfo {
    fn from(stats: super::priority_queue::QueueStats) -> Self {
        Self {
            total_added: stats.total_added,
            total_executed: stats.total_executed,
            total_expired: stats.total_expired,
            total_rejected: stats.total_rejected,
            avg_priority_score: stats.avg_priority_score,
            max_priority_score: stats.max_priority_score,
        }
    }
}