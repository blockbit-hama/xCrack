use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use crate::types::{Opportunity, StrategyType};

/// 기회의 우선순위 정보
#[derive(Debug, Clone)]
pub struct OpportunityPriority {
    /// 기회 데이터
    pub opportunity: Opportunity,
    /// 우선순위 점수 (높을수록 우선)
    pub priority_score: f64,
    /// 수익성 점수
    pub profitability_score: f64,
    /// 리스크 점수 (낮을수록 좋음)
    pub risk_score: f64,
    /// 타이밍 점수
    pub timing_score: f64,
    /// 경쟁 점수
    pub competition_score: f64,
    /// 생성 시간
    pub created_at: u64,
    /// 만료 시간
    pub expires_at: u64,
    /// 실행 시도 횟수
    pub attempt_count: u32,
}

impl OpportunityPriority {
    /// 새로운 우선순위 기회 생성
    pub fn new(opportunity: Opportunity, ttl_seconds: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            opportunity,
            priority_score: 0.0,
            profitability_score: 0.0,
            risk_score: 0.0,
            timing_score: 0.0,
            competition_score: 0.0,
            created_at: now,
            expires_at: now + ttl_seconds,
            attempt_count: 0,
        }
    }
    
    /// 종합 우선순위 점수 계산
    pub fn calculate_priority_score(&mut self, weights: &ScoringWeights) {
        self.priority_score = 
            self.profitability_score * weights.profitability +
            (1.0 - self.risk_score) * weights.risk +  // 리스크는 낮을수록 좋음
            self.timing_score * weights.timing +
            self.competition_score * weights.competition;
    }
    
    /// 만료 여부 확인
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now >= self.expires_at
    }
    
    /// 나이 (초) 반환
    pub fn age_seconds(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.created_at
    }
    
    /// 남은 시간 (초) 반환
    pub fn time_remaining(&self) -> u64 {
        if self.is_expired() {
            return 0;
        }
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        self.expires_at - now
    }
}

/// 점수 계산 가중치
#[derive(Debug, Clone)]
pub struct ScoringWeights {
    pub profitability: f64,
    pub risk: f64,
    pub timing: f64,
    pub competition: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            profitability: 0.4,  // 40% 가중치
            risk: 0.3,           // 30% 가중치
            timing: 0.2,         // 20% 가중치
            competition: 0.1,    // 10% 가중치
        }
    }
}

/// 우선순위 큐 비교를 위한 Ord 구현
impl Ord for OpportunityPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        // priority_score가 높을수록 우선순위가 높음
        self.priority_score
            .partial_cmp(&other.priority_score)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for OpportunityPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for OpportunityPriority {
    fn eq(&self, other: &Self) -> bool {
        self.priority_score == other.priority_score
    }
}

impl Eq for OpportunityPriority {}

/// 기회 우선순위 큐
pub struct OpportunityQueue {
    /// 우선순위 힙
    heap: Arc<RwLock<BinaryHeap<OpportunityPriority>>>,
    /// 최대 큐 크기
    max_size: usize,
    /// 기본 TTL (초)
    default_ttl: u64,
    /// 점수 가중치
    scoring_weights: ScoringWeights,
    /// 통계
    stats: Arc<RwLock<QueueStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub total_added: u64,
    pub total_executed: u64,
    pub total_expired: u64,
    pub total_rejected: u64,
    pub avg_priority_score: f64,
    pub max_priority_score: f64,
    pub current_size: usize,
}

impl OpportunityQueue {
    /// 새로운 우선순위 큐 생성
    pub fn new(max_size: usize, default_ttl: u64) -> Self {
        Self {
            heap: Arc::new(RwLock::new(BinaryHeap::new())),
            max_size,
            default_ttl,
            scoring_weights: ScoringWeights::default(),
            stats: Arc::new(RwLock::new(QueueStats::default())),
        }
    }
    
    /// 커스텀 가중치로 큐 생성
    pub fn with_weights(max_size: usize, default_ttl: u64, weights: ScoringWeights) -> Self {
        Self {
            heap: Arc::new(RwLock::new(BinaryHeap::new())),
            max_size,
            default_ttl,
            scoring_weights: weights,
            stats: Arc::new(RwLock::new(QueueStats::default())),
        }
    }
    
    /// 기회 추가
    pub async fn push(&self, mut priority_opp: OpportunityPriority) -> Result<bool> {
        // 만료된 기회는 추가하지 않음
        if priority_opp.is_expired() {
            let mut stats = self.stats.write().await;
            stats.total_rejected += 1;
            return Ok(false);
        }
        
        // 우선순위 점수 계산
        priority_opp.calculate_priority_score(&self.scoring_weights);
        
        let mut heap = self.heap.write().await;
        
        // 큐가 가득 찬 경우
        if heap.len() >= self.max_size {
            // 가장 낮은 우선순위와 비교
            if let Some(lowest) = heap.peek() {
                if priority_opp.priority_score <= lowest.priority_score {
                    let mut stats = self.stats.write().await;
                    stats.total_rejected += 1;
                    return Ok(false);
                }
            }
            
            // 가장 낮은 우선순위 제거
            heap.pop();
        }
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_added += 1;
        stats.current_size = heap.len() + 1;
        
        // 평균 및 최대 점수 업데이트
        if stats.total_added == 1 {
            stats.avg_priority_score = priority_opp.priority_score;
            stats.max_priority_score = priority_opp.priority_score;
        } else {
            stats.avg_priority_score = 
                (stats.avg_priority_score * (stats.total_added - 1) as f64 + priority_opp.priority_score) 
                / stats.total_added as f64;
            stats.max_priority_score = stats.max_priority_score.max(priority_opp.priority_score);
        }
        
        debug!(
            "Adding opportunity to queue: type={:?}, profit={}, score={:.3}",
            priority_opp.opportunity.opportunity_type,
            priority_opp.opportunity.expected_profit,
            priority_opp.priority_score
        );
        
        heap.push(priority_opp);
        
        Ok(true)
    }
    
