//! 아비트리지 기회 탐지 시스템
//! 
//! 이 모듈은 실시간 가격 데이터를 분석하여
//! 수익성 있는 아비트리지 기회를 탐지하고 분석합니다.

use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tokio::sync::RwLock;
use tracing::{info, debug, warn};
use ethers::types::U256;
use rust_decimal::Decimal;
use chrono::Utc;

use crate::config::Config;
use super::types::{
    PriceData, MicroArbitrageOpportunity, ExchangeInfo, 
    PriceImpactAnalysis, CompetitionAnalysis, CompetitionStrategy,
    ExecutionPriority, MicroArbitrageConfig
};

/// 아비트리지 기회 탐지기
pub struct OpportunityDetector {
    config: Arc<Config>,
    exchanges: Arc<RwLock<HashMap<String, ExchangeInfo>>>,
    min_profit_percentage: f64,
    min_profit_usd: f64,
    max_slippage_percentage: f64,
    confidence_threshold: f64,
}

impl OpportunityDetector {
    /// 새로운 기회 탐지기 생성
    pub fn new(config: Arc<Config>) -> Self {
        let micro_config = &config.strategies.micro_arbitrage;
        
        Self {
            config,
            exchanges: Arc::new(RwLock::new(HashMap::new())),
            min_profit_percentage: micro_config.min_profit_percentage,
            min_profit_usd: micro_config.min_profit_usd,
            max_slippage_percentage: 0.01, // 1% 기본값
            confidence_threshold: 0.6, // 60% 기본값
        }
    }
    
    /// 거래소 정보 업데이트
    pub async fn update_exchanges(&self, exchanges: HashMap<String, ExchangeInfo>) {
        let mut exchange_map = self.exchanges.write().await;
        *exchange_map = exchanges;
    }
    
    /// 가격 데이터에서 아비트리지 기회 탐지
    pub async fn detect_opportunities(
        &self,
        price_data_map: &HashMap<String, PriceData>,
    ) -> Result<Vec<MicroArbitrageOpportunity>> {
        let mut opportunities = Vec::new();
        
        // 심볼별로 그룹화
        let mut symbol_groups: HashMap<String, Vec<&PriceData>> = HashMap::new();
        for price_data in price_data_map.values() {
            symbol_groups.entry(price_data.symbol.clone())
                .or_insert_with(Vec::new)
                .push(price_data);
        }
        
        // 각 심볼에 대해 아비트리지 기회 탐지
        for (symbol, price_data_list) in symbol_groups {
            if price_data_list.len() < 2 {
                continue; // 최소 2개 거래소 필요
            }
            
            let symbol_opportunities = self.detect_symbol_opportunities(&symbol, &price_data_list).await?;
            opportunities.extend(symbol_opportunities);
        }
        
        // 수익성 순으로 정렬
        opportunities.sort_by(|a, b| b.profit_percentage.partial_cmp(&a.profit_percentage).unwrap());
        
        debug!("🔍 {}개 아비트리지 기회 탐지됨", opportunities.len());
        Ok(opportunities)
    }
    
    /// 특정 심볼에 대한 아비트리지 기회 탐지
    async fn detect_symbol_opportunities(
        &self,
        symbol: &str,
        price_data_list: &[&PriceData],
    ) -> Result<Vec<MicroArbitrageOpportunity>> {
        let mut opportunities = Vec::new();
        
        // 모든 거래소 조합 검사
        for i in 0..price_data_list.len() {
            for j in i + 1..price_data_list.len() {
                let buy_data = price_data_list[i];
                let sell_data = price_data_list[j];
                
                // 양방향 검사 (A에서 사서 B에서 팔기, B에서 사서 A에서 팔기)
                if let Some(opp) = self.analyze_arbitrage_pair(symbol, buy_data, sell_data).await? {
                    opportunities.push(opp);
                }
                
                if let Some(opp) = self.analyze_arbitrage_pair(symbol, sell_data, buy_data).await? {
                    opportunities.push(opp);
                }
            }
        }
        
        Ok(opportunities)
    }
    
    /// 거래소 쌍에 대한 아비트리지 분석
    async fn analyze_arbitrage_pair(
        &self,
        symbol: &str,
        buy_data: &PriceData,
        sell_data: &PriceData,
    ) -> Result<Option<MicroArbitrageOpportunity>> {
        // 기본 수익성 검사
        if sell_data.bid <= buy_data.ask {
            return Ok(None); // 수익성 없음
        }
        
        let price_spread = sell_data.bid - buy_data.ask;
        let profit_percentage = (price_spread / buy_data.ask).to_f64().unwrap_or(0.0);
        
        // 최소 수익률 확인
        if profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }
        
