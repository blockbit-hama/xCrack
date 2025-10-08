//! 성능 추적 및 통계 시스템
//! 
//! 이 모듈은 마이크로아비트리지 전략의 성능을 추적하고
//! 상세한 통계를 제공합니다.

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};
use ethers::types::U256;
use chrono::{Utc, Duration};

use crate::config::Config;
use super::types::{
    MicroArbitrageStats, ArbitrageExecutionResult, MicroArbitrageOpportunity,
    RiskMetrics, ExecutionPriority
};

/// 성능 추적기
pub struct PerformanceTracker {
    config: Arc<Config>,
    stats: Arc<RwLock<MicroArbitrageStats>>,
    execution_history: Arc<RwLock<Vec<ArbitrageExecutionResult>>>,
    opportunity_history: Arc<RwLock<Vec<MicroArbitrageOpportunity>>>,
    risk_metrics: Arc<RwLock<RiskMetrics>>,
    start_time: chrono::DateTime<Utc>,
}

impl PerformanceTracker {
    /// 새로운 성능 추적기 생성
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(MicroArbitrageStats::default())),
            execution_history: Arc::new(RwLock::new(Vec::new())),
            opportunity_history: Arc::new(RwLock::new(Vec::new())),
            risk_metrics: Arc::new(RwLock::new(RiskMetrics::default())),
            start_time: Utc::now(),
        }
    }
    
    /// 기회 탐지 기록
    pub async fn record_opportunity(&self, opportunity: MicroArbitrageOpportunity) {
        let mut history = self.opportunity_history.write().await;
        history.push(opportunity);
        
        // 최근 10000개만 유지
        if history.len() > 10000 {
            history.drain(0..history.len() - 10000);
        }
        
        // 통계 업데이트
        self.update_opportunity_stats().await;
    }
    
    /// 실행 결과 기록
    pub async fn record_execution(&self, result: ArbitrageExecutionResult) {
        let mut history = self.execution_history.write().await;
        history.push(result);
        
        // 최근 10000개만 유지
        if history.len() > 10000 {
            history.drain(0..history.len() - 10000);
        }
        
        // 통계 업데이트
        self.update_execution_stats().await;
    }
    
    /// 기회 통계 업데이트
    async fn update_opportunity_stats(&self) {
        let mut stats = self.stats.write().await;
        let opportunity_history = self.opportunity_history.read().await;
        
        stats.total_opportunities = opportunity_history.len() as u64;
        stats.last_updated = Utc::now();
    }
    
    /// 실행 통계 업데이트
    async fn update_execution_stats(&self) {
        let mut stats = self.stats.write().await;
        let execution_history = self.execution_history.read().await;
        
        let total_executions = execution_history.len() as u64;
        let successful_executions = execution_history.iter()
            .filter(|r| r.success)
            .count() as u64;
        let failed_executions = total_executions - successful_executions;
        
        stats.executed_trades = total_executions;
        stats.successful_trades = successful_executions;
        stats.failed_trades = failed_executions;
        
        // 성공률 계산
        stats.success_rate = if total_executions > 0 {
            successful_executions as f64 / total_executions as f64
        } else {
            0.0
        };
        
        // 수익 통계
        let total_profit: U256 = execution_history.iter()
            .filter_map(|r| r.actual_profit)
            .sum();
        let total_fees: U256 = execution_history.iter()
            .map(|r| r.fees_paid)
            .sum();
        
        stats.total_profit = total_profit;
        stats.total_fees = total_fees;
        
        // 평균 수익 계산
        stats.avg_profit_per_trade = if successful_executions > 0 {
            total_profit / U256::from(successful_executions)
        } else {
            U256::zero()
        };
        
        // 평균 실행 시간 계산
        let total_execution_time: u64 = execution_history.iter()
            .map(|r| r.execution_time_ms)
            .sum();
        stats.avg_execution_time_ms = if total_executions > 0 {
            total_execution_time as f64 / total_executions as f64
        } else {
            0.0
        };
        
        // 수익률 계산
        let total_volume: U256 = execution_history.iter()
            .map(|r| r.actual_profit.unwrap_or(U256::zero()))
            .sum();
        stats.profit_rate = if total_volume > U256::zero() {
            (total_profit.as_u128() as f64 / total_volume.as_u128() as f64) * 100.0
        } else {
            0.0
        };
        
        // 가동률 계산
        let uptime_seconds = (Utc::now() - self.start_time).num_seconds() as u64;
        stats.uptime_percentage = 100.0; // 실제로는 다운타임 계산 필요
        
        stats.last_updated = Utc::now();
    }
    
    /// 위험 메트릭 업데이트
    pub async fn update_risk_metrics(&self, risk_metrics: RiskMetrics) {
        let mut metrics = self.risk_metrics.write().await;
        *metrics = risk_metrics;
    }
    
    /// 성능 통계 가져오기
    pub async fn get_stats(&self) -> MicroArbitrageStats {
        self.stats.read().await.clone()
    }
    
    /// 위험 메트릭 가져오기
    pub async fn get_risk_metrics(&self) -> RiskMetrics {
        self.risk_metrics.read().await.clone()
    }
    
    /// 상세 성능 분석
    pub async fn get_detailed_analysis(&self) -> DetailedPerformanceAnalysis {
        let stats = self.get_stats().await;
        let risk_metrics = self.get_risk_metrics().await;
        let execution_history = self.execution_history.read().await;
        let opportunity_history = self.opportunity_history.read().await;
        
        // 시간대별 분석
        let hourly_analysis = self.analyze_hourly_performance(&execution_history).await;
        
        // 거래소별 분석
        let exchange_analysis = self.analyze_exchange_performance(&execution_history).await;
        
        // 심볼별 분석
        let symbol_analysis = self.analyze_symbol_performance(&execution_history).await;
        
        // 수익성 분석
        let profitability_analysis = self.analyze_profitability(&execution_history).await;
        
        // 위험 분석
        let risk_analysis = self.analyze_risk_metrics(&risk_metrics).await;
        
        DetailedPerformanceAnalysis {
            overall_stats: stats,
            risk_metrics,
            hourly_analysis,
            exchange_analysis,
            symbol_analysis,
            profitability_analysis,
            risk_analysis,
            total_opportunities: opportunity_history.len(),
            analysis_timestamp: Utc::now(),
        }
    }
    
    /// 시간대별 성능 분석
    async fn analyze_hourly_performance(
        &self,
        execution_history: &[ArbitrageExecutionResult],
    ) -> HourlyAnalysis {
        let mut hourly_stats: std::collections::HashMap<u8, HourlyStats> = std::collections::HashMap::new();
        
        for result in execution_history {
            let hour = result.created_at.hour();
            let stats = hourly_stats.entry(hour).or_insert_with(|| HourlyStats {
                hour,
                total_trades: 0,
                successful_trades: 0,
                total_profit: U256::zero(),
                avg_execution_time: 0.0,
            });
            
            stats.total_trades += 1;
            if result.success {
                stats.successful_trades += 1;
                if let Some(profit) = result.actual_profit {
                    stats.total_profit += profit;
                }
            }
            stats.avg_execution_time = (stats.avg_execution_time * (stats.total_trades - 1) as f64 + 
                                      result.execution_time_ms as f64) / stats.total_trades as f64;
        }
        
        let best_hour = hourly_stats.values()
            .max_by_key(|s| s.total_profit.as_u128())
            .map(|s| s.hour)
            .unwrap_or(0);
        
        HourlyAnalysis {
            hourly_stats: hourly_stats.into_values().collect(),
            best_hour,
            worst_hour: 0, // 계산 필요
        }
    }
    
    /// 거래소별 성능 분석
    async fn analyze_exchange_performance(
        &self,
        execution_history: &[ArbitrageExecutionResult],
    ) -> ExchangeAnalysis {
        let mut exchange_stats: std::collections::HashMap<String, ExchangeStats> = std::collections::HashMap::new();
        
        for result in execution_history {
            // 실제로는 거래소 정보를 추출해야 함
            let exchange = "unknown".to_string(); // 임시
            let stats = exchange_stats.entry(exchange).or_insert_with(|| ExchangeStats {
                exchange: exchange.clone(),
                total_trades: 0,
                successful_trades: 0,
                total_profit: U256::zero(),
                avg_slippage: 0.0,
            });
            
            stats.total_trades += 1;
            if result.success {
                stats.successful_trades += 1;
                if let Some(profit) = result.actual_profit {
                    stats.total_profit += profit;
                }
            }
            stats.avg_slippage = (stats.avg_slippage * (stats.total_trades - 1) as f64 + 
                                result.slippage) / stats.total_trades as f64;
        }
        
        let best_exchange = exchange_stats.values()
            .max_by_key(|s| s.total_profit.as_u128())
            .map(|s| s.exchange.clone())
            .unwrap_or_default();
        
        ExchangeAnalysis {
            exchange_stats: exchange_stats.into_values().collect(),
            best_exchange,
            total_exchanges: exchange_stats.len(),
        }
    }
    
    /// 심볼별 성능 분석
    async fn analyze_symbol_performance(
        &self,
        execution_history: &[ArbitrageExecutionResult],
    ) -> SymbolAnalysis {
        let mut symbol_stats: std::collections::HashMap<String, SymbolStats> = std::collections::HashMap::new();
        
        for result in execution_history {
            // 실제로는 심볼 정보를 추출해야 함
            let symbol = "ETH/USDT".to_string(); // 임시
            let stats = symbol_stats.entry(symbol).or_insert_with(|| SymbolStats {
                symbol: symbol.clone(),
                total_trades: 0,
                successful_trades: 0,
                total_profit: U256::zero(),
                avg_profit_per_trade: U256::zero(),
            });
            
            stats.total_trades += 1;
            if result.success {
                stats.successful_trades += 1;
                if let Some(profit) = result.actual_profit {
                    stats.total_profit += profit;
                }
            }
            
            if stats.successful_trades > 0 {
                stats.avg_profit_per_trade = stats.total_profit / U256::from(stats.successful_trades);
            }
        }
        
        let best_symbol = symbol_stats.values()
            .max_by_key(|s| s.total_profit.as_u128())
            .map(|s| s.symbol.clone())
            .unwrap_or_default();
        
        SymbolAnalysis {
            symbol_stats: symbol_stats.into_values().collect(),
            best_symbol,
            total_symbols: symbol_stats.len(),
        }
    }
    
    /// 수익성 분석
    async fn analyze_profitability(
        &self,
        execution_history: &[ArbitrageExecutionResult],
    ) -> ProfitabilityAnalysis {
        let total_trades = execution_history.len() as f64;
        let profitable_trades = execution_history.iter()
            .filter(|r| r.success && r.actual_profit.map_or(false, |p| p > U256::zero()))
            .count() as f64;
        
        let total_profit: U256 = execution_history.iter()
            .filter_map(|r| r.actual_profit)
            .sum();
        
        let total_fees: U256 = execution_history.iter()
            .map(|r| r.fees_paid)
            .sum();
        
        let net_profit = if total_profit > total_fees {
            total_profit - total_fees
        } else {
            U256::zero()
        };
        
        let profit_margin = if total_profit > U256::zero() {
            (net_profit.as_u128() as f64 / total_profit.as_u128() as f64) * 100.0
        } else {
            0.0
        };
        
        ProfitabilityAnalysis {
            total_trades,
            profitable_trades,
            profitability_rate: if total_trades > 0.0 {
                profitable_trades / total_trades
            } else {
                0.0
            },
            total_profit,
            net_profit,
            total_fees,
            profit_margin,
            avg_profit_per_trade: if profitable_trades > 0.0 {
                total_profit / U256::from(profitable_trades as u64)
            } else {
                U256::zero()
            },
        }
    }
    
    /// 위험 메트릭 분석
    async fn analyze_risk_metrics(&self, risk_metrics: &RiskMetrics) -> RiskAnalysis {
        let exposure_ratio = if risk_metrics.current_exposure > U256::zero() {
            risk_metrics.current_exposure.as_u128() as f64 / 1e18 // ETH 단위로 변환
        } else {
            0.0
        };
        
        let daily_pnl_ratio = if risk_metrics.daily_pnl > U256::zero() {
            risk_metrics.daily_pnl.as_u128() as f64 / 1e18
        } else {
            0.0
        };
        
        RiskAnalysis {
            current_exposure_eth: exposure_ratio,
            daily_pnl_eth: daily_pnl_ratio,
            max_drawdown_eth: risk_metrics.max_drawdown.as_u128() as f64 / 1e18,
            sharpe_ratio: risk_metrics.sharpe_ratio,
            win_rate: risk_metrics.win_rate,
            profit_factor: risk_metrics.profit_factor,
            risk_score: self.calculate_risk_score(risk_metrics),
        }
    }
    
    /// 위험 점수 계산
    fn calculate_risk_score(&self, risk_metrics: &RiskMetrics) -> f64 {
        let mut score = 0.0;
        
        // 노출도 점수 (낮을수록 좋음)
        let exposure_score = (risk_metrics.current_exposure.as_u128() as f64 / 1e18).min(1.0);
        score += exposure_score * 0.3;
        
        // 일일 PnL 점수 (양수일수록 좋음)
        let pnl_score = if risk_metrics.daily_pnl > U256::zero() {
            0.0
        } else {
            (risk_metrics.daily_pnl.as_u128() as f64 / 1e18).abs().min(1.0)
        };
        score += pnl_score * 0.2;
        
        // 샤프 비율 점수 (높을수록 좋음)
        let sharpe_score = (2.0 - risk_metrics.sharpe_ratio).max(0.0).min(1.0);
        score += sharpe_score * 0.2;
        
        // 승률 점수 (높을수록 좋음)
        let win_rate_score = 1.0 - risk_metrics.win_rate;
        score += win_rate_score * 0.15;
        
        // 수익 팩터 점수 (높을수록 좋음)
        let profit_factor_score = (2.0 - risk_metrics.profit_factor).max(0.0).min(1.0);
        score += profit_factor_score * 0.15;
        
        score.clamp(0.0, 1.0)
    }
    
    /// 성능 리포트 생성
    pub async fn generate_performance_report(&self) -> String {
        let analysis = self.get_detailed_analysis().await;
        
        format!(
            "📊 마이크로아비트리지 성능 리포트\n\
             =================================\n\
             \n\
             📈 전체 통계:\n\
             - 총 기회: {}\n\
             - 실행된 거래: {}\n\
             - 성공한 거래: {}\n\
             - 성공률: {:.2}%\n\
             - 총 수익: {} ETH\n\
             - 평균 거래당 수익: {} ETH\n\
             - 평균 실행 시간: {:.2}ms\n\
             \n\
             💰 수익성 분석:\n\
             - 수익성 있는 거래: {:.0}\n\
             - 수익성 비율: {:.2}%\n\
             - 순수익: {} ETH\n\
             - 수수료: {} ETH\n\
             - 수익 마진: {:.2}%\n\
             \n\
             ⚠️ 위험 분석:\n\
             - 현재 노출도: {:.4} ETH\n\
             - 일일 PnL: {:.4} ETH\n\
             - 최대 손실: {:.4} ETH\n\
             - 샤프 비율: {:.2}\n\
             - 승률: {:.2}%\n\
             - 수익 팩터: {:.2}\n\
             - 위험 점수: {:.2}\n\
             \n\
             🕐 시간대별 분석:\n\
             - 최고 성과 시간: {}시\n\
             - 총 시간대: {}\n\
             \n\
             🏛️ 거래소별 분석:\n\
             - 최고 성과 거래소: {}\n\
             - 총 거래소: {}\n\
             \n\
             💎 심볼별 분석:\n\
             - 최고 성과 심볼: {}\n\
             - 총 심볼: {}\n\
             \n\
             📅 분석 시간: {}",
            analysis.overall_stats.total_opportunities,
            analysis.overall_stats.executed_trades,
            analysis.overall_stats.successful_trades,
            analysis.overall_stats.success_rate * 100.0,
            format_eth_amount(analysis.overall_stats.total_profit),
            format_eth_amount(analysis.overall_stats.avg_profit_per_trade),
            analysis.overall_stats.avg_execution_time_ms,
            analysis.profitability_analysis.profitable_trades,
            analysis.profitability_analysis.profitability_rate * 100.0,
            format_eth_amount(analysis.profitability_analysis.net_profit),
            format_eth_amount(analysis.profitability_analysis.total_fees),
            analysis.profitability_analysis.profit_margin,
            analysis.risk_analysis.current_exposure_eth,
            analysis.risk_analysis.daily_pnl_eth,
            analysis.risk_analysis.max_drawdown_eth,
            analysis.risk_analysis.sharpe_ratio,
            analysis.risk_analysis.win_rate * 100.0,
            analysis.risk_analysis.profit_factor,
            analysis.risk_analysis.risk_score,
            analysis.hourly_analysis.best_hour,
            analysis.hourly_analysis.hourly_stats.len(),
            analysis.exchange_analysis.best_exchange,
            analysis.exchange_analysis.total_exchanges,
            analysis.symbol_analysis.best_symbol,
            analysis.symbol_analysis.total_symbols,
            analysis.analysis_timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}

/// 상세 성능 분석
#[derive(Debug, Clone)]
pub struct DetailedPerformanceAnalysis {
    pub overall_stats: MicroArbitrageStats,
    pub risk_metrics: RiskMetrics,
    pub hourly_analysis: HourlyAnalysis,
    pub exchange_analysis: ExchangeAnalysis,
    pub symbol_analysis: SymbolAnalysis,
    pub profitability_analysis: ProfitabilityAnalysis,
    pub risk_analysis: RiskAnalysis,
    pub total_opportunities: usize,
    pub analysis_timestamp: chrono::DateTime<Utc>,
}

/// 시간대별 분석
#[derive(Debug, Clone)]
pub struct HourlyAnalysis {
    pub hourly_stats: Vec<HourlyStats>,
    pub best_hour: u8,
    pub worst_hour: u8,
}

/// 시간대별 통계
#[derive(Debug, Clone)]
pub struct HourlyStats {
    pub hour: u8,
    pub total_trades: u64,
    pub successful_trades: u64,
    pub total_profit: U256,
    pub avg_execution_time: f64,
}

/// 거래소별 분석
#[derive(Debug, Clone)]
pub struct ExchangeAnalysis {
    pub exchange_stats: Vec<ExchangeStats>,
    pub best_exchange: String,
    pub total_exchanges: usize,
}

/// 거래소별 통계
#[derive(Debug, Clone)]
pub struct ExchangeStats {
    pub exchange: String,
    pub total_trades: u64,
    pub successful_trades: u64,
    pub total_profit: U256,
    pub avg_slippage: f64,
}

/// 심볼별 분석
#[derive(Debug, Clone)]
pub struct SymbolAnalysis {
    pub symbol_stats: Vec<SymbolStats>,
    pub best_symbol: String,
    pub total_symbols: usize,
}

/// 심볼별 통계
#[derive(Debug, Clone)]
pub struct SymbolStats {
    pub symbol: String,
    pub total_trades: u64,
    pub successful_trades: u64,
    pub total_profit: U256,
    pub avg_profit_per_trade: U256,
}

/// 수익성 분석
#[derive(Debug, Clone)]
pub struct ProfitabilityAnalysis {
    pub total_trades: f64,
    pub profitable_trades: f64,
    pub profitability_rate: f64,
    pub total_profit: U256,
    pub net_profit: U256,
    pub total_fees: U256,
    pub profit_margin: f64,
    pub avg_profit_per_trade: U256,
}

/// 위험 분석
#[derive(Debug, Clone)]
pub struct RiskAnalysis {
    pub current_exposure_eth: f64,
    pub daily_pnl_eth: f64,
    pub max_drawdown_eth: f64,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub risk_score: f64,
}

/// ETH 금액 포맷팅
fn format_eth_amount(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}