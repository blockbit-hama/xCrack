use alloy::primitives::U256;
use tracing::debug;

use crate::types::{Opportunity, OpportunityType, StrategyType};
use super::priority_queue::{OpportunityPriority, ScoringWeights};

/// 기회 점수 계산기
pub struct OpportunityScorer {
    /// 최소 수익 임계값 (ETH)
    min_profit_threshold: U256,
    /// 최대 리스크 허용치
    max_risk_tolerance: f64,
    /// 최대 가스 가격 (wei)
    max_gas_price: U256,
    /// 네트워크 혼잡도
    network_congestion: f64,
    /// 경쟁자 수
    competitor_count: u32,
}

impl OpportunityScorer {
    /// 새로운 점수 계산기 생성
    pub fn new(min_profit_threshold: U256, max_risk_tolerance: f64) -> Self {
        Self {
            min_profit_threshold,
            max_risk_tolerance,
            max_gas_price: U256::from(200_000_000_000u64) * U256::from(1_000_000_000u64), // 200 Gwei
            network_congestion: 0.5,
            competitor_count: 10,
        }
    }
    
    /// 네트워크 상태 업데이트
    pub fn update_network_state(&mut self, congestion: f64, competitors: u32) {
        self.network_congestion = congestion.clamp(0.0, 1.0);
        self.competitor_count = competitors;
    }
    
    /// 기회에 대한 종합 점수 계산
    pub fn score_opportunity(&self, opportunity: &Opportunity, ttl_seconds: u64) -> OpportunityPriority {
        let mut priority = OpportunityPriority::new(opportunity.clone(), ttl_seconds);
        
        // 각 점수 계산
        priority.profitability_score = self.calculate_profitability_score(opportunity);
        priority.risk_score = self.calculate_risk_score(opportunity);
        priority.timing_score = self.calculate_timing_score(opportunity);
        priority.competition_score = self.calculate_competition_score(opportunity);
        
        // 종합 점수 계산 (기본 가중치 사용)
        let weights = ScoringWeights::default();
        priority.calculate_priority_score(&weights);
        
        debug!(
            "Scored opportunity: type={:?}, profit_score={:.3}, risk_score={:.3}, \
             timing_score={:.3}, competition_score={:.3}, total_score={:.3}",
            opportunity.opportunity_type,
            priority.profitability_score,
            priority.risk_score,
            priority.timing_score,
            priority.competition_score,
            priority.priority_score
        );
        
        priority
    }
    
    /// 수익성 점수 계산 (0.0 ~ 1.0)
    fn calculate_profitability_score(&self, opportunity: &Opportunity) -> f64 {
        // 순수익 계산
        let approx_gas_cost = U256::from(opportunity.gas_estimate) * U256::from(20_000_000_000u64);
        let net_profit = if opportunity.expected_profit > approx_gas_cost {
            opportunity.expected_profit - approx_gas_cost
        } else {
            return 0.0;
        };
        
        // 최소 수익 대비 비율
        if net_profit < self.min_profit_threshold {
            return 0.0;
        }
        
        // 로그 스케일로 점수 계산 (수익이 클수록 점수 증가, 최대 1.0)
        let profit_ratio = net_profit.to::<u128>() as f64 / self.min_profit_threshold.to::<u128>() as f64;
        let score = (profit_ratio.ln() / 10.0).min(1.0).max(0.0);
        
        // 전략별 가중치 적용
        let strategy_weight = match opportunity.strategy {
            StrategyType::Sandwich => 1.0,      // 샌드위치는 높은 수익
            StrategyType::MicroArbitrage => 0.9,     // 아비트라지는 중간 수익
            StrategyType::Liquidation => 0.8,   // 청산은 안정적 수익
            _ => 0.7,
        };
        
        score * strategy_weight
    }
    
    /// 리스크 점수 계산 (0.0 ~ 1.0, 높을수록 위험)
    fn calculate_risk_score(&self, opportunity: &Opportunity) -> f64 {
        let mut risk_score = 0.0;
        
        // 신뢰도 기반 리스크 (신뢰도가 낮을수록 위험)
        risk_score += (1.0 - opportunity.confidence) * 0.3;
        
        // 가스 비용 리스크
        let approx_gas_cost = U256::from(opportunity.gas_estimate) * U256::from(20_000_000_000u64);
        let gas_ratio = approx_gas_cost.to::<u128>() as f64 
            / opportunity.expected_profit.to::<u128>().max(1) as f64;
        risk_score += gas_ratio.min(1.0) * 0.2;
        
        // 전략별 기본 리스크
        let strategy_risk = match opportunity.strategy {
            StrategyType::Sandwich => 0.7,      // 샌드위치는 높은 리스크
            StrategyType::Liquidation => 0.3,   // 청산은 낮은 리스크
            StrategyType::MicroArbitrage => 0.5,     // 아비트라지는 중간 리스크
            _ => 0.6,
        };
        risk_score += strategy_risk * 0.3;
        
        // 시장 변동성 리스크 (임시로 고정값)
        let volatility_risk = 0.4;
        risk_score += volatility_risk * 0.2;
        
        risk_score.min(1.0)
    }
    
