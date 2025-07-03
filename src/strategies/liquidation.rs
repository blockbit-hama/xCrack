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

/// 경쟁적 청산 프론트런 전략
/// 
/// Aave, Compound 등의 대출 프로토콜에서 청산 가능한 포지션을 감지하고,
/// 다른 청산자들보다 먼저 청산을 실행하여 보상을 획득합니다.
pub struct CompetitiveLiquidationStrategy {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    enabled: bool,
    
    // 청산 대상 프로토콜 정보
    lending_protocols: HashMap<H160, LendingProtocolInfo>,
    
    // 최소 수익성 임계값
    min_profit_eth: U256,
    min_liquidation_amount: U256,
    
    // 가스 가격 전략
    gas_multiplier: f64,
    max_gas_price: U256,
    
    // 청산 조건
    health_factor_threshold: f64,
    max_liquidation_size: U256,
    
    // 통계
    stats: Arc<Mutex<LiquidationStats>>,
}

#[derive(Debug, Clone)]
struct LendingProtocolInfo {
    name: String,
    lending_pool_address: H160,
    liquidation_function: Vec<u8>,
    liquidation_fee: u32, // basis points (e.g., 500 = 5%)
    min_health_factor: f64,
    supported_tokens: Vec<H160>,
}

#[derive(Debug, Clone)]
struct LiquidationStats {
    transactions_analyzed: u64,
    opportunities_found: u64,
    successful_liquidations: u64,
    total_profit: U256,
    avg_profit_per_liquidation: U256,
    last_analysis_time: Option<Instant>,
}

#[derive(Debug, Clone)]
struct LiquidationOpportunity {
    target_user: H160,
    collateral_token: H160,
    debt_token: H160,
    collateral_amount: U256,
    debt_amount: U256,
    health_factor: f64,
    liquidation_amount: U256,
    expected_reward: U256,
    gas_cost: U256,
    net_profit: U256,
    success_probability: f64,
}

#[derive(Debug, Clone)]
struct UserPosition {
    user: H160,
    collateral_token: H160,
    debt_token: H160,
    collateral_amount: U256,
    debt_amount: U256,
    health_factor: f64,
    liquidation_threshold: f64,
    liquidation_amount: U256,
}

impl CompetitiveLiquidationStrategy {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("💸 청산 전략 초기화 중...");
        
        let mut lending_protocols = HashMap::new();
        
        // Aave V2
        lending_protocols.insert(
            "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse()?,
            LendingProtocolInfo {
                name: "Aave V2".to_string(),
                lending_pool_address: "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse()?,
                liquidation_function: vec![0xe8, 0xed, 0xa9, 0xdf], // liquidationCall
                liquidation_fee: 500, // 5%
                min_health_factor: 1.0,
                supported_tokens: vec![
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                    "0xA0b86a33E6441b8C4C3132E4B4F4b4F4b4F4b4F4b".parse()?, // USDC
                    "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse()?, // USDT
                ],
            }
        );
        
        // Compound V3
        lending_protocols.insert(
            "0xc3d688B66703497DAA19211EEdff47fB25365b65".parse()?,
            LendingProtocolInfo {
                name: "Compound V3".to_string(),
                lending_pool_address: "0xc3d688B66703497DAA19211EEdff47fB25365b65".parse()?,
                liquidation_function: vec![0x4c, 0x0b, 0x5b, 0x3e], // liquidate
                liquidation_fee: 750, // 7.5%
                min_health_factor: 1.0,
                supported_tokens: vec![
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                    "0xA0b86a33E6441b8C4C3132E4B4F4b4F4b4F4b4F4b".parse()?, // USDC
                ],
            }
        );
        
        // MakerDAO
        lending_protocols.insert(
            "0x35D1b3F3D7966A1DFe207aa4514C12a259A0492B".parse()?,
            LendingProtocolInfo {
                name: "MakerDAO".to_string(),
                lending_pool_address: "0x35D1b3F3D7966A1DFe207aa4514C12a259A0492B".parse()?,
                liquidation_function: vec![0x1d, 0x26, 0x3b, 0x3c], // bite
                liquidation_fee: 1300, // 13%
                min_health_factor: 1.5,
                supported_tokens: vec![
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                ],
            }
        );
        
