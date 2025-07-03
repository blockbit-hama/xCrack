use std::sync::Arc;
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use tracing::{info, debug, error, warn};
use ethers::{
    providers::{Provider, Ws},
    types::{H160, H256, U256, Bytes, TransactionRequest},
    utils::keccak256,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use crate::config::Config;
use crate::types::{Transaction, Opportunity, StrategyType, Bundle, ArbitrageDetails};
use crate::strategies::Strategy;

/// 실시간 샌드위치 공격 전략
/// 
/// 멤풀에서 대형 스왑 트랜잭션을 감지하고, 해당 트랜잭션 앞뒤로
/// 우리의 트랜잭션을 삽입하여 가격 변동으로부터 수익을 추출합니다.
pub struct RealTimeSandwichStrategy {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    enabled: bool,
    
    // 샌드위치 대상 DEX 정보
    dex_addresses: HashMap<H160, DexInfo>,
    
    // 최소 수익성 임계값
    min_profit_eth: U256,
    min_profit_percentage: f64,
    
    // 가스 가격 전략
    gas_multiplier: f64,
    max_gas_price: U256,
    
    // 통계
    stats: Arc<Mutex<SandwichStats>>,
}

#[derive(Debug, Clone)]
struct DexInfo {
    name: String,
    router_address: H160,
    factory_address: H160,
    swap_function: Vec<u8>,
    fee: u32, // basis points (e.g., 30 = 0.3%)
}

#[derive(Debug, Clone)]
struct SandwichStats {
    transactions_analyzed: u64,
    opportunities_found: u64,
    successful_sandwiches: u64,
    total_profit: U256,
    avg_profit_per_sandwich: U256,
    last_analysis_time: Option<Instant>,
}

#[derive(Debug, Clone)]
struct SandwichOpportunity {
    target_tx: Transaction,
    front_run_tx: TransactionRequest,
    back_run_tx: TransactionRequest,
    expected_profit: U256,
    gas_cost: U256,
    net_profit: U256,
    success_probability: f64,
}

impl RealTimeSandwichStrategy {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🥪 샌드위치 전략 초기화 중...");
        
        let mut dex_addresses = HashMap::new();
        
        // Uniswap V2
        dex_addresses.insert(
            "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse()?,
            DexInfo {
                name: "Uniswap V2".to_string(),
                router_address: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse()?,
                factory_address: "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse()?,
                swap_function: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
                fee: 30, // 0.3%
            }
        );
        
        // SushiSwap
        dex_addresses.insert(
            "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse()?,
            DexInfo {
                name: "SushiSwap".to_string(),
                router_address: "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse()?,
                factory_address: "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse()?,
                swap_function: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
                fee: 30, // 0.3%
            }
        );
        
        // PancakeSwap V2
        dex_addresses.insert(
            "0x10ED43C718714eb63d5aA57B78B54704E256024E".parse()?,
            DexInfo {
                name: "PancakeSwap V2".to_string(),
                router_address: "0x10ED43C718714eb63d5aA57B78B54704E256024E".parse()?,
                factory_address: "0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73".parse()?,
                swap_function: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
                fee: 25, // 0.25%
            }
        );
        
        let min_profit_eth = U256::from_str_radix(
            &config.strategies.sandwich.min_profit_eth,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("100000000000000000", 10).unwrap()); // 0.1 ETH
        
        let min_profit_percentage = config.strategies.sandwich.min_profit_percentage;
        let gas_multiplier = config.strategies.sandwich.gas_multiplier;
        let max_gas_price = U256::from_str_radix(
            &config.strategies.sandwich.max_gas_price_gwei,
            10
        ).unwrap_or_else(|_| U256::from(100_000_000_000u64)) * U256::from(1_000_000_000u64); // gwei to wei
        
        info!("✅ 샌드위치 전략 초기화 완료");
        info!("  📊 최소 수익: {} ETH", ethers::utils::format_ether(min_profit_eth));
        info!("  📈 최소 수익률: {:.2}%", min_profit_percentage);
        info!("  ⛽ 가스 배수: {:.2}x", gas_multiplier);
        info!("  🔥 최대 가스 가격: {} gwei", max_gas_price / U256::from(1_000_000_000u64));
        
        Ok(Self {
            config,
            provider,
            enabled: true,
            dex_addresses,
            min_profit_eth,
            min_profit_percentage,
            gas_multiplier,
            max_gas_price,
            stats: Arc::new(Mutex::new(SandwichStats {
                transactions_analyzed: 0,
                opportunities_found: 0,
                successful_sandwiches: 0,
                total_profit: U256::zero(),
                avg_profit_per_sandwich: U256::zero(),
                last_analysis_time: None,
            })),
        })
    }
    
    /// 트랜잭션이 샌드위치 대상인지 확인
    fn is_sandwich_target(&self, tx: &Transaction) -> bool {
        // 1. DEX 라우터로의 호출인지 확인
        if let Some(to) = tx.to {
            if !self.dex_addresses.contains_key(&to) {
                return false;
            }
        } else {
            return false; // 컨트랙트 생성 트랜잭션은 제외
        }
        
        // 2. 스왑 함수 호출인지 확인
        if tx.data.len() < 4 {
            return false;
        }
        
        let function_selector = &tx.data[0..4];
        let swap_functions = vec![
            vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            vec![0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            vec![0x18, 0xcb, 0xa5, 0xe5], // swapExactTokensForETH
        ];
        
        if !swap_functions.contains(function_selector) {
            return false;
        }
        
        // 3. 최소 거래 크기 확인
        let min_value = U256::from_str_radix("1000000000000000000", 10).unwrap(); // 1 ETH
        if tx.value < min_value {
            return false;
        }
        
        // 4. 가스 가격이 너무 높지 않은지 확인 (경쟁이 치열하지 않은지)
        let max_target_gas = U256::from(50_000_000_000u64); // 50 gwei
        if tx.gas_price > max_target_gas {
            return false;
        }
        
        true
    }
    
    /// 샌드위치 기회 분석
    async fn analyze_sandwich_opportunity(&self, target_tx: &Transaction) -> Result<Option<SandwichOpportunity>> {
        let dex_info = if let Some(to) = target_tx.to {
            self.dex_addresses.get(&to).cloned()
        } else {
            return Ok(None);
        };
        
        let dex_info = dex_info.ok_or_else(|| anyhow!("DEX 정보를 찾을 수 없습니다"))?;
        
        // 1. 대상 트랜잭션의 스왑 세부사항 파싱
        let swap_details = self.parse_swap_transaction(target_tx, &dex_info).await?;
        
        // 2. 예상 가격 영향 계산
        let price_impact = self.calculate_price_impact(&swap_details, &dex_info).await?;
        
        // 3. 최적 샌드위치 크기 계산
        let optimal_size = self.calculate_optimal_sandwich_size(&swap_details, &price_impact).await?;
        
        // 4. 프론트런 트랜잭션 생성
        let front_run_tx = self.create_front_run_transaction(
            &swap_details,
            &optimal_size,
            target_tx.gas_price,
            &dex_info
        ).await?;
        
        // 5. 백런 트랜잭션 생성
        let back_run_tx = self.create_back_run_transaction(
            &swap_details,
            &optimal_size,
            target_tx.gas_price,
            &dex_info
        ).await?;
        
        // 6. 수익성 계산
        let (expected_profit, gas_cost, net_profit) = self.calculate_sandwich_profit(
            &front_run_tx,
            &back_run_tx,
            &swap_details,
            &optimal_size
        ).await?;
        
        // 7. 수익성 검증
        if net_profit < self.min_profit_eth {
            debug!("❌ 샌드위치 수익이 너무 낮음: {} ETH", ethers::utils::format_ether(net_profit));
            return Ok(None);
        }
        
        let profit_percentage = (net_profit.as_u128() as f64 / optimal_size.amount.as_u128() as f64) * 100.0;
        if profit_percentage < self.min_profit_percentage {
            debug!("❌ 샌드위치 수익률이 너무 낮음: {:.2}%", profit_percentage);
            return Ok(None);
        }
        
        // 8. 성공 확률 계산
        let success_probability = self.calculate_success_probability(target_tx, &net_profit).await?;
        
        if success_probability < 0.3 {
            debug!("❌ 샌드위치 성공 확률이 너무 낮음: {:.2}%", success_probability * 100.0);
            return Ok(None);
        }
        
        info!("🎯 샌드위치 기회 발견!");
        info!("  📊 예상 수익: {} ETH", ethers::utils::format_ether(net_profit));
        info!("  📈 수익률: {:.2}%", profit_percentage);
        info!("  🎲 성공 확률: {:.2}%", success_probability * 100.0);
        info!("  ⛽ 가스 비용: {} ETH", ethers::utils::format_ether(gas_cost));
        
        Ok(Some(SandwichOpportunity {
            target_tx: target_tx.clone(),
            front_run_tx,
            back_run_tx,
            expected_profit,
            gas_cost,
            net_profit,
            success_probability,
        }))
    }
    
    /// 스왑 트랜잭션 파싱
    async fn parse_swap_transaction(&self, tx: &Transaction, dex_info: &DexInfo) -> Result<SwapDetails> {
        // 실제 구현에서는 ABI를 사용하여 스왑 파라미터를 파싱
        // 여기서는 간단한 예시로 구현
        
        let amount_in = tx.value;
        let token_in = H160::zero(); // ETH
        let token_out = "0xA0b86a33E6441b8C4C3132E4B4F4b4F4b4F4b4F4b".parse()?; // 예시 토큰
        
        Ok(SwapDetails {
            token_in,
            token_out,
            amount_in,
            amount_out_min: U256::zero(), // 실제로는 파싱 필요
            path: vec![token_in, token_out],
            deadline: U256::from(chrono::Utc::now().timestamp() + 300), // 5분 후
        })
    }
    
    /// 가격 영향 계산
    async fn calculate_price_impact(&self, swap_details: &SwapDetails, dex_info: &DexInfo) -> Result<PriceImpact> {
        // 실제 구현에서는 DEX의 풀 상태를 조회하여 계산
        // 여기서는 간단한 추정치 사용
        
        let pool_size = U256::from_str_radix("1000000000000000000000", 10).unwrap(); // 1000 ETH
        let impact = (swap_details.amount_in.as_u128() as f64 / pool_size.as_u128() as f64) * 100.0;
        
        Ok(PriceImpact {
            percentage: impact,
            absolute: swap_details.amount_in,
        })
    }
    
    /// 최적 샌드위치 크기 계산
    async fn calculate_optimal_sandwich_size(&self, swap_details: &SwapDetails, price_impact: &PriceImpact) -> Result<OptimalSize> {
        // Kelly Criterion을 사용한 최적 크기 계산
        let pool_size = U256::from_str_radix("1000000000000000000000", 10).unwrap();
        let max_size = pool_size / U256::from(100); // 풀의 1%
        
        let optimal_size = if price_impact.percentage > 5.0 {
            // 큰 가격 영향이 예상되는 경우 보수적으로 접근
            swap_details.amount_in / U256::from(10)
        } else {
            // 작은 가격 영향의 경우 더 적극적으로 접근
            swap_details.amount_in / U256::from(5)
        };
        
        let final_size = std::cmp::min(optimal_size, max_size);
        
        Ok(OptimalSize {
            amount: final_size,
            confidence: 0.8,
        })
    }
    
    /// 프론트런 트랜잭션 생성
    async fn create_front_run_transaction(
        &self,
        swap_details: &SwapDetails,
        optimal_size: &OptimalSize,
        target_gas_price: U256,
        dex_info: &DexInfo,
    ) -> Result<TransactionRequest> {
        let gas_price = std::cmp::min(
            target_gas_price * U256::from((self.gas_multiplier * 100.0) as u64) / U256::from(100),
            self.max_gas_price
        );
        
        let mut data = dex_info.swap_function.clone();
        
        // 실제 구현에서는 ABI 인코딩을 사용
        // 여기서는 간단한 예시
        data.extend_from_slice(&optimal_size.amount.to_be_bytes());
        data.extend_from_slice(&swap_details.amount_out_min.to_be_bytes());
        data.extend_from_slice(&swap_details.path[0].to_fixed_bytes());
        data.extend_from_slice(&swap_details.path[1].to_fixed_bytes());
        data.extend_from_slice(&swap_details.deadline.to_be_bytes());
        
        Ok(TransactionRequest::new()
            .to(dex_info.router_address)
            .value(optimal_size.amount)
            .gas_price(gas_price)
            .data(Bytes::from(data)))
    }
    
    /// 백런 트랜잭션 생성
    async fn create_back_run_transaction(
        &self,
        swap_details: &SwapDetails,
        optimal_size: &OptimalSize,
        target_gas_price: U256,
        dex_info: &DexInfo,
    ) -> Result<TransactionRequest> {
        let gas_price = std::cmp::min(
            target_gas_price * U256::from((self.gas_multiplier * 100.0) as u64) / U256::from(100),
            self.max_gas_price
        );
        
        let mut data = dex_info.swap_function.clone();
        
        // 백런에서는 토큰을 다시 ETH로 스왑
        data.extend_from_slice(&optimal_size.amount.to_be_bytes());
        data.extend_from_slice(&U256::zero().to_be_bytes()); // 최소 출력량
        data.extend_from_slice(&swap_details.path[1].to_fixed_bytes()); // 토큰
        data.extend_from_slice(&swap_details.path[0].to_fixed_bytes()); // ETH
        data.extend_from_slice(&swap_details.deadline.to_be_bytes());
        
        Ok(TransactionRequest::new()
            .to(dex_info.router_address)
            .value(U256::zero())
            .gas_price(gas_price)
            .data(Bytes::from(data)))
    }
    
    /// 샌드위치 수익 계산
    async fn calculate_sandwich_profit(
        &self,
        front_run_tx: &TransactionRequest,
        back_run_tx: &TransactionRequest,
        swap_details: &SwapDetails,
        optimal_size: &OptimalSize,
    ) -> Result<(U256, U256, U256)> {
        // 가스 비용 계산
        let front_run_gas = U256::from(300_000u64);
        let back_run_gas = U256::from(300_000u64);
        let total_gas = front_run_gas + back_run_gas;
        
        let gas_cost = total_gas * front_run_tx.gas_price.unwrap_or(U256::zero());
        
        // 예상 수익 계산 (간단한 추정)
        let price_impact = (optimal_size.amount.as_u128() as f64 / 1_000_000_000_000_000_000_000.0) * 2.0; // 2% 가격 변동
        let expected_profit = optimal_size.amount * U256::from((price_impact * 100.0) as u64) / U256::from(100);
        
        let net_profit = if expected_profit > gas_cost {
            expected_profit - gas_cost
        } else {
            U256::zero()
        };
        
        Ok((expected_profit, gas_cost, net_profit))
    }
    
    /// 성공 확률 계산
    async fn calculate_success_probability(&self, target_tx: &Transaction, net_profit: &U256) -> Result<f64> {
        // 여러 요인을 고려한 성공 확률 계산
        
        // 1. 가스 가격 경쟁
        let gas_competition_factor = if target_tx.gas_price < U256::from(20_000_000_000u64) {
            0.8 // 낮은 가스 가격 = 낮은 경쟁
        } else {
            0.4 // 높은 가스 가격 = 높은 경쟁
        };
        
        // 2. 수익성
        let profitability_factor = if net_profit > U256::from_str_radix("500000000000000000", 10).unwrap() {
            0.9 // 높은 수익
        } else {
            0.6 // 낮은 수익
        };
        
        // 3. 트랜잭션 크기
        let size_factor = if target_tx.value > U256::from_str_radix("5000000000000000000", 10).unwrap() {
            0.8 // 큰 거래
        } else {
            0.5 // 작은 거래
        };
        
        // 4. 네트워크 혼잡도 (간단한 추정)
        let network_factor = 0.7; // 실제로는 네트워크 상태를 조회해야 함
        
        let total_probability = gas_competition_factor * profitability_factor * size_factor * network_factor;
        
        Ok(total_probability)
    }
    
    /// 통계 업데이트
    async fn update_stats(&self, opportunities_found: usize, profit: Option<U256>) {
        let mut stats = self.stats.lock().await;
        stats.transactions_analyzed += 1;
        stats.opportunities_found += opportunities_found as u64;
        stats.last_analysis_time = Some(Instant::now());
        
        if let Some(profit) = profit {
            stats.successful_sandwiches += 1;
            stats.total_profit += profit;
            stats.avg_profit_per_sandwich = stats.total_profit / U256::from(stats.successful_sandwiches);
        }
    }
}

#[derive(Debug, Clone)]
struct SwapDetails {
    token_in: H160,
    token_out: H160,
    amount_in: U256,
    amount_out_min: U256,
    path: Vec<H160>,
    deadline: U256,
}

#[derive(Debug, Clone)]
struct PriceImpact {
    percentage: f64,
    absolute: U256,
}

#[derive(Debug, Clone)]
struct OptimalSize {
    amount: U256,
    confidence: f64,
}

#[async_trait]
impl Strategy for RealTimeSandwichStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Sandwich
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    async fn start(&mut self) -> Result<()> {
        self.enabled = true;
        info!("🚀 샌드위치 전략 시작됨");
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        self.enabled = false;
        info!("⏹️ 샌드위치 전략 중지됨");
        Ok(())
    }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(vec![]);
        }
        
        let start_time = Instant::now();
        let mut opportunities = Vec::new();
        
        // 샌드위치 대상인지 확인
        if !self.is_sandwich_target(transaction) {
            return Ok(opportunities);
        }
        
        // 샌드위치 기회 분석
        if let Some(sandwich_opp) = self.analyze_sandwich_opportunity(transaction).await? {
            let opportunity = Opportunity {
                id: format!("sandwich_{}", transaction.hash),
                strategy: StrategyType::Sandwich,
                transaction_hash: transaction.hash,
                expected_profit: sandwich_opp.net_profit,
                gas_cost: sandwich_opp.gas_cost,
                net_profit: sandwich_opp.net_profit,
                success_probability: sandwich_opp.success_probability,
                details: ArbitrageDetails {
                    token_in: sandwich_opp.target_tx.to.unwrap_or(H160::zero()),
                    token_out: H160::zero(), // ETH
                    amount_in: sandwich_opp.front_run_tx.value.unwrap_or(U256::zero()),
                    amount_out: sandwich_opp.expected_profit,
                    dex_a: "Sandwich".to_string(),
                    dex_b: "Sandwich".to_string(),
                    price_a: U256::zero(),
                    price_b: U256::zero(),
                },
                timestamp: chrono::Utc::now(),
            };
            
            opportunities.push(opportunity);
        }
        
        // 통계 업데이트
        self.update_stats(opportunities.len(), None).await;
        
        let duration = start_time.elapsed();
        debug!("🥪 샌드위치 분석 완료: {:.2}ms, {}개 기회", duration.as_millis(), opportunities.len());
        
        Ok(opportunities)
    }
    
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        // 샌드위치 기회 검증
        if opportunity.strategy != StrategyType::Sandwich {
            return Ok(false);
        }
        
        // 수익성 재검증
        if opportunity.net_profit < self.min_profit_eth {
            return Ok(false);
        }
        
        // 가스 가격 검증
        let current_gas_price = self.provider.get_gas_price().await?;
        if current_gas_price > self.max_gas_price {
            return Ok(false);
        }
        
        // 성공 확률 검증
        if opportunity.success_probability < 0.3 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle> {
        // 샌드위치 번들 생성
        // 실제 구현에서는 프론트런과 백런 트랜잭션을 포함한 번들 생성
        
        let bundle = Bundle {
            id: format!("sandwich_bundle_{}", opportunity.id),
            transactions: vec![], // 실제 트랜잭션들로 채워야 함
            target_block: 0, // 실제 타겟 블록으로 설정
            max_gas_price: self.max_gas_price,
            min_timestamp: 0,
            max_timestamp: 0,
            refund_recipient: H160::zero(),
            refund_percentage: 0,
        };
        
        Ok(bundle)
    }
}

