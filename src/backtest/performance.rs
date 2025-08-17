/// Performance analysis for backtesting
use crate::types::OrderExecutionResult;
use alloy::primitives::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestPerformance {
    pub total_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_profit: U256,
    pub total_fees: U256,
    pub net_profit: U256,
    pub success_rate: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub returns_by_strategy: HashMap<String, StrategyPerformance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub trades: u64,
    pub profit: U256,
    pub success_rate: f64,
    pub avg_profit_per_trade: U256,
}

#[derive(Debug, Clone)]
pub struct PerformanceAnalyzer {
    pub executions: Vec<OrderExecutionResult>,
    pub starting_balance: U256,
    pub current_balance: U256,
}

impl PerformanceAnalyzer {
    pub fn new(starting_balance: U256) -> Self {
        Self {
            executions: Vec::new(),
            starting_balance,
            current_balance: starting_balance,
        }
    }
    
    /// Calculate metrics for backtesting
    pub async fn calculate_metrics(
        &self,
        trades: &[crate::backtest::engine::BacktestTrade],
        portfolio_timeline: &[(u64, f64)],
        initial_capital: f64,
    ) -> anyhow::Result<crate::types::PerformanceMetrics> {
        use crate::types::PerformanceMetrics;
        use alloy::primitives::U256;
        
        let total_trades = trades.len() as u64;
        let successful_trades = trades.iter().filter(|t| {
            match &t.trade_type {
                crate::backtest::engine::TradeType::Mev { profit, .. } => *profit > t.fee,
                _ => true, // Consider non-MEV trades as successful if they executed
            }
        }).count() as u64;
        
        // Calculate total profit
        let total_profit: f64 = trades.iter().map(|t| {
            match &t.trade_type {
                crate::backtest::engine::TradeType::Mev { profit, .. } => *profit - t.fee,
                _ => -t.fee, // Other trades cost fees
            }
        }).sum();
        
        // Calculate success rate
        let success_rate = if total_trades > 0 {
            successful_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        
        // Calculate average analysis time (mock for backtesting)
        let avg_analysis_time = 50.0; // 50ms average
        
        // Calculate average submission time (mock for backtesting)  
        let avg_submission_time = 100.0; // 100ms average
        
        Ok(PerformanceMetrics {
            transactions_processed: total_trades,
            opportunities_found: total_trades, // In backtest, all trades are opportunities
            bundles_submitted: total_trades,
            bundles_included: successful_trades,
            total_profit: U256::from((total_profit.max(0.0) * 1e18) as u128), // Convert to wei
            total_gas_spent: U256::from((trades.len() as u128) * 21000 * 20_000_000_000u128), // Mock gas calculation
            avg_analysis_time,
            avg_submission_time,
            success_rate,
            uptime: 100, // Mock uptime
        })
    }
    
    /// Add execution result for analysis
    pub fn add_execution(&mut self, execution: OrderExecutionResult) {
        self.executions.push(execution);
    }
    
    /// Calculate overall performance metrics
    pub fn calculate_performance(&self) -> BacktestPerformance {
        let total_trades = self.executions.len() as u64;
        let successful_trades = self.executions.iter()
            .filter(|e| matches!(e.status, crate::types::OrderStatus::Filled))
            .count() as u64;
        let failed_trades = total_trades - successful_trades;
        
        let total_profit = self.executions.iter()
            .map(|e| e.filled_amount)
            .fold(U256::ZERO, |acc, x| acc + x);
            
        let total_fees = self.executions.iter()
            .map(|e| e.fees)
            .fold(U256::ZERO, |acc, x| acc + x);
            
        let net_profit = if total_profit > total_fees {
            total_profit - total_fees
        } else {
            U256::ZERO
        };
        
        let success_rate = if total_trades > 0 {
            successful_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        
        let returns_by_strategy = self.calculate_returns_by_strategy();

        BacktestPerformance {
            total_trades,
            successful_trades,
            failed_trades,
            total_profit,
            total_fees,
            net_profit,
            success_rate,
            profit_factor: self.calculate_profit_factor(),
            max_drawdown: self.calculate_max_drawdown(),
            sharpe_ratio: self.calculate_sharpe_ratio(),
            returns_by_strategy,
        }
    }

    /// 전략별 수익 요약 생성
    fn calculate_returns_by_strategy(&self) -> HashMap<String, StrategyPerformance> {
        let mut map: HashMap<String, StrategyPerformance> = HashMap::new();

        for exec in &self.executions {
            // 전략 식별자: 없으면 "unknown"
            let strategy_key = exec.strategy_key.clone().unwrap_or_else(|| "unknown".to_string());

            let entry = map.entry(strategy_key).or_insert_with(|| StrategyPerformance {
                trades: 0,
                profit: U256::ZERO,
                success_rate: 0.0,
                avg_profit_per_trade: U256::ZERO,
            });

            entry.trades += 1;
            let pnl = if exec.filled_amount > exec.fees { exec.filled_amount - exec.fees } else { U256::ZERO };
            entry.profit = entry.profit + pnl;
        }

        // 성공률/평균 수익 계산 (OrderExecutionResult에 성공 여부가 없으면 체결금액>0 기준)
        for (_k, perf) in map.iter_mut() {
            if perf.trades > 0 {
                let avg = perf.profit / U256::from(perf.trades as u128);
                perf.avg_profit_per_trade = avg;
                // 간이 성공률: 수익>0 비율 (정밀한 성공 판정 필드 없을 경우)
                // 여기서는 보수적으로 trades 중 절반 성공으로 가정 불가하므로 0/1로 계산 불가 → 유지
                // perf.success_rate = ...
            }
        }

        map
    }
    
    /// Calculate profit factor (gross profit / gross loss)
    fn calculate_profit_factor(&self) -> f64 {
        let (gross_profit, gross_loss) = self.executions.iter().fold((0u128, 0u128), |(profit, loss), execution| {
            let pnl = execution.filled_amount.to::<u128>() as i128 - execution.fees.to::<u128>() as i128;
            if pnl > 0 {
                (profit + pnl as u128, loss)
            } else {
                (profit, loss + (-pnl) as u128)
            }
        });
        
        if gross_loss > 0 {
            gross_profit as f64 / gross_loss as f64
        } else {
            f64::INFINITY
        }
    }
    
    /// Calculate maximum drawdown
    fn calculate_max_drawdown(&self) -> f64 {
        let mut peak = self.starting_balance.to::<u128>() as f64;
        let mut max_drawdown = 0.0;
        let mut current_balance = peak;
        
        for execution in &self.executions {
            let pnl = execution.filled_amount.to::<u128>() as f64 - execution.fees.to::<u128>() as f64;
            current_balance += pnl;
            
            if current_balance > peak {
                peak = current_balance;
            }
            
            let drawdown = (peak - current_balance) / peak;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }
        
        max_drawdown
    }
    
    /// Calculate Sharpe ratio (simplified)
    fn calculate_sharpe_ratio(&self) -> f64 {
        if self.executions.is_empty() {
            return 0.0;
        }
        
        let returns: Vec<f64> = self.executions.iter()
            .map(|e| {
                let pnl = e.filled_amount.to::<u128>() as f64 - e.fees.to::<u128>() as f64;
                pnl / self.starting_balance.to::<u128>() as f64
            })
            .collect();
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev > 0.0 {
            mean_return / std_dev
        } else {
            0.0
        }
    }
}

impl Default for PerformanceAnalyzer {
    fn default() -> Self {
        Self::new(U256::from(1000000000000000000u64)) // 1 ETH default
    }
}