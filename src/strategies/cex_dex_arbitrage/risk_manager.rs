//! 위험 관리 시스템
//! 
//! 이 모듈은 마이크로아비트리지 전략의 위험을 관리하고
//! 손실을 제한하는 시스템을 제공합니다.

use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};
use ethers::types::U256;
use chrono::{Utc, Duration};
use rust_decimal::Decimal;

use crate::config::Config;
use super::types::{
    MicroArbitrageOpportunity, RiskManagementConfig, RiskMetrics,
    MicroArbitrageStats, ArbitrageExecutionResult
};

/// 위험 관리자
pub struct RiskManager {
    config: Arc<Config>,
    risk_config: RiskManagementConfig,
    current_exposure: Arc<RwLock<U256>>,
    daily_volume: Arc<RwLock<U256>>,
    daily_pnl: Arc<RwLock<U256>>,
    max_drawdown: Arc<RwLock<U256>>,
    position_history: Arc<RwLock<Vec<PositionRecord>>>,
    risk_metrics: Arc<RwLock<RiskMetrics>>,
}

/// 포지션 기록
#[derive(Debug, Clone)]
pub struct PositionRecord {
    pub opportunity_id: String,
    pub symbol: String,
    pub amount: U256,
    pub entry_time: chrono::DateTime<Utc>,
    pub exit_time: Option<chrono::DateTime<Utc>>,
    pub pnl: Option<U256>,
    pub status: PositionStatus,
}

/// 포지션 상태
#[derive(Debug, Clone, PartialEq)]
pub enum PositionStatus {
    Open,
    Closed,
    Stopped,
}

impl RiskManager {
    /// 새로운 위험 관리자 생성
    pub fn new(config: Arc<Config>) -> Self {
        let micro_config = &config.strategies.micro_arbitrage;
        
        let risk_config = RiskManagementConfig {
            max_position_size: micro_config.risk_limit_per_trade,
            max_daily_volume: micro_config.daily_volume_limit,
            max_daily_loss: U256::from(1000), // 1000 USD 기본값
            max_concurrent_trades: micro_config.max_concurrent_trades,
            min_profit_threshold: U256::from(10), // 10 USD 기본값
            max_slippage_percentage: 0.01, // 1% 기본값
            stop_loss_percentage: 0.05, // 5% 기본값
            position_timeout_seconds: 300, // 5분 기본값
        };
        
        Self {
            config,
            risk_config,
            current_exposure: Arc::new(RwLock::new(U256::zero())),
            daily_volume: Arc::new(RwLock::new(U256::zero())),
            daily_pnl: Arc::new(RwLock::new(U256::zero())),
            max_drawdown: Arc::new(RwLock::new(U256::zero())),
            position_history: Arc::new(RwLock::new(Vec::new())),
            risk_metrics: Arc::new(RwLock::new(RiskMetrics::default())),
        }
    }
    
    /// 아비트리지 기회 위험 평가
    pub async fn assess_opportunity_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskAssessment> {
        let mut risk_score = 0.0;
        let mut risk_factors = Vec::new();
        
        // 1. 포지션 크기 위험
        let position_risk = self.assess_position_size_risk(opportunity).await?;
        risk_score += position_risk.score;
        risk_factors.push(position_risk);
        
        // 2. 일일 거래량 위험
        let volume_risk = self.assess_volume_risk(opportunity).await?;
        risk_score += volume_risk.score;
        risk_factors.push(volume_risk);
        
        // 3. 수익성 위험
        let profitability_risk = self.assess_profitability_risk(opportunity).await?;
        risk_score += profitability_risk.score;
        risk_factors.push(profitability_risk);
        
        // 4. 시장 위험
        let market_risk = self.assess_market_risk(opportunity).await?;
        risk_score += market_risk.score;
        risk_factors.push(market_risk);
        
        // 5. 유동성 위험
        let liquidity_risk = self.assess_liquidity_risk(opportunity).await?;
        risk_score += liquidity_risk.score;
        risk_factors.push(liquidity_risk);
        
        // 6. 집중도 위험
        let concentration_risk = self.assess_concentration_risk(opportunity).await?;
        risk_score += concentration_risk.score;
        risk_factors.push(concentration_risk);
        
        // 전체 위험 등급 결정
        let risk_grade = self.determine_risk_grade(risk_score);
        
        // 실행 권장사항 결정
        let recommendation = self.determine_recommendation(risk_score, opportunity).await?;
        
        Ok(RiskAssessment {
            opportunity_id: opportunity.id.clone(),
            risk_score,
            risk_grade,
            risk_factors,
            recommendation,
            max_position_size: self.calculate_max_position_size(opportunity).await?,
            stop_loss_level: self.calculate_stop_loss_level(opportunity).await?,
            created_at: Utc::now(),
        })
    }
    
