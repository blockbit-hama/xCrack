//! Multi-Asset Arbitrage 타입 정의
//!
//! 이 모듈은 다중자산 아비트리지 전략에서 사용되는
//! 모든 데이터 구조와 타입을 정의합니다.

use std::collections::HashMap;
use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

/// 다중자산 아비트리지 전략 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MultiAssetStrategyType {
    /// 삼각 아비트리지: A → B → C → A
    TriangularArbitrage {
        token_a: Address,
        token_b: Address,
        token_c: Address,
        amount_a: U256,
        amount_b: U256,
    },

    /// 포지션 마이그레이션: Aave → Compound
    PositionMigration {
        from_protocol: String,
        to_protocol: String,
        assets: Vec<Address>,
        amounts: Vec<U256>,
    },

    /// 복합 아비트리지: 여러 DEX를 거친 복잡한 경로
    ComplexArbitrage {
        route: Vec<SwapStep>,
        total_hops: usize,
    },
}

/// 스왑 단계
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapStep {
    pub dex: String,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub expected_amount_out: U256,
}

/// 다중자산 아비트리지 기회
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAssetArbitrageOpportunity {
    pub id: String,
    pub strategy_type: MultiAssetStrategyType,
    pub expected_profit: U256,
    pub expected_profit_usd: Decimal,
    pub profit_percentage: f64,
    pub total_gas_estimate: u64,
    pub execution_deadline: DateTime<Utc>,
    pub confidence_score: f64,
    pub flashloan_amounts: Vec<FlashLoanAmount>,
    pub created_at: DateTime<Utc>,
}

/// FlashLoan 금액
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanAmount {
    pub asset: Address,
    pub amount: U256,
    pub premium: U256,
}

/// 다중자산 아비트리지 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAssetArbitrageStats {
    pub total_opportunities: u64,
    pub executed_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_volume: U256,
    pub total_profit: U256,
    pub total_fees: U256,
    pub avg_profit_per_trade: U256,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub profit_rate: f64,
    pub uptime_percentage: f64,

    // 전략별 통계
    pub triangular_arbitrage_count: u64,
    pub position_migration_count: u64,
    pub complex_arbitrage_count: u64,

    // DEX 성능
    pub dex_performance: HashMap<String, DexPerformanceData>,
}

/// DEX 성능 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexPerformanceData {
    pub dex_name: String,
    pub total_swaps: u64,
    pub successful_swaps: u64,
    pub failed_swaps: u64,
    pub avg_slippage: f64,
    pub avg_gas_used: u64,
    pub total_volume: U256,
}

impl Default for MultiAssetArbitrageStats {
    fn default() -> Self {
        Self {
            total_opportunities: 0,
            executed_trades: 0,
            successful_trades: 0,
            failed_trades: 0,
            total_volume: U256::zero(),
            total_profit: U256::zero(),
            total_fees: U256::zero(),
            avg_profit_per_trade: U256::zero(),
            avg_execution_time_ms: 0.0,
            success_rate: 0.0,
            profit_rate: 0.0,
            uptime_percentage: 100.0,
            triangular_arbitrage_count: 0,
            position_migration_count: 0,
            complex_arbitrage_count: 0,
            dex_performance: HashMap::new(),
        }
    }
}

impl MultiAssetArbitrageOpportunity {
    /// 기회 유효성 검증
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();

        // 유효기간 확인
        if now > self.execution_deadline {
            return false;
        }

        // 수익률 확인
        if self.profit_percentage <= 0.0 {
            return false;
        }

        // 신뢰도 확인
        if self.confidence_score < 0.5 {
            return false;
        }

        true
    }

    /// 예상 순이익 계산 (가스 및 FlashLoan 수수료 차감)
    pub fn calculate_net_profit(&self, gas_price: U256) -> U256 {
        // 가스 비용 계산
        let gas_cost = U256::from(self.total_gas_estimate) * gas_price;

        // FlashLoan 프리미엄 합계 계산
        let total_premium: U256 = self.flashloan_amounts
            .iter()
            .map(|fl| fl.premium)
            .fold(U256::zero(), |acc, p| acc + p);

        let total_cost = gas_cost + total_premium;

        if self.expected_profit > total_cost {
            self.expected_profit - total_cost
        } else {
            U256::zero()
        }
    }
}