    /// 가장 높은 우선순위 기회 가져오기
    pub async fn pop(&self) -> Option<OpportunityPriority> {
        let mut heap = self.heap.write().await;
        
        // 만료된 기회들 제거
        self.remove_expired(&mut heap).await;
        
        if let Some(opp) = heap.pop() {
            let mut stats = self.stats.write().await;
            stats.total_executed += 1;
            stats.current_size = heap.len();
            
            debug!(
                "Popping opportunity from queue: type={:?}, score={:.3}, age={}s",
                opp.opportunity.opportunity_type,
                opp.priority_score,
                opp.age_seconds()
            );
            
            Some(opp)
        } else {
            None
        }
    }
    
    /// 최상위 기회 확인 (제거하지 않음)
    pub async fn peek(&self) -> Option<OpportunityPriority> {
        let heap = self.heap.read().await;
        heap.peek().cloned()
    }
    
    /// 여러 개의 최상위 기회 가져오기
    pub async fn pop_batch(&self, count: usize) -> Vec<OpportunityPriority> {
        let mut heap = self.heap.write().await;
        let mut batch = Vec::new();
        
        // 만료된 기회들 제거
        self.remove_expired(&mut heap).await;
        
        for _ in 0..count {
            if let Some(opp) = heap.pop() {
                batch.push(opp);
            } else {
                break;
            }
        }
        
        let mut stats = self.stats.write().await;
        stats.total_executed += batch.len() as u64;
        stats.current_size = heap.len();
        
        batch
    }
    
    /// 특정 전략 타입의 기회만 가져오기
    pub async fn pop_by_strategy(&self, strategy: StrategyType) -> Option<OpportunityPriority> {
        let mut heap = self.heap.write().await;
        
        // 만료된 기회들 제거
        self.remove_expired(&mut heap).await;
        
        // 모든 기회를 임시로 저장
        let mut temp = Vec::new();
        let mut found = None;
        
        while let Some(opp) = heap.pop() {
            if found.is_none() && opp.opportunity.strategy == strategy {
                found = Some(opp);
            } else {
                temp.push(opp);
            }
        }
        
        // 나머지 기회들 다시 추가
        for opp in temp {
            heap.push(opp);
        }
        
        if found.is_some() {
            let mut stats = self.stats.write().await;
            stats.total_executed += 1;
            stats.current_size = heap.len();
        }
        
        found
    }
    
    /// 만료된 기회들 제거
    async fn remove_expired(&self, heap: &mut BinaryHeap<OpportunityPriority>) {
        let mut temp = Vec::new();
        let mut expired_count = 0;
        
        while let Some(opp) = heap.pop() {
            if !opp.is_expired() {
                temp.push(opp);
            } else {
                expired_count += 1;
                debug!(
                    "Removing expired opportunity: type={:?}, age={}s",
                    opp.opportunity.opportunity_type,
                    opp.age_seconds()
                );
            }
        }
        
        for opp in temp {
            heap.push(opp);
        }
        
        if expired_count > 0 {
            let mut stats = self.stats.write().await;
            stats.total_expired += expired_count;
            stats.current_size = heap.len();
        }
    }
    
    /// 큐 크기 반환
    pub async fn size(&self) -> usize {
        let heap = self.heap.read().await;
        heap.len()
    }
    
    /// 큐가 비어있는지 확인
    pub async fn is_empty(&self) -> bool {
        let heap = self.heap.read().await;
        heap.is_empty()
    }
    
    /// 큐 비우기
    pub async fn clear(&self) {
        let mut heap = self.heap.write().await;
        heap.clear();
        
        let mut stats = self.stats.write().await;
        stats.current_size = 0;
    }
    
    /// 통계 반환
    pub async fn get_stats(&self) -> QueueStats {
        let stats = self.stats.read().await;
        stats.clone()
    }
    
    /// 큐의 모든 기회 반환 (우선순위 순)
    pub async fn get_all_sorted(&self) -> Vec<OpportunityPriority> {
        let heap = self.heap.read().await;
        let mut opportunities: Vec<OpportunityPriority> = heap.iter().cloned().collect();
        opportunities.sort_by(|a, b| b.cmp(a));  // 내림차순 정렬
        opportunities
    }
    
    /// 특정 조건을 만족하는 기회 필터링
    pub async fn filter<F>(&self, predicate: F) -> Vec<OpportunityPriority>
    where
        F: Fn(&OpportunityPriority) -> bool,
    {
        let heap = self.heap.read().await;
        heap.iter()
            .filter(|opp| predicate(opp))
            .cloned()
            .collect()
    }
    
    /// 큐 상태 요약
    pub async fn summary(&self) -> String {
        let stats = self.get_stats().await;
        let heap = self.heap.read().await;
        
        let mut strategy_counts = std::collections::HashMap::new();
        for opp in heap.iter() {
            *strategy_counts.entry(opp.opportunity.strategy).or_insert(0) += 1;
        }
        
        format!(
            "OpportunityQueue Summary:\n\
             - Current size: {}/{}\n\
             - Total added: {}\n\
             - Total executed: {}\n\
             - Total expired: {}\n\
             - Total rejected: {}\n\
             - Avg priority score: {:.3}\n\
             - Max priority score: {:.3}\n\
             - Strategy distribution: {:?}",
            stats.current_size,
            self.max_size,
            stats.total_added,
            stats.total_executed,
            stats.total_expired,
            stats.total_rejected,
            stats.avg_priority_score,
            stats.max_priority_score,
            strategy_counts
        )
    }
}