    /// 포지션 크기 위험 평가
    async fn assess_position_size_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskFactor> {
        let position_size = opportunity.buy_amount;
        let max_position = self.risk_config.max_position_size;
        
        let size_ratio = position_size.as_u128() as f64 / max_position.as_u128() as f64;
        
        let (score, description) = if size_ratio > 1.0 {
            (1.0, "포지션 크기가 한도를 초과합니다".to_string())
        } else if size_ratio > 0.8 {
            (0.8, "포지션 크기가 한도에 근접합니다".to_string())
        } else if size_ratio > 0.5 {
            (0.5, "포지션 크기가 중간 수준입니다".to_string())
        } else {
            (0.2, "포지션 크기가 안전한 수준입니다".to_string())
        };
        
        Ok(RiskFactor {
            name: "포지션 크기".to_string(),
            score,
            description,
            weight: 0.25,
        })
    }
    
    /// 일일 거래량 위험 평가
    async fn assess_volume_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskFactor> {
        let current_volume = *self.daily_volume.read().await;
        let max_volume = self.risk_config.max_daily_volume;
        let new_volume = current_volume + opportunity.buy_amount;
        
        let volume_ratio = new_volume.as_u128() as f64 / max_volume.as_u128() as f64;
        
        let (score, description) = if volume_ratio > 1.0 {
            (1.0, "일일 거래량 한도를 초과합니다".to_string())
        } else if volume_ratio > 0.9 {
            (0.8, "일일 거래량 한도에 근접합니다".to_string())
        } else if volume_ratio > 0.7 {
            (0.6, "일일 거래량이 높은 수준입니다".to_string())
        } else {
            (0.2, "일일 거래량이 안전한 수준입니다".to_string())
        };
        
        Ok(RiskFactor {
            name: "일일 거래량".to_string(),
            score,
            description,
            weight: 0.2,
        })
    }
    
    /// 수익성 위험 평가
    async fn assess_profitability_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskFactor> {
        let profit_percentage = opportunity.profit_percentage;
        let min_threshold = self.risk_config.min_profit_threshold.as_u128() as f64 / 1e18;
        
        let (score, description) = if profit_percentage < 0.001 { // 0.1% 미만
            (1.0, "수익률이 매우 낮습니다".to_string())
        } else if profit_percentage < 0.005 { // 0.5% 미만
            (0.7, "수익률이 낮습니다".to_string())
        } else if profit_percentage < 0.01 { // 1% 미만
            (0.4, "수익률이 보통 수준입니다".to_string())
        } else {
            (0.1, "수익률이 양호합니다".to_string())
        };
        
        Ok(RiskFactor {
            name: "수익성".to_string(),
            score,
            description,
            weight: 0.2,
        })
    }
    
    /// 시장 위험 평가
    async fn assess_market_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskFactor> {
        // 실제 구현에서는 시장 변동성, 상관관계 등을 분석
        // 여기서는 간단한 추정치 사용
        
        let spread_ratio = opportunity.price_spread.to_f64().unwrap_or(0.0) / 
                          opportunity.buy_price.to_f64().unwrap_or(1.0);
        
        let (score, description) = if spread_ratio > 0.05 { // 5% 이상
            (0.9, "시장 변동성이 높습니다".to_string())
        } else if spread_ratio > 0.02 { // 2% 이상
            (0.6, "시장 변동성이 중간 수준입니다".to_string())
        } else {
            (0.2, "시장 변동성이 낮습니다".to_string())
        };
        
        Ok(RiskFactor {
            name: "시장 위험".to_string(),
            score,
            description,
            weight: 0.15,
        })
    }
    
    /// 유동성 위험 평가
    async fn assess_liquidity_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskFactor> {
        // 실제 구현에서는 오더북 깊이, 유동성 지표 등을 분석
        // 여기서는 간단한 추정치 사용
        
        let amount_eth = opportunity.buy_amount.as_u128() as f64 / 1e18;
        
        let (score, description) = if amount_eth > 100.0 { // 100 ETH 이상
            (0.9, "거래량이 크고 유동성 위험이 높습니다".to_string())
        } else if amount_eth > 10.0 { // 10 ETH 이상
            (0.6, "거래량이 중간이고 유동성 위험이 보통입니다".to_string())
        } else {
            (0.2, "거래량이 작고 유동성 위험이 낮습니다".to_string())
        };
        
        Ok(RiskFactor {
            name: "유동성 위험".to_string(),
            score,
            description,
            weight: 0.1,
        })
    }
    
