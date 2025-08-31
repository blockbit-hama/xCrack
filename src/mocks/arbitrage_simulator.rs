use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Result;
use tracing::{info, debug};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use chrono::Utc;

use crate::types::{
    Opportunity, OpportunityType, MicroArbitrageDetails, 
    ExchangeInfo, PriceData
};
use alloy::primitives::U256;
use crate::mocks::{MockConfig, get_mock_config};

/// 마이크로아비트래지 기회 시뮬레이터
/// 
/// 실제 시장 조건을 모방하여 거래소 간 가격 차이를 생성하고
/// 수익성 있는 아비트래지 기회를 시뮬레이션합니다.
pub struct ArbitrageOpportunitySimulator {
    mock_config: MockConfig,
    market_conditions: Arc<MarketConditions>,
    exchange_configs: HashMap<String, ExchangeSimConfig>,
    price_history: Arc<tokio::sync::Mutex<PriceHistory>>,
}

/// 시장 상황 시뮬레이션
#[derive(Debug, Clone)]
pub struct MarketConditions {
    volatility_multiplier: f64,      // 변동성 배수 (0.5 ~ 2.0)
    arbitrage_efficiency: f64,       // 아비트래지 효율성 (높을수록 기회 적음)
    network_congestion: f64,         // 네트워크 혼잡도 (0.0 ~ 1.0)
    market_maker_activity: f64,      // 마켓 메이커 활동도 (높을수록 스프레드 좁음)
}

/// 거래소별 시뮬레이션 설정
#[derive(Debug, Clone)]
struct ExchangeSimConfig {
    base_latency_ms: u64,
    price_deviation: f64,            // 기준 가격 대비 편차
    liquidity_multiplier: f64,       // 유동성 배수
    fee_rate: f64,                   // 거래 수수료율
    reliability_score: f64,          // 신뢰도 점수 (0.0 ~ 1.0)
}

/// 가격 히스토리 관리
#[derive(Debug)]
struct PriceHistory {
    price_data: HashMap<String, HashMap<String, Vec<PriceData>>>, // exchange -> symbol -> history
    max_history_size: usize,
}

impl ArbitrageOpportunitySimulator {
    pub fn new() -> Self {
        let mock_config = get_mock_config();
        
        // 거래소별 설정
        let exchange_configs = HashMap::from([
            ("uniswap_v2".to_string(), ExchangeSimConfig {
                base_latency_ms: 100,
                price_deviation: 0.002,     // DEX는 약간의 가격 편차
                liquidity_multiplier: 0.7,  // DEX는 상대적으로 낮은 유동성
                fee_rate: 0.003,            // 0.3% 수수료
                reliability_score: 0.85,
            }),
            ("sushiswap".to_string(), ExchangeSimConfig {
                base_latency_ms: 120,
                price_deviation: -0.001,    // SushiSwap은 약간 낮은 가격
                liquidity_multiplier: 0.6,
                fee_rate: 0.003,
                reliability_score: 0.83,
            }),
            ("mock_binance".to_string(), ExchangeSimConfig {
                base_latency_ms: 50,
                price_deviation: 0.001,     // CEX는 일반적으로 안정적
                liquidity_multiplier: 1.5,  // CEX는 높은 유동성
                fee_rate: 0.001,            // 0.1% 수수료
                reliability_score: 0.95,
            }),
            ("mock_coinbase".to_string(), ExchangeSimConfig {
                base_latency_ms: 60,
                price_deviation: -0.0005,
                liquidity_multiplier: 1.3,
                fee_rate: 0.005,            // Coinbase는 수수료가 높음
                reliability_score: 0.92,
            }),
        ]);
        
        Self {
            mock_config,
            market_conditions: Arc::new(Self::generate_market_conditions()),
            exchange_configs,
            price_history: Arc::new(tokio::sync::Mutex::new(PriceHistory {
                price_data: HashMap::new(),
                max_history_size: 100,
            })),
        }
    }
    
