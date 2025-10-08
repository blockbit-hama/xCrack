use ethers::types::{Address, U256, H256, Bytes, Transaction};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// 샌드위치 공격 기회
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandwichOpportunity {
    pub target_tx_hash: H256,
    pub target_tx: TargetTransaction,
    pub dex_router: Address,
    pub dex_type: DexType,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub expected_amount_out: U256,
    pub front_run_amount: U256,
    pub back_run_amount: U256,
    pub estimated_profit: U256,
    pub gas_cost: U256,
    pub net_profit: U256,
    pub profit_percentage: f64,
    pub success_probability: f64,
    pub price_impact: f64,
    pub slippage_tolerance: f64,
    pub optimal_size_kelly: U256,
    pub competition_level: CompetitionLevel,
    pub detected_at: u64, // block number
}

/// 타겟 트랜잭션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetTransaction {
    pub hash: H256,
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub gas: U256,
    pub gas_price: U256,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub nonce: U256,
    pub data: Bytes,
    pub block_number: Option<u64>,
}

impl From<Transaction> for TargetTransaction {
    fn from(tx: Transaction) -> Self {
        Self {
            hash: tx.hash,
            from: tx.from,
            to: tx.to.unwrap_or_default(),
            value: tx.value,
            gas: tx.gas,
            gas_price: tx.gas_price.unwrap_or_default(),
            max_fee_per_gas: tx.max_fee_per_gas,
            max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
            nonce: tx.nonce,
            data: tx.input,
            block_number: tx.block_number.map(|b| b.as_u64()),
        }
    }
}

/// DEX 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexType {
    UniswapV2,
    UniswapV3,
    SushiSwap,
    PancakeSwap,
    Curve,
    Balancer,
}

impl DexType {
    pub fn name(&self) -> &str {
        match self {
            DexType::UniswapV2 => "Uniswap V2",
            DexType::UniswapV3 => "Uniswap V3",
            DexType::SushiSwap => "SushiSwap",
            DexType::PancakeSwap => "PancakeSwap",
            DexType::Curve => "Curve",
            DexType::Balancer => "Balancer",
        }
    }

    pub fn default_fee_bps(&self) -> u32 {
        match self {
            DexType::UniswapV2 | DexType::SushiSwap | DexType::PancakeSwap => 30, // 0.3%
            DexType::UniswapV3 => 30, // 0.3% (default, can be 0.05%, 0.3%, 1%)
            DexType::Curve => 4, // 0.04%
            DexType::Balancer => 30, // 0.3%
        }
    }
}

/// DEX 라우터 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexRouterInfo {
    pub dex_type: DexType,
    pub router_address: Address,
    pub factory_address: Address,
    pub swap_exact_tokens_selector: [u8; 4],
    pub swap_tokens_for_exact_selector: [u8; 4],
    pub fee_bps: u32,
}

/// 경쟁 수준
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompetitionLevel {
    Low,      // 낮은 경쟁 (<2 경쟁자)
    Medium,   // 중간 경쟁 (2-5 경쟁자)
    High,     // 높은 경쟁 (5-10 경쟁자)
    Critical, // 치열한 경쟁 (>10 경쟁자)
}

impl CompetitionLevel {
    pub fn success_probability(&self) -> f64 {
        match self {
            CompetitionLevel::Low => 0.85,
            CompetitionLevel::Medium => 0.65,
            CompetitionLevel::High => 0.40,
            CompetitionLevel::Critical => 0.20,
        }
    }

    pub fn recommended_gas_multiplier(&self) -> f64 {
        match self {
            CompetitionLevel::Low => 1.1,
            CompetitionLevel::Medium => 1.3,
            CompetitionLevel::High => 1.5,
            CompetitionLevel::Critical => 2.0,
        }
    }
}

/// 샌드위치 번들
#[derive(Debug, Clone)]
pub struct SandwichBundle {
    pub opportunity: SandwichOpportunity,
    pub front_run_tx: Bytes,
    pub target_tx_hash: H256,
    pub back_run_tx: Bytes,
    pub bundle_hash: Option<H256>,
    pub estimated_profit: U256,
    pub total_gas_cost: U256,
    pub net_profit: U256,
    pub success_probability: f64,
    pub submitted_at: u64, // block number
}

/// 샌드위치 실행 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandwichExecutionResult {
    pub opportunity_id: String,
    pub bundle_hash: H256,
    pub front_run_tx_hash: Option<H256>,
    pub back_run_tx_hash: Option<H256>,
    pub success: bool,
    pub actual_profit: U256,
    pub actual_gas_cost: U256,
    pub net_profit: U256,
    pub execution_time_ms: u64,
    pub block_number: u64,
    pub error_message: Option<String>,
}

/// 샌드위치 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandwichStats {
    pub total_opportunities_detected: u64,
    pub total_opportunities_analyzed: u64,
    pub total_bundles_submitted: u64,
    pub total_bundles_included: u64,
    pub total_successful_sandwiches: u64,
    pub total_failed_sandwiches: u64,
    pub total_profit: U256,
    pub total_gas_cost: U256,
    pub net_profit: U256,
    pub avg_profit_per_sandwich: U256,
    pub success_rate: f64,
    pub avg_competition_level: f64,
    pub last_update: Instant,
    pub start_time: Instant,
}

impl Default for SandwichStats {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            total_opportunities_detected: 0,
            total_opportunities_analyzed: 0,
            total_bundles_submitted: 0,
            total_bundles_included: 0,
            total_successful_sandwiches: 0,
            total_failed_sandwiches: 0,
            total_profit: U256::zero(),
            total_gas_cost: U256::zero(),
            net_profit: U256::zero(),
            avg_profit_per_sandwich: U256::zero(),
            success_rate: 0.0,
            avg_competition_level: 0.0,
            last_update: now,
            start_time: now,
        }
    }
}

impl SandwichStats {
    pub fn update_success(&mut self, profit: U256, gas_cost: U256) {
        self.total_successful_sandwiches += 1;
        self.total_profit += profit;
        self.total_gas_cost += gas_cost;
        if profit > gas_cost {
            self.net_profit += profit - gas_cost;
        }
        self.update_derived_stats();
    }

    pub fn update_failure(&mut self) {
        self.total_failed_sandwiches += 1;
        self.update_derived_stats();
    }

    fn update_derived_stats(&mut self) {
        let total_executions = self.total_successful_sandwiches + self.total_failed_sandwiches;
        if total_executions > 0 {
            self.success_rate = self.total_successful_sandwiches as f64 / total_executions as f64;
        }
        if self.total_successful_sandwiches > 0 {
            self.avg_profit_per_sandwich = self.total_profit / self.total_successful_sandwiches;
        }
        self.last_update = Instant::now();
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

/// Kelly Criterion 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KellyCriterionParams {
    pub success_probability: f64,  // p: 성공 확률 (0.0 ~ 1.0)
    pub price_impact_bps: u32,     // b: 가격 영향 (basis points)
    pub available_capital: U256,   // 사용 가능한 자본
    pub risk_factor: f64,          // 위험 조정 계수 (0.5 = Half Kelly, 1.0 = Full Kelly)
}

/// Kelly Criterion 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KellyCriterionResult {
    pub optimal_size: U256,
    pub optimal_size_percentage: f64,
    pub kelly_percentage: f64,
    pub adjusted_kelly_percentage: f64,
    pub expected_value: f64,
    pub risk_of_ruin: f64,
}