    /// 집중도 위험 평가
    async fn assess_concentration_risk(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskFactor> {
        let position_history = self.position_history.read().await;
        let recent_positions: Vec<_> = position_history.iter()
            .filter(|p| p.entry_time > Utc::now() - Duration::hours(24))
            .collect();
        
        let symbol_count = recent_positions.iter()
            .filter(|p| p.symbol == opportunity.token_symbol)
            .count();
        
        let total_count = recent_positions.len();
        let concentration_ratio = if total_count > 0 {
            symbol_count as f64 / total_count as f64
        } else {
            0.0
        };
        
        let (score, description) = if concentration_ratio > 0.5 {
            (0.8, "특정 심볼에 집중도가 높습니다".to_string())
        } else if concentration_ratio > 0.3 {
            (0.5, "특정 심볼에 어느 정도 집중되어 있습니다".to_string())
        } else {
            (0.2, "심볼 집중도가 적절합니다".to_string())
        };
        
        Ok(RiskFactor {
            name: "집중도 위험".to_string(),
            score,
            description,
            weight: 0.1,
        })
    }
    
    /// 위험 등급 결정
    fn determine_risk_grade(&self, risk_score: f64) -> RiskGrade {
        if risk_score >= 0.8 {
            RiskGrade::High
        } else if risk_score >= 0.6 {
            RiskGrade::Medium
        } else if risk_score >= 0.4 {
            RiskGrade::Low
        } else {
            RiskGrade::VeryLow
        }
    }
    
    /// 실행 권장사항 결정
    async fn determine_recommendation(
        &self,
        risk_score: f64,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<RiskRecommendation> {
        if risk_score >= 0.8 {
            Ok(RiskRecommendation::Reject)
        } else if risk_score >= 0.6 {
            Ok(RiskRecommendation::ReduceSize)
        } else if risk_score >= 0.4 {
            Ok(RiskRecommendation::ProceedWithCaution)
        } else {
            Ok(RiskRecommendation::Proceed)
        }
    }
    
    /// 최대 포지션 크기 계산
    async fn calculate_max_position_size(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<U256> {
        let base_size = opportunity.buy_amount;
        let max_size = self.risk_config.max_position_size;
        
        // 위험도에 따라 크기 조정
        let risk_adjustment = 0.8; // 80%로 제한
        let adjusted_size = base_size * U256::from((risk_adjustment * 100.0) as u64) / U256::from(100);
        
        Ok(std::cmp::min(adjusted_size, max_size))
    }
    
    /// 손절매 레벨 계산
    async fn calculate_stop_loss_level(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<U256> {
        let entry_price = opportunity.buy_price;
        let stop_loss_pct = self.risk_config.stop_loss_percentage;
        
        let stop_loss_price = entry_price * 
            Decimal::from_f64_retain(1.0 - stop_loss_pct).unwrap_or_default();
        
        Ok(U256::from((stop_loss_price.to_f64().unwrap_or(0.0) * 1e18) as u64))
    }
    
    /// 포지션 열기
    pub async fn open_position(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<()> {
        let position = PositionRecord {
            opportunity_id: opportunity.id.clone(),
            symbol: opportunity.token_symbol.clone(),
            amount: opportunity.buy_amount,
            entry_time: Utc::now(),
            exit_time: None,
            pnl: None,
            status: PositionStatus::Open,
        };
        
        // 포지션 기록 추가
        {
            let mut history = self.position_history.write().await;
            history.push(position);
        }
        
        // 노출도 업데이트
        {
            let mut exposure = self.current_exposure.write().await;
            *exposure += opportunity.buy_amount;
        }
        
        // 일일 거래량 업데이트
        {
            let mut volume = self.daily_volume.write().await;
            *volume += opportunity.buy_amount;
        }
        
        debug!("📊 포지션 열기: {} ({} {})", 
               opportunity.id, opportunity.buy_amount, opportunity.token_symbol);
        
        Ok(())
    }
    
    /// 포지션 닫기
    pub async fn close_position(
        &self,
        opportunity_id: &str,
        pnl: U256,
    ) -> Result<()> {
        let mut history = self.position_history.write().await;
        
        if let Some(position) = history.iter_mut().find(|p| p.opportunity_id == opportunity_id) {
            position.exit_time = Some(Utc::now());
            position.pnl = Some(pnl);
            position.status = PositionStatus::Closed;
            
            // 노출도 업데이트
            {
                let mut exposure = self.current_exposure.write().await;
                if *exposure >= position.amount {
                    *exposure -= position.amount;
                }
            }
            
            // 일일 PnL 업데이트
            {
                let mut daily_pnl = self.daily_pnl.write().await;
                *daily_pnl += pnl;
            }
            
            debug!("📊 포지션 닫기: {} (PnL: {})", opportunity_id, pnl);
        }
        
        Ok(())
    }
    
    /// 위험 메트릭 업데이트
    pub async fn update_risk_metrics(&self) -> Result<()> {
        let position_history = self.position_history.read().await;
        let current_exposure = *self.current_exposure.read().await;
        let daily_pnl = *self.daily_pnl.read().await;
        
        // 최근 30일 데이터로 계산
        let recent_positions: Vec<_> = position_history.iter()
            .filter(|p| p.entry_time > Utc::now() - Duration::days(30))
            .collect();
        
        let total_trades = recent_positions.len() as f64;
        let winning_trades = recent_positions.iter()
            .filter(|p| p.pnl.map_or(false, |pnl| pnl > U256::zero()))
            .count() as f64;
        
        let win_rate = if total_trades > 0.0 {
            winning_trades / total_trades
        } else {
            0.0
        };
        
        let total_pnl: U256 = recent_positions.iter()
            .filter_map(|p| p.pnl)
            .sum();
        
        let avg_win = if winning_trades > 0.0 {
            recent_positions.iter()
                .filter_map(|p| p.pnl)
                .filter(|pnl| *pnl > U256::zero())
                .sum::<U256>() / U256::from(winning_trades as u64)
        } else {
            U256::zero()
        };
        
        let losing_trades = total_trades - winning_trades;
        let avg_loss = if losing_trades > 0.0 {
            recent_positions.iter()
                .filter_map(|p| p.pnl)
                .filter(|pnl| *pnl < U256::zero())
                .map(|pnl| -pnl) // 절댓값
                .sum::<U256>() / U256::from(losing_trades as u64)
        } else {
            U256::zero()
        };
        
        let profit_factor = if avg_loss > U256::zero() {
            avg_win.as_u128() as f64 / avg_loss.as_u128() as f64
        } else {
            0.0
        };
        
        // 샤프 비율 계산 (간단한 버전)
        let sharpe_ratio = if total_trades > 1.0 {
            let mean_return = total_pnl.as_u128() as f64 / total_trades;
            // 실제로는 표준편차를 계산해야 함
            mean_return / 100.0 // 임시값
        } else {
            0.0
        };
        
        let risk_metrics = RiskMetrics {
            current_exposure,
            daily_pnl,
            max_drawdown: *self.max_drawdown.read().await,
            var_95: U256::zero(), // VaR 계산은 별도 구현 필요
            sharpe_ratio,
            win_rate,
            avg_win,
            avg_loss,
            profit_factor,
        };
        
        {
            let mut metrics = self.risk_metrics.write().await;
            *metrics = risk_metrics;
        }
        
        debug!("📊 위험 메트릭 업데이트 완료");
        Ok(())
    }
    
    /// 위험 메트릭 가져오기
    pub async fn get_risk_metrics(&self) -> RiskMetrics {
        self.risk_metrics.read().await.clone()
    }
    
    /// 일일 리셋
    pub async fn daily_reset(&self) -> Result<()> {
        {
            let mut volume = self.daily_volume.write().await;
            *volume = U256::zero();
        }
        
        {
            let mut pnl = self.daily_pnl.write().await;
            *pnl = U256::zero();
        }
        
        info!("🔄 일일 위험 관리 리셋 완료");
        Ok(())
    }
}

/// 위험 평가 결과
#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub opportunity_id: String,
    pub risk_score: f64,
    pub risk_grade: RiskGrade,
    pub risk_factors: Vec<RiskFactor>,
    pub recommendation: RiskRecommendation,
    pub max_position_size: U256,
    pub stop_loss_level: U256,
    pub created_at: chrono::DateTime<Utc>,
}

/// 위험 요소
#[derive(Debug, Clone)]
pub struct RiskFactor {
    pub name: String,
    pub score: f64,
    pub description: String,
    pub weight: f64,
}

/// 위험 등급
#[derive(Debug, Clone, PartialEq)]
pub enum RiskGrade {
    VeryLow,
    Low,
    Medium,
    High,
}

/// 위험 권장사항
#[derive(Debug, Clone, PartialEq)]
pub enum RiskRecommendation {
    Proceed,              // 진행
    ProceedWithCaution,   // 주의하며 진행
    ReduceSize,           // 크기 축소
    Reject,               // 거부
}