use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn};
use alloy::primitives::{Address, U256, Bytes};
use ethers::providers::{Provider, Ws};
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::dex::{DexAggregator, SwapQuote, SwapParams, ZeroXAggregator, OneInchAggregator, DexType};
use crate::protocols::{
    MultiProtocolScanner, LiquidatableUser, ProtocolType,
};
use crate::utils::profitability::{
    ProfitabilityCalculator, LiquidationProfitabilityAnalysis, LiquidationStrategy as ProfitabilityStrategy,
};
use crate::execution::transaction_builder::TransactionBuilder;
use crate::mev::opportunity::{Opportunity, MEVStrategy};

/// 새로운 청산 전략 - 실제 프로토콜 상태 기반
pub struct LiquidationStrategyV2 {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    protocol_scanner: Arc<MultiProtocolScanner>,
    profitability_calculator: ProfitabilityCalculator,
    dex_aggregators: HashMap<DexType, Box<dyn DexAggregator>>,
    transaction_builder: TransactionBuilder,
    liquidation_contract: Address,
    eth_price_cache: Arc<tokio::sync::RwLock<(f64, chrono::DateTime<chrono::Utc>)>>,
}

#[derive(Debug, Clone)]
pub struct LiquidationOpportunity {
    pub user: LiquidatableUser,
    pub strategy: ProfitabilityStrategy,
    pub profitability_analysis: LiquidationProfitabilityAnalysis,
    pub execution_transaction: Option<Bytes>,
    pub estimated_execution_time: Duration,
    pub confidence_score: f64,
}