        let min_profit_eth = U256::from_str_radix(
            &config.strategies.liquidation.min_profit_eth,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("50000000000000000", 10).unwrap()); // 0.05 ETH
        
        let min_liquidation_amount = U256::from_str_radix(
            &config.strategies.liquidation.min_liquidation_amount,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("1000000000000000000", 10).unwrap()); // 1 ETH
        
        let gas_multiplier = config.strategies.liquidation.gas_multiplier;
        let max_gas_price = U256::from_str_radix(
            &config.strategies.liquidation.max_gas_price_gwei,
            10
        ).unwrap_or_else(|_| U256::from(200_000_000_000u64)) * U256::from(1_000_000_000u64); // gwei to wei
        
        let health_factor_threshold = config.strategies.liquidation.health_factor_threshold;
        let max_liquidation_size = U256::from_str_radix(
            &config.strategies.liquidation.max_liquidation_size,
            10
        ).unwrap_or_else(|_| U256::from_str_radix("10000000000000000000", 10).unwrap()); // 10 ETH
        
        info!("✅ 청산 전략 초기화 완료");
        info!("  📊 최소 수익: {} ETH", ethers::utils::format_ether(min_profit_eth));
        info!("  💰 최소 청산 금액: {} ETH", ethers::utils::format_ether(min_liquidation_amount));
        info!("  ⛽ 가스 배수: {:.2}x", gas_multiplier);
        info!("  🔥 최대 가스 가격: {} gwei", max_gas_price / U256::from(1_000_000_000u64));
        info!("  🏥 건강도 임계값: {:.2}", health_factor_threshold);
        info!("  📈 최대 청산 크기: {} ETH", ethers::utils::format_ether(max_liquidation_size));
        