    /// 동적 시장 상황 생성
    fn generate_market_conditions() -> MarketConditions {
        MarketConditions {
            volatility_multiplier: 0.8 + fastrand::f64() * 0.4,      // 0.8 ~ 1.2
            arbitrage_efficiency: 0.7 + fastrand::f64() * 0.25,      // 0.7 ~ 0.95
            network_congestion: fastrand::f64() * 0.3,               // 0.0 ~ 0.3
            market_maker_activity: 0.6 + fastrand::f64() * 0.35,     // 0.6 ~ 0.95
        }
    }
    
    /// 아비트래지 기회 검색
    pub async fn find_arbitrage_opportunities(&self, trading_pairs: &[String]) -> Result<Vec<Opportunity>> {
        let mut opportunities = Vec::new();
        
        debug!("🔍 아비트래지 기회 스캔 중... ({}개 페어)", trading_pairs.len());
        
        for pair in trading_pairs {
            // 각 거래소의 현재 가격 시뮬레이션
            let mut exchange_prices = HashMap::new();
            
            for (exchange_name, exchange_config) in &self.exchange_configs {
                let price_data = self.simulate_price_data(exchange_name, pair, exchange_config).await;
                exchange_prices.insert(exchange_name.clone(), price_data);
            }
            
            // 가격 히스토리 업데이트
            self.update_price_history(&exchange_prices).await;
            
            // 아비트래지 기회 분석
            if let Some(opportunity) = self.analyze_arbitrage_opportunity(pair, &exchange_prices).await {
                opportunities.push(opportunity);
            }
        }
        
        // 기회 정렬 (수익률 기준)
        opportunities.sort_by(|a, b| {
            // 간단한 수익률 비교 - OpportunityType은 enum variant이므로 직접 접근 불가
            b.expected_profit.cmp(&a.expected_profit)
        });
        
        if !opportunities.is_empty() {
            info!("✨ {}개의 아비트래지 기회 발견", opportunities.len());
        }
        
        Ok(opportunities)
    }
    
    /// 특정 거래소의 가격 데이터 시뮬레이션
    async fn simulate_price_data(&self, exchange_name: &str, symbol: &str, config: &ExchangeSimConfig) -> PriceData {
        // 기본 가격 (글로벌 시장 가격)
        let base_price = match symbol {
            "WETH/USDC" => 2000.0,
            "WETH/USDT" => 2001.0,
            "WETH/DAI" => 1999.0,
            "WBTC/USDC" => 45000.0,
            "WBTC/USDT" => 45050.0,
            _ => 100.0,
        };
        
        // 시장 상황 적용
        let market_volatility = self.market_conditions.volatility_multiplier;
        let random_fluctuation = (fastrand::f64() - 0.5) * 0.01 * market_volatility;
        
        // 거래소별 편차 적용
        let exchange_deviation = config.price_deviation;
        
        // 최종 가격 계산
        let adjusted_price = base_price * (1.0 + exchange_deviation + random_fluctuation);
        
        // 스프레드 계산 (시장 메이커 활동도에 따라)
        let base_spread = if exchange_name.contains("dex") || exchange_name.contains("uniswap") || exchange_name.contains("sushi") {
            0.005 // DEX 기본 스프레드
        } else {
            0.001 // CEX 기본 스프레드
        };
        
        let market_maker_effect = 1.0 / self.market_conditions.market_maker_activity;
        let effective_spread = base_spread * market_maker_effect;
        
        let bid_price = adjusted_price * (1.0 - effective_spread / 2.0);
        let ask_price = adjusted_price * (1.0 + effective_spread / 2.0);
        
        // 거래량 시뮬레이션
        let base_volume = if exchange_name.contains("binance") || exchange_name.contains("coinbase") {
            fastrand::u64(5_000_000..20_000_000) // CEX는 높은 거래량
        } else {
            fastrand::u64(500_000..2_000_000)    // DEX는 낮은 거래량
        };
        
        PriceData {
            symbol: symbol.to_string(),
            exchange: exchange_name.to_string(),
            bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
            ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
            last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
            volume_24h: U256::from(base_volume),
            timestamp: Utc::now(),
            sequence: fastrand::u64(..),
        }
    }
    