    /// 타이밍 점수 계산 (0.0 ~ 1.0)
    fn calculate_timing_score(&self, opportunity: &Opportunity) -> f64 {
        let mut timing_score = 1.0;
        
        // 네트워크 혼잡도 영향
        timing_score *= 1.0 - self.network_congestion * 0.5;
        
        // 블록 번호 기반 긴급도
        if opportunity.expiry_block > 0 {
            let current_block = 0u64; // TODO: pass actual block
            let blocks_remaining = opportunity.expiry_block.saturating_sub(current_block);
            
            if blocks_remaining == 0 {
                return 0.0;  // 이미 데드라인 지남
            }
            
            // 남은 블록이 적을수록 점수 감소
            if blocks_remaining < 5 {
                timing_score *= blocks_remaining as f64 / 5.0;
            }
        }
        
        // 전략별 타이밍 중요도
        let timing_importance = match opportunity.strategy {
            StrategyType::Sandwich => 1.0,      // 샌드위치는 타이밍이 매우 중요
            StrategyType::MicroArbitrage => 0.9,     // 아비트라지도 타이밍 중요
            StrategyType::Liquidation => 0.6,   // 청산은 상대적으로 덜 중요
            _ => 0.7,
        };
        
        timing_score * timing_importance
    }
    
    /// 경쟁 점수 계산 (0.0 ~ 1.0, 낮을수록 경쟁 심함)
    fn calculate_competition_score(&self, opportunity: &Opportunity) -> f64 {
        // 경쟁자 수에 따른 점수
        let competition_factor = if self.competitor_count == 0 {
            1.0
        } else {
            1.0 / (1.0 + self.competitor_count as f64 * 0.1)
        };
        
        // 기회 타입별 경쟁 정도
        let type_competition = match opportunity.opportunity_type {
            OpportunityType::Sandwich => 0.3,      // 샌드위치는 경쟁 심함
            OpportunityType::MicroArbitrage => 0.5,     // 아비트라지는 중간 경쟁
            OpportunityType::Liquidation => 0.7,   // 청산은 경쟁 덜함
            _ => 0.5,
        };
        
        // 수익 크기에 따른 경쟁 (큰 수익일수록 경쟁 심함)
        let profit_competition = if opportunity.expected_profit > U256::from(10).pow(U256::from(18)) {
            0.3  // 1 ETH 이상은 매우 경쟁적
        } else if opportunity.expected_profit > U256::from(10).pow(U256::from(17)) {
            0.5  // 0.1 ETH 이상은 중간 경쟁
        } else {
            0.8  // 작은 수익은 경쟁 덜함
        };
        
        competition_factor * type_competition * profit_competition
    }
    
    /// 배치 점수 계산
    pub fn score_batch(&self, opportunities: &[Opportunity], ttl_seconds: u64) -> Vec<OpportunityPriority> {
        opportunities.iter()
            .map(|opp| self.score_opportunity(opp, ttl_seconds))
            .collect()
    }
    
    /// 동적 가중치 계산 (시장 상황에 따라)
    pub fn calculate_dynamic_weights(&self) -> ScoringWeights {
        let mut weights = ScoringWeights::default();
        
        // 네트워크가 혼잡할 때는 수익성 중시
        if self.network_congestion > 0.7 {
            weights.profitability = 0.5;
            weights.risk = 0.2;
            weights.timing = 0.2;
            weights.competition = 0.1;
        }
        // 경쟁이 심할 때는 타이밍과 리스크 중시
        else if self.competitor_count > 20 {
            weights.profitability = 0.3;
            weights.risk = 0.3;
            weights.timing = 0.3;
            weights.competition = 0.1;
        }
        // 정상 상황
        else {
            weights.profitability = 0.4;
            weights.risk = 0.3;
            weights.timing = 0.2;
            weights.competition = 0.1;
        }
        
        weights
    }
}

/// 기회 점수 비교기
pub struct OpportunityComparator;

impl OpportunityComparator {
    /// 두 기회 비교
    pub fn compare(a: &OpportunityPriority, b: &OpportunityPriority) -> std::cmp::Ordering {
        // 우선순위 점수로 비교
        let score_cmp = b.priority_score.partial_cmp(&a.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal);
        
        if score_cmp != std::cmp::Ordering::Equal {
            return score_cmp;
        }
        
        // 점수가 같으면 수익성으로 비교
        let profit_cmp = b.profitability_score.partial_cmp(&a.profitability_score)
            .unwrap_or(std::cmp::Ordering::Equal);
        
        if profit_cmp != std::cmp::Ordering::Equal {
            return profit_cmp;
        }
        
        // 그래도 같으면 리스크로 비교 (낮을수록 좋음)
        a.risk_score.partial_cmp(&b.risk_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
    
    /// 최적 기회 선택
    pub fn select_best(opportunities: Vec<OpportunityPriority>) -> Option<OpportunityPriority> {
        opportunities.into_iter()
            .filter(|o| !o.is_expired())
            .max_by(|a, b| Self::compare(a, b))
    }
    
    /// 상위 N개 기회 선택
    pub fn select_top_n(mut opportunities: Vec<OpportunityPriority>, n: usize) -> Vec<OpportunityPriority> {
        opportunities.retain(|o| !o.is_expired());
        opportunities.sort_by(|a, b| Self::compare(a, b));
        opportunities.into_iter().take(n).collect()
    }
}