impl std::fmt::Debug for RealTimeSandwichStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealTimeSandwichStrategy")
            .field("enabled", &self.enabled)
            .field("dex_count", &self.dex_addresses.len())
            .field("min_profit_eth", &self.min_profit_eth)
            .field("min_profit_percentage", &self.min_profit_percentage)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transaction, H256, H160, U256};
    use chrono::Utc;

    #[tokio::test]
    async fn test_sandwich_strategy_creation() {
        let config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let strategy = RealTimeSandwichStrategy::new(config, provider).await;
        // assert!(strategy.is_ok());
    }

    #[test]
    fn test_sandwich_target_detection() {
        let config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let strategy = RealTimeSandwichStrategy::new(config, provider).await.unwrap();
        
        // 샌드위치 대상 트랜잭션
        let target_tx = Transaction {
            hash: H256::zero(),
            from: H160::zero(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()), // Uniswap V2
            value: U256::from_str_radix("5000000000000000000", 10).unwrap(), // 5 ETH
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            gas_limit: U256::from(300_000u64),
            data: vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00, 0x00, 0x00], // swapExactTokensForTokens
            nonce: 0,
            timestamp: Utc::now(),
            block_number: Some(1000),
        };
        
        // assert!(strategy.is_sandwich_target(&target_tx));
    }
}