    /// 아비트래지 기회 분석
    async fn analyze_arbitrage_opportunity(
        &self, 
        symbol: &str, 
        exchange_prices: &HashMap<String, PriceData>
    ) -> Option<Opportunity> {
        let mut best_buy = None;
        let mut best_sell = None;
        let mut max_profit = 0.0;
        
        // 모든 거래소 조합에서 최고 수익 찾기
        for (buy_exchange, buy_price_data) in exchange_prices {
            for (sell_exchange, sell_price_data) in exchange_prices {
                if buy_exchange == sell_exchange {
                    continue;
                }
                
                // 매수: ask 가격, 매도: bid 가격
                let buy_price = buy_price_data.ask.to_f64().unwrap_or(0.0);
                let sell_price = sell_price_data.bid.to_f64().unwrap_or(0.0);
                
                if sell_price <= buy_price {
                    continue; // 수익 없음
                }
                
                // 거래 수수료 고려
                let buy_config = self.exchange_configs.get(buy_exchange)?;
                let sell_config = self.exchange_configs.get(sell_exchange)?;
                
                let effective_buy_price = buy_price * (1.0 + buy_config.fee_rate);
                let effective_sell_price = sell_price * (1.0 - sell_config.fee_rate);
                
                if effective_sell_price <= effective_buy_price {
                    continue;
                }
                
                let profit_percentage = (effective_sell_price - effective_buy_price) / effective_buy_price * 100.0;
                
                if profit_percentage > max_profit && profit_percentage >= 0.05 { // 최소 0.05% 수익
                    max_profit = profit_percentage;
                    best_buy = Some((buy_exchange.clone(), buy_price_data.clone()));
                    best_sell = Some((sell_exchange.clone(), sell_price_data.clone()));
                }
            }
        }
        
        if let (Some((buy_exchange, buy_data)), Some((sell_exchange, sell_data))) = (best_buy, best_sell) {
            // 아비트래지 기회 발견 확률 체크
            if fastrand::f64() > self.mock_config.arbitrage_opportunity_rate {
                return None; // 이번에는 기회 없음
            }
            
            // 거래 가능한 수량 계산
            let buy_config = &self.exchange_configs[&buy_exchange];
            let sell_config = &self.exchange_configs[&sell_exchange];
            
            let max_amount_usd = self.mock_config.min_profit_usd 
                + fastrand::f64() * (self.mock_config.max_profit_usd - self.mock_config.min_profit_usd);
            
            let trade_amount = U256::from((max_amount_usd / buy_data.ask.to_f64().unwrap_or(1.0) * 1e6) as u64);
            
            let estimated_profit_usd = max_amount_usd * (max_profit / 100.0);
            
            debug!("💡 아비트래지 기회: {} ({} -> {}) - {:.3}% 수익 (${:.2})", 
                   symbol, buy_exchange, sell_exchange, max_profit, estimated_profit_usd);
            
            Some(Opportunity {
                id: format!("arb_{}_{}", symbol.replace('/', "_"), fastrand::u64(10000..99999)),
                opportunity_type: OpportunityType::MicroArbitrage,
                strategy: crate::types::StrategyType::MicroArbitrage,
                expected_profit: U256::from((estimated_profit_usd * 1e6) as u64), // Convert to wei-like units
                confidence: (buy_config.reliability_score + sell_config.reliability_score) / 2.0,
                gas_estimate: if buy_exchange.contains("dex") || sell_exchange.contains("dex") {
                    fastrand::u64(50000..150000) // DEX 가스
                } else {
                    0 // CEX는 가스비 없음
                },
                priority: crate::types::Priority::Medium, // 임시값
                timestamp: Utc::now(),
                expiry_block: 1000000, // 임시값
                details: crate::types::OpportunityDetails::MicroArbitrage(MicroArbitrageDetails {
                    token_symbol: symbol.to_string(),
                    buy_exchange: ExchangeInfo {
                        name: buy_exchange.clone(),
                        exchange_type: crate::types::ExchangeType::DEX, // 임시값
                        api_endpoint: "".to_string(),
                        trading_pairs: vec![],
                        fee_percentage: buy_config.fee_rate,
                        min_order_size: U256::from(1000),
                        max_order_size: U256::from(1000000),
                        latency_ms: 50,
                    },
                    sell_exchange: ExchangeInfo {
                        name: sell_exchange.clone(),
                        exchange_type: crate::types::ExchangeType::CEX, // 임시값
                        api_endpoint: "".to_string(),
                        trading_pairs: vec![],
                        fee_percentage: sell_config.fee_rate,
                        min_order_size: U256::from(1000),
                        max_order_size: U256::from(1000000),
                        latency_ms: 30,
                    },
                    amount: trade_amount,
                    buy_price: buy_data.ask,
                    sell_price: sell_data.bid,
                    price_diff: sell_data.bid - buy_data.ask,
                    profit_percentage: max_profit,
                    execution_time_ms: 100, // 임시값
                    order_books: vec![], // 임시값
                }),
            })
        } else {
            None
        }
    }
    