        // 거래소 수수료 고려
        let exchanges = self.exchanges.read().await;
        let buy_exchange_info = exchanges.get(&buy_data.exchange);
        let sell_exchange_info = exchanges.get(&sell_data.exchange);
        
        if buy_exchange_info.is_none() || sell_exchange_info.is_none() {
            return Ok(None);
        }
        
        let buy_fee = buy_exchange_info.unwrap().fee_percentage;
        let sell_fee = sell_exchange_info.unwrap().fee_percentage;
        let total_fees = buy_fee + sell_fee;
        let net_profit_percentage = profit_percentage - total_fees;
        
        if net_profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }
        
        // 최대 거래 가능 수량 계산
        let max_amount = self.calculate_max_trade_amount(
            buy_exchange_info.unwrap(),
            sell_exchange_info.unwrap(),
            &buy_data.ask,
        ).await?;
        
        // 최소 수익 USD 확인
        let profit_usd = (max_amount.as_u128() as f64 * net_profit_percentage / 1e18) * 
                        buy_data.ask.to_f64().unwrap_or(0.0);
        if profit_usd < self.min_profit_usd {
            return Ok(None);
        }
        
        // 가격 임팩트 분석
        let price_impact = self.analyze_price_impact(symbol, max_amount, buy_data, sell_data).await?;
        
        // 경쟁 분석
        let competition = self.analyze_competition(symbol, buy_data, sell_data).await?;
        
        // 신뢰도 점수 계산
        let confidence_score = self.calculate_confidence_score(
            symbol,
            &buy_data.exchange,
            &sell_data.exchange,
            net_profit_percentage,
            &price_impact,
            &competition,
        ).await?;
        
        if confidence_score < self.confidence_threshold {
            return Ok(None);
        }
        
        // 실행 우선순위 결정
        let priority = self.determine_execution_priority(
            net_profit_percentage,
            confidence_score,
            &price_impact,
        );
        
        // 예상 수익 계산
        let expected_profit = U256::from(
            (max_amount.as_u128() as f64 * net_profit_percentage) as u64
        );
        
        // 가스 비용 추정
        let gas_cost = self.estimate_gas_cost().await?;
        let net_profit = if expected_profit > gas_cost {
            expected_profit - gas_cost
        } else {
            U256::zero()
        };
        
        if net_profit <= U256::zero() {
            return Ok(None);
        }
        
        // 기본 자산 추출
        let parts: Vec<&str> = symbol.split('/').collect();
        let base_asset = parts.get(0).unwrap_or(&"ETH").to_string();
        
        // 실행 시간 윈도우 계산
        let execution_window_ms = self.calculate_execution_window(
            buy_exchange_info.unwrap(),
            sell_exchange_info.unwrap(),
        );
        
        let opportunity = MicroArbitrageOpportunity {
            id: format!("arb_{}_{}", Utc::now().timestamp_millis(), fastrand::u32(..)),
            token_symbol: symbol.to_string(),
            buy_exchange: buy_data.exchange.clone(),
            sell_exchange: sell_data.exchange.clone(),
            buy_price: buy_data.ask,
            sell_price: sell_data.bid,
            price_spread,
            profit_percentage: net_profit_percentage,
            max_amount,
            execution_window_ms,
            confidence_score,
            expected_profit,
            buy_amount: max_amount,
            base_asset,
            quote_asset: parts.get(1).unwrap_or(&"USDT").to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(30),
            gas_cost,
            net_profit,
            success_probability: confidence_score,
        };
        
        debug!("🎯 아비트리지 기회 발견: {} {} -> {} (수익률: {:.2}%)",
               symbol, buy_data.exchange, sell_data.exchange, net_profit_percentage * 100.0);
        
        Ok(Some(opportunity))
    }
    
    /// 최대 거래 수량 계산
    async fn calculate_max_trade_amount(
        &self,
        buy_exchange: &ExchangeInfo,
        sell_exchange: &ExchangeInfo,
        price: &Decimal,
    ) -> Result<U256> {
        // 거래소별 최소/최대 한도
        let min_size = std::cmp::max(buy_exchange.min_order_size, sell_exchange.min_order_size);
        let max_size = std::cmp::min(buy_exchange.max_order_size, sell_exchange.max_order_size);
        
        // 위험 관리 한도 적용 (설정에서 가져오기)
        let risk_limit = U256::from(1000); // 1000 USD 기본값
        let risk_based_limit = U256::from(
            (risk_limit.as_u128() as f64 / price.to_f64().unwrap_or(1.0)) as u64
        );
        
        let final_amount = std::cmp::min(max_size, risk_based_limit);
        Ok(std::cmp::max(min_size, final_amount))
    }
    
    /// 가격 임팩트 분석 (오더북 기반)
    async fn analyze_price_impact(
        &self,
        symbol: &str,
        amount: U256,
        buy_data: &PriceData,
        sell_data: &PriceData,
    ) -> Result<PriceImpactAnalysis> {
        // 실제 오더북 데이터를 사용하여 정확한 가격 임팩트 계산
        let amount_f64 = amount.as_u128() as f64 / 1e18;

        // 매수 오더북 분석
        let (buy_impact, buy_liquidity) = if let Some(orderbook) = &buy_data.orderbook {
            self.calculate_orderbook_impact(&orderbook.asks, amount_f64, true)
        } else {
            // 오더북이 없으면 보수적인 추정치 사용
            (amount_f64 * 0.002, 500000.0) // 0.2% 추정, 500K USD 유동성
        };

        // 매도 오더북 분석
        let (sell_impact, sell_liquidity) = if let Some(orderbook) = &sell_data.orderbook {
            self.calculate_orderbook_impact(&orderbook.bids, amount_f64, false)
        } else {
            // 오더북이 없으면 보수적인 추정치 사용
            (amount_f64 * 0.002, 500000.0) // 0.2% 추정, 500K USD 유동성
        };

        let total_impact = buy_impact + sell_impact;
        let min_liquidity = buy_liquidity.min(sell_liquidity);

        // 슬리피지 계산: 임팩트의 70% 정도를 실제 슬리피지로 예상
        let estimated_slippage = total_impact * 0.7;

        // 안전한 거래 크기 계산: 유동성의 10% 이하
        let max_safe_trade_size_f64 = min_liquidity * 0.1;
        let max_safe_trade_size = U256::from((max_safe_trade_size_f64 * 1e18) as u128);

        // 최적 거래 크기: 임팩트가 0.5% 이하가 되도록
        let optimal_trade_size = if total_impact > 0.005 {
            // 임팩트를 0.5%로 제한하려면 거래 크기 조정
            let ratio = 0.005 / total_impact;
            U256::from((amount_f64 * ratio * 1e18) as u128)
        } else {
            amount
        };

        Ok(PriceImpactAnalysis {
            symbol: symbol.to_string(),
            exchange: buy_data.exchange.clone(),
            current_price: buy_data.ask,
            impact_percentage: total_impact,
            liquidity_depth: U256::from(min_liquidity as u128),
            optimal_trade_size,
            max_safe_trade_size,
            estimated_slippage,
        })
    }

    /// 오더북 기반 임팩트 계산
    fn calculate_orderbook_impact(
        &self,
        orders: &[(Decimal, Decimal)], // (가격, 수량)
        amount: f64,
        is_ask: bool, // true면 asks (매수), false면 bids (매도)
    ) -> (f64, f64) {
        if orders.is_empty() {
            return (0.005, 100000.0); // 기본값: 0.5% 임팩트, 100K USD 유동성
        }

        let mut remaining_amount = amount;
        let mut total_cost = 0.0;
        let mut total_liquidity = 0.0;
        let first_price = orders[0].0.to_f64().unwrap_or(0.0);

        for (price, quantity) in orders.iter().take(50) { // 최대 50개 레벨만 확인
            let price_f64 = price.to_f64().unwrap_or(0.0);
            let quantity_f64 = quantity.to_f64().unwrap_or(0.0);

            if price_f64 <= 0.0 || quantity_f64 <= 0.0 {
                continue;
            }

            let available = quantity_f64;
            total_liquidity += available * price_f64;

            if remaining_amount <= 0.0 {
                break;
            }

            let filled = remaining_amount.min(available);
            total_cost += filled * price_f64;
            remaining_amount -= filled;
        }

        // 임팩트 계산: (평균 실행 가격 - 최초 가격) / 최초 가격
        let impact = if first_price > 0.0 && amount > 0.0 {
            let avg_execution_price = total_cost / amount;
            let price_diff = if is_ask {
                avg_execution_price - first_price
            } else {
                first_price - avg_execution_price
            };
            (price_diff / first_price).abs()
        } else {
            0.005 // 기본 0.5%
        };

        (impact, total_liquidity)
    }
    
    /// 경쟁 분석
    async fn analyze_competition(
        &self,
        symbol: &str,
        buy_data: &PriceData,
        sell_data: &PriceData,
    ) -> Result<CompetitionAnalysis> {
        // 실제 구현에서는 멤풀 분석, 과거 데이터 등을 사용
        // 여기서는 간단한 추정치 사용
        
        let spread_ratio = (sell_data.bid - buy_data.ask) / buy_data.ask;
        let competition_intensity = if spread_ratio.to_f64().unwrap_or(0.0) > 0.01 {
            0.3 // 낮은 경쟁
        } else {
            0.8 // 높은 경쟁
        };
        
        let recommended_strategy = if competition_intensity < 0.5 {
            CompetitionStrategy::Aggressive
        } else {
            CompetitionStrategy::Conservative
        };
        
        Ok(CompetitionAnalysis {
            symbol: symbol.to_string(),
            competitor_count: 2, // 추정
            avg_competitor_size: U256::from(1000),
            competition_intensity,
            market_share: 0.1, // 10% 추정
            recommended_strategy,
        })
    }
    
    /// 신뢰도 점수 계산
    async fn calculate_confidence_score(
        &self,
        symbol: &str,
        buy_exchange: &str,
        sell_exchange: &str,
        profit_percentage: f64,
        price_impact: &PriceImpactAnalysis,
        competition: &CompetitionAnalysis,
    ) -> Result<f64> {
        let mut score = 0.5; // 기본 점수
        
        // 수익률 기반 점수 (높을수록 좋음)
        score += (profit_percentage * 1000.0).min(0.3);
        
        // 가격 임팩트 기반 점수 (낮을수록 좋음)
        if price_impact.impact_percentage < 0.01 {
            score += 0.2;
        } else if price_impact.impact_percentage < 0.05 {
            score += 0.1;
        }
        
        // 경쟁 강도 기반 점수 (낮을수록 좋음)
        score += (1.0 - competition.competition_intensity) * 0.2;
        
        // 거래소 신뢰도 (실제로는 거래소별 신뢰도 데이터 사용)
        let exchange_trust = 0.1; // 기본값
        score += exchange_trust;
        
        Ok(score.clamp(0.0, 1.0))
    }
    
    /// 실행 우선순위 결정
    fn determine_execution_priority(
        &self,
        profit_percentage: f64,
        confidence_score: f64,
        price_impact: &PriceImpactAnalysis,
    ) -> ExecutionPriority {
        if profit_percentage > 0.05 && confidence_score > 0.8 && price_impact.impact_percentage < 0.01 {
            ExecutionPriority::Critical
        } else if profit_percentage > 0.02 && confidence_score > 0.7 {
            ExecutionPriority::High
        } else if profit_percentage > 0.01 && confidence_score > 0.6 {
            ExecutionPriority::Medium
        } else {
            ExecutionPriority::Low
        }
    }
    
    /// 실행 시간 윈도우 계산
    fn calculate_execution_window(
        &self,
        buy_exchange: &ExchangeInfo,
        sell_exchange: &ExchangeInfo,
    ) -> u64 {
        let latency = buy_exchange.latency_ms + sell_exchange.latency_ms;
        let buffer = 1000; // 1초 버퍼
        latency + buffer
    }
    
    /// 가스 비용 추정
    async fn estimate_gas_cost(&self) -> Result<U256> {
        // 실제 구현에서는 현재 가스 가격을 조회
        let gas_price = U256::from(20_000_000_000u64); // 20 gwei
        let gas_limit = U256::from(300_000u64); // 300k gas
        Ok(gas_price * gas_limit)
    }
    
    /// 기회 유효성 검증
    pub async fn validate_opportunity(&self, opportunity: &MicroArbitrageOpportunity) -> bool {
        // 기본 유효성 검사
        if !opportunity.is_valid() {
            return false;
        }
        
        // 수익성 재검증
        if opportunity.profit_percentage < self.min_profit_percentage {
            return false;
        }
        
        // 신뢰도 검증
        if opportunity.confidence_score < self.confidence_threshold {
            return false;
        }
        
        // 만료 시간 검증
        if opportunity.time_to_expiry_ms() < 5000 { // 5초 미만
            return false;
        }
        
        true
    }
    
    /// 기회 필터링
    pub async fn filter_opportunities(
        &self,
        opportunities: Vec<MicroArbitrageOpportunity>,
    ) -> Vec<MicroArbitrageOpportunity> {
        let mut filtered = Vec::new();
        
        for opportunity in opportunities {
            if self.validate_opportunity(&opportunity).await {
                filtered.push(opportunity);
            }
        }
        
        // 수익성 순으로 정렬
        filtered.sort_by(|a, b| b.net_profit.cmp(&a.net_profit));
        
        debug!("🔍 {}개 기회 중 {}개 유효한 기회 필터링됨", 
               opportunities.len(), filtered.len());
        
        filtered
    }
}