        Ok(Self {
            config,
            provider,
            enabled: true,
            lending_protocols,
            min_profit_eth,
            min_liquidation_amount,
            gas_multiplier,
            max_gas_price,
            health_factor_threshold,
            max_liquidation_size,
            stats: Arc::new(Mutex::new(LiquidationStats {
                transactions_analyzed: 0,
                opportunities_found: 0,
                successful_liquidations: 0,
                total_profit: U256::zero(),
                avg_profit_per_liquidation: U256::zero(),
                last_analysis_time: None,
            })),
        })
    }
    
    /// 트랜잭션이 청산 관련인지 확인
    fn is_liquidation_related(&self, tx: &Transaction) -> bool {
        // 1. 대출 프로토콜로의 호출인지 확인
        if let Some(to) = tx.to {
            if !self.lending_protocols.contains_key(&to) {
                return false;
            }
        } else {
            return false;
        }
        
        // 2. 청산 함수 호출인지 확인
        if tx.data.len() < 4 {
            return false;
        }
        
        let function_selector = &tx.data[0..4];
        let liquidation_functions = vec![
            vec![0xe8, 0xed, 0xa9, 0xdf], // Aave liquidationCall
            vec![0x4c, 0x0b, 0x5b, 0x3e], // Compound liquidate
            vec![0x1d, 0x26, 0x3b, 0x3c], // MakerDAO bite
        ];
        
        if !liquidation_functions.contains(function_selector) {
            return false;
        }
        
        true
    }
    
    /// 청산 기회 분석
    async fn analyze_liquidation_opportunity(&self, tx: &Transaction) -> Result<Option<LiquidationOpportunity>> {
        let protocol_info = if let Some(to) = tx.to {
            self.lending_protocols.get(&to).cloned()
        } else {
            return Ok(None);
        };
        
        let protocol_info = protocol_info.ok_or_else(|| anyhow!("대출 프로토콜 정보를 찾을 수 없습니다"))?;
        
        // 1. 청산 대상 사용자 포지션 조회
        let user_positions = self.get_liquidatable_positions(&protocol_info).await?;
        
        if user_positions.is_empty() {
            return Ok(None);
        }
        
        // 2. 가장 수익성 높은 청산 기회 선택
        let mut best_opportunity = None;
        let mut best_profit = U256::zero();
        
        for position in user_positions {
            let opportunity = self.calculate_liquidation_opportunity(&position, &protocol_info).await?;
            
            if let Some(opp) = opportunity {
                if opp.net_profit > best_profit {
                    best_profit = opp.net_profit;
                    best_opportunity = Some(opp);
                }
            }
        }
        
        if let Some(opportunity) = best_opportunity {
            // 3. 수익성 검증
            if opportunity.net_profit < self.min_profit_eth {
                debug!("❌ 청산 수익이 너무 낮음: {} ETH", ethers::utils::format_ether(opportunity.net_profit));
                return Ok(None);
            }
            
            if opportunity.liquidation_amount < self.min_liquidation_amount {
                debug!("❌ 청산 금액이 너무 작음: {} ETH", ethers::utils::format_ether(opportunity.liquidation_amount));
                return Ok(None);
            }
            
            // 4. 성공 확률 계산
            let success_probability = self.calculate_liquidation_success_probability(&opportunity, tx).await?;
            
            if success_probability < 0.4 {
                debug!("❌ 청산 성공 확률이 너무 낮음: {:.2}%", success_probability * 100.0);
                return Ok(None);
            }
            
            info!("💸 청산 기회 발견!");
            info!("  👤 대상 사용자: {}", opportunity.target_user);
            info!("  💰 청산 금액: {} ETH", ethers::utils::format_ether(opportunity.liquidation_amount));
            info!("  📊 예상 수익: {} ETH", ethers::utils::format_ether(opportunity.net_profit));
            info!("  🏥 건강도: {:.2}", opportunity.health_factor);
            info!("  🎲 성공 확률: {:.2}%", success_probability * 100.0);
            
            return Ok(Some(opportunity));
        }
        
        Ok(None)
    }
    
    /// 청산 가능한 포지션 조회
    async fn get_liquidatable_positions(&self, protocol_info: &LendingProtocolInfo) -> Result<Vec<UserPosition>> {
        // 실제 구현에서는 프로토콜의 상태를 조회하여 청산 가능한 포지션을 찾아야 함
        // 여기서는 샘플 데이터로 구현
        
        let mut positions = Vec::new();
        
        // 샘플 청산 가능한 포지션들
        let sample_users = vec![
            "0x742d35Cc6570000000000000000000000000001",
            "0x742d35Cc6570000000000000000000000000002",
            "0x742d35Cc6570000000000000000000000000003",
        ];
        
        for user_addr in sample_users {
            let user: H160 = user_addr.parse()?;
            
            // 실제로는 프로토콜에서 사용자 포지션을 조회
            let position = UserPosition {
                user,
                collateral_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
                debt_token: "0xA0b86a33E6441b8C4C3132E4B4F4b4F4b4F4b4F4b".parse()?, // USDC
                collateral_amount: U256::from_str_radix("5000000000000000000", 10).unwrap(), // 5 ETH
                debt_amount: U256::from_str_radix("8000000000", 10).unwrap(), // 8000 USDC
                health_factor: 0.95, // 청산 임계값 아래
                liquidation_threshold: 0.8,
                liquidation_amount: U256::from_str_radix("2000000000000000000", 10).unwrap(), // 2 ETH
            };
            
            if position.health_factor < self.health_factor_threshold {
                positions.push(position);
            }
        }
        
        Ok(positions)
    }
    
    /// 청산 기회 계산
    async fn calculate_liquidation_opportunity(
        &self,
        position: &UserPosition,
        protocol_info: &LendingProtocolInfo,
    ) -> Result<Option<LiquidationOpportunity>> {
        // 1. 청산 금액 계산
        let liquidation_amount = std::cmp::min(
            position.liquidation_amount,
            self.max_liquidation_size
        );
        
        // 2. 청산 보상 계산
        let liquidation_fee_bps = protocol_info.liquidation_fee as f64 / 10000.0;
        let expected_reward = liquidation_amount * U256::from((liquidation_fee_bps * 10000.0) as u64) / U256::from(10000);
        
        // 3. 가스 비용 계산
        let gas_limit = U256::from(500_000u64); // 청산 트랜잭션은 가스가 많이 필요
        let current_gas_price = self.provider.get_gas_price().await?;
        let gas_cost = gas_limit * current_gas_price;
        
        // 4. 순수익 계산
        let net_profit = if expected_reward > gas_cost {
            expected_reward - gas_cost
        } else {
            U256::zero()
        };
        
        if net_profit == U256::zero() {
            return Ok(None);
        }
        
        Ok(Some(LiquidationOpportunity {
            target_user: position.user,
            collateral_token: position.collateral_token,
            debt_token: position.debt_token,
            collateral_amount: position.collateral_amount,
            debt_amount: position.debt_amount,
            health_factor: position.health_factor,
            liquidation_amount,
            expected_reward,
            gas_cost,
            net_profit,
            success_probability: 0.0, // 나중에 계산
        }))
    }
    
    /// 청산 성공 확률 계산
    async fn calculate_liquidation_success_probability(
        &self,
        opportunity: &LiquidationOpportunity,
        competing_tx: &Transaction,
    ) -> Result<f64> {
        // 여러 요인을 고려한 성공 확률 계산
        
        // 1. 가스 가격 경쟁
        let gas_competition_factor = if competing_tx.gas_price < U256::from(50_000_000_000u64) {
            0.8 // 낮은 가스 가격 = 낮은 경쟁
        } else {
            0.3 // 높은 가스 가격 = 높은 경쟁
        };
        
        // 2. 청산 금액 크기
        let size_factor = if opportunity.liquidation_amount > U256::from_str_radix("5000000000000000000", 10).unwrap() {
            0.9 // 큰 청산 = 높은 보상
        } else {
            0.6 // 작은 청산 = 낮은 보상
        };
        
        // 3. 건강도 (낮을수록 더 긴급)
        let health_factor = if opportunity.health_factor < 0.8 {
            0.9 // 매우 낮은 건강도
        } else if opportunity.health_factor < 0.9 {
            0.7 // 낮은 건강도
        } else {
            0.5 // 경계선 건강도
        };
        
        // 4. 네트워크 혼잡도
        let network_factor = 0.7; // 실제로는 네트워크 상태를 조회해야 함
        
        // 5. 프로토콜별 경쟁 정도
        let protocol_factor = 0.8; // 실제로는 프로토콜별 통계를 조회해야 함
        
        let total_probability = gas_competition_factor * size_factor * health_factor * network_factor * protocol_factor;
        
        Ok(total_probability)
    }
    
    /// 청산 트랜잭션 생성
    async fn create_liquidation_transaction(
        &self,
        opportunity: &LiquidationOpportunity,
        protocol_info: &LendingProtocolInfo,
    ) -> Result<TransactionRequest> {
        let gas_price = std::cmp::min(
            U256::from(100_000_000_000u64) * U256::from((self.gas_multiplier * 100.0) as u64) / U256::from(100),
            self.max_gas_price
        );
        
        let mut data = protocol_info.liquidation_function.clone();
        
        // 실제 구현에서는 ABI 인코딩을 사용
        // 여기서는 간단한 예시
        data.extend_from_slice(&opportunity.target_user.to_fixed_bytes());
        data.extend_from_slice(&opportunity.collateral_token.to_fixed_bytes());
        data.extend_from_slice(&opportunity.debt_token.to_fixed_bytes());
        data.extend_from_slice(&opportunity.liquidation_amount.to_be_bytes());
        data.extend_from_slice(&[0u8; 32]); // receiveAToken flag
        
        Ok(TransactionRequest::new()
            .to(protocol_info.lending_pool_address)
            .value(U256::zero())
            .gas_price(gas_price)
            .data(Bytes::from(data)))
    }
    
    /// 통계 업데이트
    async fn update_stats(&self, opportunities_found: usize, profit: Option<U256>) {
        let mut stats = self.stats.lock().await;
        stats.transactions_analyzed += 1;
        stats.opportunities_found += opportunities_found as u64;
        stats.last_analysis_time = Some(Instant::now());
        
        if let Some(profit) = profit {
            stats.successful_liquidations += 1;
            stats.total_profit += profit;
            stats.avg_profit_per_liquidation = stats.total_profit / U256::from(stats.successful_liquidations);
        }
    }
}