impl LiquidationStrategyV2 {
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        protocol_scanner: Arc<MultiProtocolScanner>,
    ) -> Result<Self> {
        info!("💰 Initializing Liquidation Strategy v2...");
        
        let profitability_calculator = ProfitabilityCalculator::new((*config).clone());
        
        // DEX Aggregator 초기화
        let mut dex_aggregators: HashMap<DexType, Box<dyn DexAggregator>> = HashMap::new();
        
        // 0x Protocol
        if let Some(ref api_key) = config.dex.ox_api_key {
            let ox_aggregator = ZeroXAggregator::new(Some(api_key.clone()), config.network.chain_id);
            dex_aggregators.insert(DexType::ZeroX, Box::new(ox_aggregator));
        }
        
        // 1inch Protocol  
        if let Some(ref api_key) = config.dex.oneinch_api_key {
            let oneinch_aggregator = OneInchAggregator::new(Some(api_key.clone()), config.network.chain_id);
            dex_aggregators.insert(DexType::OneInch, Box::new(oneinch_aggregator));
        }
        
        let transaction_builder = TransactionBuilder::new(Arc::clone(&provider), Arc::clone(&config)).await?;
        
        // 청산 컨트랙트 주소 (mainnet)
        let liquidation_contract: Address = config.contracts.liquidation_strategy
            .as_ref()
            .and_then(|addr| addr.parse().ok())
            .unwrap_or_else(|| "0x0000000000000000000000000000000000000000".parse().unwrap()); // TODO: 배포 후 업데이트
        
        let eth_price_cache = Arc::new(tokio::sync::RwLock::new((3000.0, chrono::Utc::now())));
        
        info!("✅ Liquidation Strategy v2 initialized with {} DEX aggregators", dex_aggregators.len());
        
        Ok(Self {
            config,
            provider,
            protocol_scanner,
            profitability_calculator,
            dex_aggregators,
            transaction_builder,
            liquidation_contract,
            eth_price_cache,
        })
    }
    
    /// 메인 기회 탐지 함수 - 프로토콜 상태 기반
    pub async fn detect_opportunities(&self) -> Result<Vec<LiquidationOpportunity>> {
        info!("🔍 Starting liquidation opportunity detection...");
        let start_time = std::time::Instant::now();
        
        // 1. 모든 프로토콜에서 청산 대상자 스캔
        let liquidatable_users = self.protocol_scanner.scan_all_protocols().await?;
        let total_users: usize = liquidatable_users.values().map(|users| users.len()).sum();
        
        if total_users == 0 {
            debug!("📭 No liquidatable users found");
            return Ok(Vec::new());
        }
        
        info!("👥 Found {} liquidatable users across {} protocols", total_users, liquidatable_users.len());
        
        // 2. ETH 가격 업데이트
        self.update_eth_price().await?;
        let eth_price = self.eth_price_cache.read().await.0;
        
        // 3. 각 사용자에 대해 수익성 분석
        let mut opportunities = Vec::new();
        
        for (protocol_type, users) in liquidatable_users {
            debug!("🔬 Analyzing {} {} users", users.len(), protocol_type);
            
            for user in users {
                // 높은 우선순위 사용자만 분석 (성능 최적화)
                if user.priority_score < 1000.0 {
                    continue;
                }
                
                match self.analyze_user_profitability(&user, eth_price).await {
                    Ok(Some(opportunity)) => {
                        opportunities.push(opportunity);
                    }
                    Ok(None) => {
                        debug!("💸 User {} not profitable", user.address);
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to analyze user {}: {}", user.address, e);
                    }
                }
                
                // Rate limiting
                sleep(Duration::from_millis(10)).await;
            }
        }
        
        // 4. 수익성 순으로 정렬
        opportunities.sort_by(|a, b| {
            b.strategy.net_profit_usd.partial_cmp(&a.strategy.net_profit_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        let duration = start_time.elapsed();
        info!("✅ Opportunity detection complete: {} opportunities found in {}ms", 
              opportunities.len(), duration.as_millis());
        
        Ok(opportunities)
    }
    
    /// 개별 사용자 수익성 분석
    async fn analyze_user_profitability(
        &self,
        user: &LiquidatableUser,
        eth_price: f64,
    ) -> Result<Option<LiquidationOpportunity>> {
        debug!("💹 Analyzing profitability for user {}", user.address);
        
        // 1. 필요한 스왑 경로의 견적 수집
        let swap_quotes = self.collect_swap_quotes(user).await?;
        
        if swap_quotes.is_empty() {
            debug!("🚫 No swap routes available for user {}", user.address);
            return Ok(None);
        }
        
        // 2. 수익성 분석 실행
        let profitability_analysis = self.profitability_calculator
            .analyze_liquidation_profitability(user, &swap_quotes, eth_price)
            .await?;
        
        // 3. 수익성이 있는 경우에만 기회로 생성
        if !profitability_analysis.is_profitable {
            return Ok(None);
        }
        
        let best_strategy = profitability_analysis.best_strategy.as_ref().unwrap().clone();
        
        // 4. 실행 트랜잭션 생성
        let execution_transaction = self.build_execution_transaction(
            user,
            &best_strategy,
            &profitability_analysis,
        ).await.ok();
        
        // 5. 신뢰도 점수 계산
        let confidence_score = self.calculate_confidence_score(user, &best_strategy, &swap_quotes);
        // 6. 실행 시간 추정
        let estimated_execution_time = Duration::from_millis(
            best_strategy.execution_time_estimate_ms + 1000 // 안전 마진
        );
        
        let opportunity = LiquidationOpportunity {
            user: user.clone(),
            strategy: best_strategy.clone(),
            profitability_analysis,
            execution_transaction,
            estimated_execution_time,
            confidence_score,
        };
        
        info!("💰 Profitable opportunity found: User {}, Profit ${:.2} ({:.2}%)", 
              user.address, best_strategy.net_profit_usd, best_strategy.profit_margin_percent);
        
        Ok(Some(opportunity))
    }
    
    /// 스왑 견적 수집
    async fn collect_swap_quotes(&self, user: &LiquidatableUser) -> Result<HashMap<(Address, Address), Vec<SwapQuote>>> {
        let mut swap_quotes = HashMap::new();
        
        // 각 담보-부채 쌍에 대해 스왑 견적 수집
        for collateral_position in &user.collateral_positions {
            for debt_position in &user.debt_positions {
                let collateral_asset = collateral_position.asset;
                let debt_asset = debt_position.asset;
                
                let max_liquidatable = user.max_liquidatable_debt.get(&debt_asset).copied()
                    .unwrap_or(debt_position.amount);
                
                // 청산 보너스를 고려한 예상 담보 획득량 계산
                let liquidation_bonus = user.liquidation_bonus.get(&debt_asset).copied().unwrap_or(0.05);
                let expected_collateral_amount = max_liquidatable * U256::from((1.05 * 1e18) as u128) / U256::from(1e18 as u128);
                
                // 각 DEX에서 견적 수집
                let mut quotes_for_pair = Vec::new();
                
                for (dex_type, aggregator) in &self.dex_aggregators {
                    let swap_params = SwapParams {
                        sell_token: collateral_asset,
                        buy_token: debt_asset,
                        sell_amount: expected_collateral_amount,
                        slippage_tolerance: 0.005, // 0.5%
                        recipient: Some(self.liquidation_contract),
                        deadline_seconds: Some(300), // 5분
                        exclude_sources: vec![],
                        include_sources: vec![],
                        fee_recipient: None,
                        buy_token_percentage_fee: None,
                    };
                    
                    match aggregator.get_quote(swap_params).await {
                        Ok(quote) => {
                            debug!("📊 Got quote from {:?}: {} -> {} (impact: {:.2}%)", 
                                   dex_type, collateral_asset, debt_asset, quote.price_impact * 100.0);
                            quotes_for_pair.push(quote);
                        }
                        Err(e) => {
                            debug!("❌ Failed to get quote from {:?}: {}", dex_type, e);
                        }
                    }
                    
                    // Rate limiting
                    sleep(Duration::from_millis(100)).await;
                }
                
                if !quotes_for_pair.is_empty() {
                    swap_quotes.insert((collateral_asset, debt_asset), quotes_for_pair);
                }
            }
        }
        
        debug!("📈 Collected quotes for {} asset pairs", swap_quotes.len());
        Ok(swap_quotes)
    }
    
    /// 실행 트랜잭션 구축
    async fn build_execution_transaction(
        &self,
        user: &LiquidatableUser,
        strategy: &ProfitabilityStrategy,
        analysis: &LiquidationProfitabilityAnalysis,
    ) -> Result<Bytes> {
        debug!("🔨 Building execution transaction for user {}", user.address);
        
        // LiquidationStrategy.sol의 executeLiquidation 함수 호출 데이터 생성
        let liquidation_params = self.encode_liquidation_params(user, strategy)?;
        
        let calldata = self.transaction_builder.encode_liquidation_call(
            strategy.debt_asset,
            strategy.liquidation_amount,
            liquidation_params,
        ).await?;
        
        debug!("✅ Transaction built successfully, calldata length: {}", calldata.len());
        Ok(calldata)
    }
    
    /// 청산 파라미터 인코딩
    fn encode_liquidation_params(&self, user: &LiquidatableUser, strategy: &ProfitabilityStrategy) -> Result<Vec<u8>> {
        // Solidity struct LiquidationParams를 인코딩
        // 실제 구현에서는 ethers-rs의 ABI 인코딩 사용
        
        // 임시 구현
        let mut params = Vec::new();
        params.extend_from_slice(user.address.as_slice());
        params.extend_from_slice(strategy.collateral_asset.as_slice());
        params.extend_from_slice(&strategy.liquidation_amount.to_be_bytes::<32>());
        
        Ok(params)
    }
    
    /// 신뢰도 점수 계산
    fn calculate_confidence_score(
        &self,
        user: &LiquidatableUser,
        strategy: &ProfitabilityStrategy,
        swap_quotes: &HashMap<(Address, Address), Vec<SwapQuote>>,
    ) -> f64 {
        let mut confidence = 1.0;
        
        // 1. Health Factor 기반 신뢰도
        if user.account_data.health_factor > 1.02 {
            confidence *= 0.8; // HF가 너무 높으면 신뢰도 하락
        }
        
        // 2. 수익 마진 기반
        if strategy.profit_margin_percent < 10.0 {
            confidence *= 0.9; // 낮은 마진
        } else if strategy.profit_margin_percent > 25.0 {
            confidence *= 1.1; // 높은 마진
        }
        
        // 3. 슬리피지 위험
        if strategy.swap_route.price_impact_percent > 1.0 {
            confidence *= 0.85; // 높은 슬리피지
        }
        
        // 4. 스왑 경로 다양성
        let quote_count = swap_quotes.values().map(|quotes| quotes.len()).sum::<usize>();
        if quote_count > 3 {
            confidence *= 1.05; // 많은 선택지
        }
        
        // 5. 청산 금액 크기
        if strategy.liquidation_amount_usd > 50_000.0 {
            confidence *= 0.95; // 큰 금액은 위험
        }
        
        confidence.min(1.0).max(0.0)
    }
    
    /// ETH 가격 업데이트
    async fn update_eth_price(&self) -> Result<()> {
        let mut cache = self.eth_price_cache.write().await;
        let (cached_price, cached_time) = *cache;
        
        // 5분마다 업데이트
        if chrono::Utc::now().signed_duration_since(cached_time).num_minutes() < 5 {
            return Ok(());
        }
        
        // 간단한 ETH 가격 조회 (실제로는 오라클이나 DEX에서)
        let eth_price = self.fetch_eth_price().await.unwrap_or(cached_price);
        
        *cache = (eth_price, chrono::Utc::now());
        debug!("💱 ETH price updated: ${:.2}", eth_price);
        
        Ok(())
    }
    
    /// ETH 가격 조회 (단순화된 구현)
    async fn fetch_eth_price(&self) -> Result<f64> {
        // 실제로는 Chainlink 오라클이나 DEX에서 가져옴
        // 여기서는 임시로 고정값 반환
        Ok(3000.0)
    }
    
    /// 최고 우선순위 기회 반환
    pub async fn get_top_opportunity(&self) -> Result<Option<LiquidationOpportunity>> {
        let opportunities = self.detect_opportunities().await?;
        Ok(opportunities.into_iter().next())
    }
    
    /// 특정 사용자의 청산 기회 분석
    pub async fn analyze_specific_user(&self, user_address: Address) -> Result<Option<LiquidationOpportunity>> {
        debug!("🎯 Analyzing specific user: {}", user_address);
        
        if let Some(user) = self.protocol_scanner.get_user_data(user_address).await? {
            let eth_price = self.eth_price_cache.read().await.0;
            self.analyze_user_profitability(&user, eth_price).await
        } else {
            Ok(None)
        }
    }
    
    /// 청산 실행 (실제 트랜잭션 전송)
    pub async fn execute_liquidation(&self, opportunity: &LiquidationOpportunity) -> Result<String> {
        info!("⚡ Executing liquidation for user {} with ${:.2} profit", 
              opportunity.user.address, opportunity.strategy.net_profit_usd);
        
        if let Some(ref calldata) = opportunity.execution_transaction {
            // 실제 구현에서는 MEV bundle로 전송하거나 직접 전송
            let tx_hash = self.transaction_builder.send_liquidation_transaction(
                self.liquidation_contract,
                calldata.clone(),
                opportunity.strategy.cost_breakdown.gas_cost_usd,
            ).await?;
            
            info!("🚀 Liquidation executed: {}", tx_hash);
            Ok(tx_hash)
        } else {
            Err(anyhow!("No execution transaction prepared"))
        }
    }
    
    /// 전략 통계
    pub async fn get_strategy_stats(&self) -> Result<LiquidationStrategyStats> {
        let opportunities = self.detect_opportunities().await?;
        
        let total_opportunities = opportunities.len();
        let total_profit_potential = opportunities.iter()
            .map(|opp| opp.strategy.net_profit_usd)
            .sum::<f64>();
        
        let avg_profit_margin = if !opportunities.is_empty() {
            opportunities.iter()
                .map(|opp| opp.strategy.profit_margin_percent)
                .sum::<f64>() / opportunities.len() as f64
        } else {
            0.0
        };
        
        let protocol_breakdown = self.calculate_protocol_breakdown(&opportunities);
        
        Ok(LiquidationStrategyStats {
            total_opportunities,
            total_profit_potential,
            avg_profit_margin,
            protocol_breakdown,
            last_scan: chrono::Utc::now(),
        })
    }
    
    fn calculate_protocol_breakdown(&self, opportunities: &[LiquidationOpportunity]) -> HashMap<ProtocolType, u32> {
        let mut breakdown = HashMap::new();
        
        for opportunity in opportunities {
            *breakdown.entry(opportunity.user.protocol).or_insert(0) += 1;
        }
        
        breakdown
    }
}

#[derive(Debug, Clone)]
pub struct LiquidationStrategyStats {
    pub total_opportunities: usize,
    pub total_profit_potential: f64,
    pub avg_profit_margin: f64,
    pub protocol_breakdown: HashMap<ProtocolType, u32>,
    pub last_scan: chrono::DateTime<chrono::Utc>,
}

/// MEV 통합을 위한 Opportunity 변환
impl From<LiquidationOpportunity> for Opportunity {
    fn from(liquidation_opp: LiquidationOpportunity) -> Self {
        Opportunity {
            strategy: MEVStrategy::Liquidation,
            profit_estimate: liquidation_opp.strategy.net_profit_usd,
            gas_estimate: liquidation_opp.strategy.cost_breakdown.total_cost_usd,
            execution_data: liquidation_opp.execution_transaction.unwrap_or_default(),
            priority_score: liquidation_opp.profitability_analysis.estimated_net_profit_usd,
            target_transaction: None, // 청산은 멤풀 기반이 아님
            detected_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_liquidation_opportunity_detection() {
        // 테스트는 실제 네트워크 연결이 필요하므로 mock 환경에서 실행
        println!("Liquidation Strategy v2 tests require live network connection");
    }
}