    /// 수익률 계산
    fn calculate_profit_percentage(buy_price: &Decimal, sell_price: &Decimal) -> f64 {
        let buy_f64 = buy_price.to_f64().unwrap_or(0.0);
        let sell_f64 = sell_price.to_f64().unwrap_or(0.0);
        
        if buy_f64 > 0.0 {
            (sell_f64 - buy_f64) / buy_f64 * 100.0
        } else {
            0.0
        }
    }
    
    /// 가격 히스토리 업데이트
    async fn update_price_history(&self, exchange_prices: &HashMap<String, PriceData>) {
        let mut history = self.price_history.lock().await;
        let max_size = history.max_history_size; // 먼저 읽어서 차용 충돌 방지
        
        for (exchange, price_data) in exchange_prices {
            let exchange_history = history.price_data.entry(exchange.clone()).or_insert_with(HashMap::new);
            let symbol_history = exchange_history.entry(price_data.symbol.clone()).or_insert_with(Vec::new);
            
            symbol_history.push(price_data.clone());
            
            // 히스토리 크기 제한
            if symbol_history.len() > max_size {
                symbol_history.drain(..symbol_history.len() - max_size);
            }
        }
    }
    
    /// 시장 상황 업데이트 (주기적 호출)
    pub async fn update_market_conditions(&mut self) {
        // 시장 상황을 동적으로 변경
        let new_conditions = Self::generate_market_conditions();
        
        debug!("📊 시장 상황 업데이트 - 변동성: {:.2}, 효율성: {:.2}, 혼잡: {:.2}", 
               new_conditions.volatility_multiplier,
               new_conditions.arbitrage_efficiency,
               new_conditions.network_congestion);
        
        self.market_conditions = Arc::new(new_conditions);
    }
    
    /// 가격 히스토리 조회
    pub async fn get_price_history(&self, exchange: &str, symbol: &str) -> Vec<PriceData> {
        let history = self.price_history.lock().await;
        history.price_data
            .get(exchange)
            .and_then(|exchange_data| exchange_data.get(symbol))
            .cloned()
            .unwrap_or_default()
    }
    
    /// 현재 시장 상황 조회
    pub fn get_market_conditions(&self) -> MarketConditions {
        (*self.market_conditions).clone()
    }
}

impl Default for ArbitrageOpportunitySimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_arbitrage_simulator() {
        let mut simulator = ArbitrageOpportunitySimulator::new();
        let trading_pairs = vec!["WETH/USDC".to_string(), "WBTC/USDC".to_string()];
        
        let opportunities = simulator.find_arbitrage_opportunities(&trading_pairs).await;
        assert!(opportunities.is_ok());
        
        // 시장 상황 업데이트 테스트
        simulator.update_market_conditions().await;
        let market_conditions = simulator.get_market_conditions();
        assert!(market_conditions.volatility_multiplier > 0.0);
        assert!(market_conditions.arbitrage_efficiency > 0.0);
    }
    
    #[test]
    fn test_profit_calculation() {
        let buy_price = Decimal::from(2000);
        let sell_price = Decimal::from(2010);
        
        let profit = ArbitrageOpportunitySimulator::calculate_profit_percentage(&buy_price, &sell_price);
        assert!((profit - 0.5).abs() < 0.01); // 0.5% 수익
    }
}