#[async_trait]
impl Strategy for CompetitiveLiquidationStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Liquidation
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    async fn start(&mut self) -> Result<()> {
        self.enabled = true;
        info!("🚀 청산 전략 시작됨");
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        self.enabled = false;
        info!("⏹️ 청산 전략 중지됨");
        Ok(())
    }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(vec![]);
        }
        
        let start_time = Instant::now();
        let mut opportunities = Vec::new();
        
        // 청산 관련 트랜잭션인지 확인
        if !self.is_liquidation_related(transaction) {
            return Ok(opportunities);
        }
        
        // 청산 기회 분석
        if let Some(liquidation_opp) = self.analyze_liquidation_opportunity(transaction).await? {
            let opportunity = Opportunity {
                id: format!("liquidation_{}", transaction.hash),
                strategy: StrategyType::Liquidation,
                transaction_hash: transaction.hash,
                expected_profit: liquidation_opp.net_profit,
                gas_cost: liquidation_opp.gas_cost,
                net_profit: liquidation_opp.net_profit,
                success_probability: liquidation_opp.success_probability,
                details: ArbitrageDetails {
                    token_in: liquidation_opp.collateral_token,
                    token_out: liquidation_opp.debt_token,
                    amount_in: liquidation_opp.liquidation_amount,
                    amount_out: liquidation_opp.expected_reward,
                    dex_a: "Liquidation".to_string(),
                    dex_b: "Liquidation".to_string(),
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
        debug!("💸 청산 분석 완료: {:.2}ms, {}개 기회", duration.as_millis(), opportunities.len());
        
        Ok(opportunities)
    }
    
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        // 청산 기회 검증
        if opportunity.strategy != StrategyType::Liquidation {
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
        if opportunity.success_probability < 0.4 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle> {
        // 청산 번들 생성
        // 실제 구현에서는 청산 트랜잭션을 포함한 번들 생성
        
        let bundle = Bundle {
            id: format!("liquidation_bundle_{}", opportunity.id),
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

impl std::fmt::Debug for CompetitiveLiquidationStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompetitiveLiquidationStrategy")
            .field("enabled", &self.enabled)
            .field("protocol_count", &self.lending_protocols.len())
            .field("min_profit_eth", &self.min_profit_eth)
            .field("min_liquidation_amount", &self.min_liquidation_amount)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transaction, H256, H160, U256};
    use chrono::Utc;

    #[tokio::test]
    async fn test_liquidation_strategy_creation() {
        let config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let strategy = CompetitiveLiquidationStrategy::new(config, provider).await;
        // assert!(strategy.is_ok());
    }

    #[test]
    fn test_liquidation_target_detection() {
        let config = Arc::new(Config::default());
        // 실제 테스트에서는 더미 프로바이더가 필요
        // let provider = Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()));
        // let strategy = CompetitiveLiquidationStrategy::new(config, provider).await.unwrap();
        
        // 청산 관련 트랜잭션
        let liquidation_tx = Transaction {
            hash: H256::zero(),
            from: H160::zero(),
            to: Some("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap()), // Aave V2
            value: U256::zero(),
            gas_price: U256::from(100_000_000_000u64), // 100 gwei
            gas_limit: U256::from(500_000u64),
            data: vec![0xe8, 0xed, 0xa9, 0xdf, 0x00, 0x00, 0x00, 0x00], // liquidationCall
            nonce: 0,
            timestamp: Utc::now(),
            block_number: Some(1000),
        };
        
        // assert!(strategy.is_liquidation_related(&liquidation_tx));
    }
}
