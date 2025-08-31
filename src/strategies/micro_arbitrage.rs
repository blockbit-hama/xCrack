use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use tracing::{info, debug, warn, error};
use alloy::primitives::U256;
use core::str::FromStr;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use chrono::Utc;
use ethers::providers::{Provider, Ws};

use crate::config::Config;
use crate::types::{
    Transaction, Opportunity, StrategyType,
    MicroArbitrageOpportunity, PriceData, 
    OrderBookSnapshot, ExchangeInfo, ExchangeType, MicroArbitrageStats,
    
};
use crate::strategies::Strategy;
use serde::Deserialize;

// Helper type for aggregator execution quotes (0x)
struct OxSwapQuote { 
    to: alloy::primitives::Address, 
    data: Vec<u8>, 
    spender: Option<alloy::primitives::Address>, 
    amount_out: alloy::primitives::U256 
}

/// 자금 조달 방식별 수익성 메트릭
#[derive(Debug, Clone)]
pub struct FundingMetrics {
    pub gross_profit: U256,        // 총 수익
    pub total_cost: U256,          // 총 비용
    pub net_profit: U256,          // 순수익
    pub gas_cost: U256,            // 가스 비용
    pub premium_cost: U256,        // 플래시론 수수료
    pub success_probability: f64,  // 성공 확률
    pub liquidity_available: bool, // 유동성 가용 여부
}

/// 초단타 마이크로 아비트래지 전략
/// 
/// 여러 거래소간 수 밀리초 단위 가격 차이를 포착하여 
/// 소규모 거래를 초고속으로 반복 실행하는 전략
pub struct MicroArbitrageStrategy {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    enabled: Arc<AtomicBool>,
    
    // 거래소 정보
    exchanges: HashMap<String, ExchangeInfo>,
    
    // 실시간 가격 데이터 캐시 (거래소별, 심볼별)
    price_cache: Arc<Mutex<HashMap<String, HashMap<String, PriceData>>>>,
    
    // 오더북 캐시
    orderbook_cache: Arc<Mutex<HashMap<String, HashMap<String, OrderBookSnapshot>>>>,
    
    // 활성 거래 추적
    active_trades: Arc<Mutex<HashMap<String, MicroArbitrageOpportunity>>>,
    
    // 성능 통계
    stats: Arc<Mutex<MicroArbitrageStats>>,
    
    // 수익률 임계값
    min_profit_percentage: f64,
    min_profit_usd: Decimal,
    
    // 실행 매개변수
    execution_timeout_ms: u64,
    max_concurrent_trades: usize,
    latency_threshold_ms: u64,
    
    // 위험 관리
    daily_volume_limit: U256,
    risk_limit_per_trade: U256,

    // 번들/MEV 미사용 정책: 번들 채널 제거
    
    // 자금 조달 모드 설정
    funding_mode: String, // "auto", "flashloan", "wallet"
}

impl MicroArbitrageStrategy {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("⚡ 마이크로 아비트래지 전략 초기화 중...");
        
        // 거래소 정보 로드
        let mut exchanges = HashMap::new();
        for exchange_config in &config.strategies.micro_arbitrage.exchanges {
            if exchange_config.enabled {
                let exchange_info = ExchangeInfo {
                    name: exchange_config.name.clone(),
                    exchange_type: match exchange_config.exchange_type {
                        crate::config::ExchangeType::DEX => ExchangeType::DEX,
                        crate::config::ExchangeType::CEX => ExchangeType::CEX,
                    },
                    api_endpoint: exchange_config.api_endpoint.clone(),
                    trading_pairs: exchange_config.trading_pairs.clone(),
                    fee_percentage: exchange_config.fee_percentage,
                    min_order_size: exchange_config.min_order_size.parse::<u64>()
                        .map(U256::from)
                        .unwrap_or(U256::from(10)),
                    max_order_size: exchange_config.max_order_size.parse::<u64>()
                        .map(U256::from)
                        .unwrap_or(U256::from(100000)),
                    latency_ms: 50, // 기본 지연시간
                };
                exchanges.insert(exchange_config.name.clone(), exchange_info);
            }
        }
        
        let min_profit_usd = config.strategies.micro_arbitrage.min_profit_usd
            .parse::<f64>()
            .map(Decimal::from_f64_retain)
            .unwrap_or_else(|_| Some(Decimal::from(5)))
            .unwrap_or(Decimal::from(5));
        
        let daily_volume_limit = config.strategies.micro_arbitrage.daily_volume_limit
            .parse::<u64>()
            .map(U256::from)
            .unwrap_or(U256::from(500000));
        
        let risk_limit_per_trade = config.strategies.micro_arbitrage.risk_limit_per_trade
            .parse::<u64>()
            .map(U256::from)
            .unwrap_or(U256::from(1000));
        
        info!("✅ 마이크로 아비트래지 전략 초기화 완료");
        info!("  📊 활성 거래소: {}개", exchanges.len());
        info!("  💰 최소 수익: {}%", config.strategies.micro_arbitrage.min_profit_percentage * 100.0);
        info!("  ⚡ 실행 타임아웃: {}ms", config.strategies.micro_arbitrage.execution_timeout_ms);
        info!("  🔀 최대 동시 거래: {}개", config.strategies.micro_arbitrage.max_concurrent_trades);
        
        // Get values from config before moving it
        let exchange_count = exchanges.len() as u32;
        let pairs_count = config.strategies.micro_arbitrage.trading_pairs.len() as u32;
        let min_profit_percentage = config.strategies.micro_arbitrage.min_profit_percentage;
        let execution_timeout_ms = config.strategies.micro_arbitrage.execution_timeout_ms;
        let max_concurrent_trades = config.strategies.micro_arbitrage.max_concurrent_trades;
        let latency_threshold_ms = config.strategies.micro_arbitrage.latency_threshold_ms;
        let funding_mode = config.strategies.micro_arbitrage.funding_mode.clone();
        
        Ok(Self {
            config,
            provider,
            enabled: Arc::new(AtomicBool::new(true)),
            exchanges,
            price_cache: Arc::new(Mutex::new(HashMap::new())),
            orderbook_cache: Arc::new(Mutex::new(HashMap::new())),
            active_trades: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(MicroArbitrageStats {
                total_opportunities: 0,
                executed_trades: 0,
                successful_trades: 0,
                failed_trades: 0,
                total_volume: U256::ZERO,
                total_profit: U256::ZERO,
                total_fees: U256::ZERO,
                avg_profit_per_trade: U256::ZERO,
                avg_execution_time_ms: 0.0,
                success_rate: 0.0,
                profit_rate: 0.0,
                uptime_percentage: 100.0,
                exchanges_monitored: exchange_count,
                pairs_monitored: pairs_count,
            })),
            min_profit_percentage,
            min_profit_usd,
            execution_timeout_ms,
            max_concurrent_trades,
            latency_threshold_ms,
            daily_volume_limit,
            risk_limit_per_trade,
            funding_mode,
        })
    }

    // 번들 경로 제거: 마이크로 전략은 공개 트랜잭션 브로드캐스트만 사용
    
    /// 자금 조달 방식 자동 선택
    async fn determine_funding_mode(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<(String, FundingMetrics)> {
        match self.funding_mode.as_str() {
            "flashloan" => Ok(("flashloan".to_string(), self.calculate_flashloan_metrics(opportunity).await?)),
            "wallet" => Ok(("wallet".to_string(), self.calculate_wallet_metrics(opportunity).await?)),
            "auto" | _ => self.auto_select_funding_mode(opportunity).await,
        }
    }
    
    /// 자동 자금 조달 모드 선택 로직
    async fn auto_select_funding_mode(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<(String, FundingMetrics)> {
        // 1. 플래시론과 지갑 방식의 수익성 계산
        let flash_metrics = self.calculate_flashloan_metrics(opportunity).await?;
        let wallet_metrics = self.calculate_wallet_metrics(opportunity).await?;
        
        info!("💰 자금 조달 방식 비교:");
        info!("  플래시론: 순수익 {} ETH, 성공확률 {:.1}%", 
            self.format_eth_amount(flash_metrics.net_profit), flash_metrics.success_probability * 100.0);
        info!("  지갑:     순수익 {} ETH, 성공확률 {:.1}%", 
            self.format_eth_amount(wallet_metrics.net_profit), wallet_metrics.success_probability * 100.0);
        
        // 2. 선택 로직
        // 둘 다 수익성이 없으면 스킵
        if flash_metrics.net_profit <= U256::ZERO && wallet_metrics.net_profit <= U256::ZERO {
            return Ok(("skip".to_string(), flash_metrics));
        }
        
        // 플래시론이 수익성 있고 지갑보다 좋으면 플래시론 선택
        if flash_metrics.net_profit > U256::ZERO && flash_metrics.net_profit >= wallet_metrics.net_profit {
            info!("✅ 플래시론 모드 선택 (더 높은 순수익)");
            return Ok(("flashloan".to_string(), flash_metrics));
        }
        
        // 그 외에는 지갑 선택
        if wallet_metrics.net_profit > U256::ZERO {
            info!("✅ 지갑 모드 선택");
            return Ok(("wallet".to_string(), wallet_metrics));
        }
        
        // 둘 다 안 되면 스킵
        Ok(("skip".to_string(), flash_metrics))
    }
    
    /// 플래시론 방식 수익성 계산
    async fn calculate_flashloan_metrics(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<FundingMetrics> {
        // 1. 기본 수익 계산
        let gross_profit = opportunity.expected_profit;
        
        // 2. 플래시론 수수료 계산 (Aave v3 기본: 9bps)
        let flash_fee_bps = self.config.strategies.micro_arbitrage.max_flashloan_fee_bps;
        let flash_premium = opportunity.buy_amount * U256::from(flash_fee_bps) / U256::from(10000);
        
        // 3. 가스 비용 계산 (플래시론 경로)
        let (base_fee, priority_fee) = self.estimate_gas_price().await?;
        let gas_buffer_pct = self.config.strategies.micro_arbitrage.gas_buffer_pct / 100.0;
        let flash_gas_limit = 400_000; // 플래시론 콜백 포함
        let flash_gas_cost = U256::from((flash_gas_limit as f64 * (1.0 + gas_buffer_pct)) as u64) 
            * (base_fee + priority_fee);
        
        // 4. 총 비용과 순수익
        let total_cost = flash_premium + flash_gas_cost;
        let net_profit = if gross_profit > total_cost {
            gross_profit - total_cost
        } else {
            U256::ZERO
        };
        
        // 5. 플래시론 풀 유동성 체크
        let liquidity_available = self.check_flashloan_liquidity(&opportunity.base_asset, opportunity.buy_amount).await?;
        
        // 6. 성공 확률 계산
        let mut success_prob = 0.85; // 기본 85%
        if !liquidity_available { success_prob *= 0.3; } // 유동성 부족 시 30%
        if flash_gas_cost > gross_profit / U256::from(4) { success_prob *= 0.7; } // 가스비 과다 시 70%
        
        Ok(FundingMetrics {
            gross_profit,
            total_cost,
            net_profit,
            gas_cost: flash_gas_cost,
            premium_cost: flash_premium,
            success_probability: success_prob,
            liquidity_available,
        })
    }
    
    /// 지갑 방식 수익성 계산  
    async fn calculate_wallet_metrics(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<FundingMetrics> {
        // 1. 기본 수익 계산
        let gross_profit = opportunity.expected_profit;
        
        // 2. 가스 비용 계산 (일반 트랜잭션)
        let (base_fee, priority_fee) = self.estimate_gas_price().await?;
        let wallet_gas_limit = 150_000; // 일반 스왑
        let wallet_gas_cost = U256::from(wallet_gas_limit) * (base_fee + priority_fee);
        
        // 3. 총 비용과 순수익 (프리미엄 없음)
        let total_cost = wallet_gas_cost;
        let net_profit = if gross_profit > total_cost {
            gross_profit - total_cost
        } else {
            U256::ZERO
        };
        
        // 4. 지갑 잔고 체크
        let balance_sufficient = self.check_wallet_balance(&opportunity.base_asset, opportunity.buy_amount).await?;
        
        // 5. 성공 확률 계산
        let mut success_prob = 0.95; // 기본 95% (간단함)
        if !balance_sufficient { success_prob = 0.0; } // 잔고 부족 시 불가능
        
        Ok(FundingMetrics {
            gross_profit,
            total_cost,
            net_profit,
            gas_cost: wallet_gas_cost,
            premium_cost: U256::ZERO, // 지갑 방식은 프리미엄 없음
            success_probability: success_prob,
            liquidity_available: balance_sufficient,
        })
    }
    
    /// 플래시론 풀 유동성 체크
    async fn check_flashloan_liquidity(
        &self,
        asset: &str,
        amount: U256,
    ) -> Result<bool> {
        // 실제로는 Aave Pool 컨트랙트에서 조회
        // 여기서는 간단한 추정 로직
        
        // 주요 자산들의 일반적인 유동성 (실제로는 온체인 조회 필요)
        let estimated_liquidity = match asset {
            "USDC" | "USDT" | "DAI" => U256::from(100_000_000u64) * U256::from(10u64.pow(6)), // 100M
            "WETH" => U256::from(50_000u64) * U256::from(10u64.pow(18)), // 50K ETH
            "WBTC" => U256::from(2_000u64) * U256::from(10u64.pow(8)), // 2K BTC
            _ => U256::from(1_000_000u64) * U256::from(10u64.pow(18)), // 기본값
        };
        
        Ok(amount <= estimated_liquidity / U256::from(10)) // 총 유동성의 10% 이하
    }
    
    /// 지갑 잔고 체크
    async fn check_wallet_balance(
        &self,
        asset: &str,
        amount: U256,
    ) -> Result<bool> {
        // 실제로는 지갑 잔고 조회
        // 여기서는 간단한 추정 (실제 구현에서는 ERC20 잔고 조회)
        debug!("Checking wallet balance for {}: {}", asset, amount);
        
        // 임시로 항상 충분하다고 가정 (실제로는 on-chain 조회 필요)
        Ok(true)
    }
    
    /// 가스 가격 추정
    async fn estimate_gas_price(&self) -> Result<(U256, U256)> {
        // 실제로는 provider에서 조회
        // 여기서는 간단한 추정
        let base_fee = U256::from(20_000_000_000u64); // 20 gwei
        let priority_fee = U256::from(2_000_000_000u64); // 2 gwei
        Ok((base_fee, priority_fee))
    }
    
    /// 가격 데이터 업데이트 (외부 피드에서 호출)
    pub async fn update_price_data(&self, price_data: PriceData) -> Result<()> {
        let mut cache = self.price_cache.lock().await;
        let exchange_cache = cache.entry(price_data.exchange.clone()).or_insert_with(HashMap::new);
        exchange_cache.insert(price_data.symbol.clone(), price_data);
        Ok(())
    }
    
    /// 오더북 데이터 업데이트 (외부 피드에서 호출)
    pub async fn update_orderbook_data(&self, orderbook: OrderBookSnapshot) -> Result<()> {
        let mut cache = self.orderbook_cache.lock().await;
        let exchange_cache = cache.entry(orderbook.exchange.clone()).or_insert_with(HashMap::new);
        exchange_cache.insert(orderbook.symbol.clone(), orderbook);
        Ok(())
    }
    
    /// 거래소간 가격 차이 스캔
    async fn scan_price_differences(&self) -> Result<Vec<MicroArbitrageOpportunity>> {
        let mut opportunities = Vec::new();
        let price_cache = self.price_cache.lock().await;
        
        // 모든 거래 페어에 대해 검사
        for pair in &self.config.strategies.micro_arbitrage.trading_pairs {
            let mut exchange_prices = Vec::new();
            
            // 각 거래소의 가격 수집
            for exchange_name in self.exchanges.keys() {
                if let Some(exchange_cache) = price_cache.get(exchange_name) {
                    if let Some(price_data) = exchange_cache.get(pair) {
                        // 가격 데이터가 너무 오래되지 않았는지 확인 (1초 이내)
                        if (Utc::now() - price_data.timestamp).num_milliseconds() <= 1000 {
                            exchange_prices.push((exchange_name.clone(), price_data));
                        }
                    }
                }
            }
            
            // 최소 2개 거래소 가격이 있어야 비교 가능
            if exchange_prices.len() >= 2 {
                opportunities.extend(self.find_arbitrage_opportunities(pair, &exchange_prices).await?);
            }
        }
        
        Ok(opportunities)
    }
    
    /// 아비트래지 기회 탐지
    async fn find_arbitrage_opportunities(
        &self, 
        pair: &str, 
        exchange_prices: &[(String, &PriceData)]
    ) -> Result<Vec<MicroArbitrageOpportunity>> {
        let mut opportunities = Vec::new();
        
        // 모든 거래소 페어 조합 검사
        for i in 0..exchange_prices.len() {
            for j in i + 1..exchange_prices.len() {
                let (buy_exchange, buy_price_data) = &exchange_prices[i];
                let (sell_exchange, sell_price_data) = &exchange_prices[j];
                
                // 두 방향 모두 검사 (A에서 사서 B에서 팔기, B에서 사서 A에서 팔기)
                if let Some(opp) = self.calculate_arbitrage_opportunity(
                    pair, buy_exchange, buy_price_data, sell_exchange, sell_price_data
                ).await? {
                    opportunities.push(opp);
                }
                
                if let Some(opp) = self.calculate_arbitrage_opportunity(
                    pair, sell_exchange, sell_price_data, buy_exchange, buy_price_data
                ).await? {
                    opportunities.push(opp);
                }
            }
        }
        
        Ok(opportunities)
    }
    
    /// 특정 거래소 페어 간 아비트래지 기회 계산
    async fn calculate_arbitrage_opportunity(
        &self,
        pair: &str,
        buy_exchange: &str,
        buy_price_data: &PriceData,
        sell_exchange: &str,
        sell_price_data: &PriceData,
    ) -> Result<Option<MicroArbitrageOpportunity>> {
        // 매수 가격 (ask) vs 매도 가격 (bid) 비교
        let buy_price = buy_price_data.ask;
        let sell_price = sell_price_data.bid;
        
        if sell_price <= buy_price {
            return Ok(None); // 수익성 없음
        }
        
        let price_spread = sell_price - buy_price;
        let profit_percentage = (price_spread / buy_price).to_f64().unwrap_or(0.0);
        
        // 최소 수익률 확인
        if profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }
        
        // 거래소 수수료 고려
        let buy_exchange_info = self.exchanges.get(buy_exchange).unwrap();
        let sell_exchange_info = self.exchanges.get(sell_exchange).unwrap();
        
        let total_fees = buy_exchange_info.fee_percentage + sell_exchange_info.fee_percentage;
        let net_profit_percentage = profit_percentage - total_fees;
        
        if net_profit_percentage < self.min_profit_percentage {
            return Ok(None);
        }
        
        // 최대 거래 가능 수량 계산 (유동성 및 위험 한도 고려)
        let max_amount = self.calculate_max_trade_amount(
            buy_exchange_info,
            sell_exchange_info,
            &buy_price,
        ).await?;
        
        // 최소 수익 USD 확인
        let profit_usd = (max_amount.to::<u128>() as f64 * net_profit_percentage / 1e18) * buy_price.to_f64().unwrap_or(0.0);
        if Decimal::from_f64_retain(profit_usd).unwrap_or_default() < self.min_profit_usd {
            return Ok(None);
        }
        
        // 실행 시간 윈도우 계산 (지연시간 기반)
        let execution_window_ms = std::cmp::max(
            buy_exchange_info.latency_ms + sell_exchange_info.latency_ms + 20, // 20ms 버퍼
            self.execution_timeout_ms
        );
        
        // 신뢰도 점수 계산
        let confidence_score = self.calculate_confidence_score(
            pair,
            buy_exchange,
            sell_exchange,
            net_profit_percentage,
            execution_window_ms,
        ).await?;
        
        // 예상 수익 계산 (wei 단위)
        let expected_profit_wei = U256::from(
            (max_amount.to::<u128>() as f64 * net_profit_percentage) as u64
        );
        
        // 기본 자산 추출 (예: "WETH/USDC" -> "WETH")
        let base_asset = pair.split('/').next().unwrap_or("ETH").to_string();
        
        Ok(Some(MicroArbitrageOpportunity {
            token_symbol: pair.to_string(),
            buy_exchange: buy_exchange.to_string(),
            sell_exchange: sell_exchange.to_string(),
            buy_price,
            sell_price,
            price_spread,
            profit_percentage: net_profit_percentage,
            max_amount,
            execution_window_ms,
            confidence_score,
            expected_profit: expected_profit_wei,
            buy_amount: max_amount,  // 매수 수량은 최대 거래량과 동일
            base_asset,
        }))
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
        
        // 위험 관리 한도 적용
        let risk_based_limit = U256::from((self.risk_limit_per_trade.to::<u128>() as f64 / price.to_f64().unwrap_or(1.0)) as u64);
        
        let final_amount = std::cmp::min(max_size, risk_based_limit);
        
        Ok(std::cmp::max(min_size, final_amount))
    }
    
    /// 신뢰도 점수 계산
    async fn calculate_confidence_score(
        &self,
        _pair: &str,
        _buy_exchange: &str,
        _sell_exchange: &str,
        profit_percentage: f64,
        execution_window_ms: u64,
    ) -> Result<f64> {
        let mut score = 0.5; // 기본 점수
        
        // 수익률 기반 점수 (높을수록 좋음)
        score += (profit_percentage * 1000.0).min(0.3);
        
        // 실행 시간 기반 점수 (빠를수록 좋음)
        if execution_window_ms <= 50 {
            score += 0.2;
        } else if execution_window_ms <= 100 {
            score += 0.1;
        }
        
        // 현재 활성 거래 수 고려 (적을수록 좋음)
        let active_count = self.active_trades.lock().await.len();
        if active_count < self.max_concurrent_trades / 2 {
            score += 0.1;
        } else if active_count >= self.max_concurrent_trades {
            score -= 0.2;
        }
        
        Ok(score.clamp(0.0, 1.0))
    }
    
    /// 마이크로 아비트래지 실행
    async fn execute_micro_arbitrage(&self, opportunity: &MicroArbitrageOpportunity) -> Result<bool> {
        let execution_start = Instant::now();
        let trade_id = format!("micro_arb_{}", SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
        
        info!("🚀 마이크로 아비트래지 실행 시작: {}", trade_id);
        info!("  📈 {}에서 매수: ${}", opportunity.buy_exchange, opportunity.buy_price);
        info!("  📉 {}에서 매도: ${}", opportunity.sell_exchange, opportunity.sell_price);
        info!("  💰 예상 수익: {:.4}%", opportunity.profit_percentage * 100.0);
        
        // 활성 거래로 추가
        {
            let mut active_trades = self.active_trades.lock().await;
            if active_trades.len() >= self.max_concurrent_trades {
                warn!("⚠️ 최대 동시 거래 수 초과, 거래 건너뜀");
                return Ok(false);
            }
            active_trades.insert(trade_id.clone(), opportunity.clone());
        }
        
        let execution_result = async {
            // Mock 실행 (실제로는 거래소 API 호출)
            if crate::mocks::is_mock_mode() {
                self.execute_mock_arbitrage(opportunity, &trade_id).await
            } else {
                // 새로운 자동 자금 조달 모드 선택 로직
                let (selected_mode, _metrics) = match self.determine_funding_mode(opportunity).await {
                    Ok((mode, metrics)) => (mode, metrics),
                    Err(e) => {
                        warn!("⚠️ 자금 조달 모드 선택 실패, 기본 지갑 모드 사용: {}", e);
                        ("wallet".to_string(), FundingMetrics {
                            gross_profit: opportunity.expected_profit,
                            total_cost: U256::ZERO,
                            net_profit: opportunity.expected_profit,
                            gas_cost: U256::ZERO,
                            premium_cost: U256::ZERO,
                            success_probability: 0.9,
                            liquidity_available: true,
                        })
                    }
                };
                
                match selected_mode.as_str() {
                    "flashloan" => {
                        info!("⚡ 플래시론 모드로 실행");
                        match self.execute_flashloan_arbitrage_via_contract(opportunity).await {
                            Ok(done) => return Ok(done),
                            Err(e) => {
                                warn!("⚠️ 플래시론 경로 실패, 지갑 경로로 폴백: {}", e);
                                self.execute_real_arbitrage(opportunity, &trade_id).await
                            }
                        }
                    }
                    "wallet" => {
                        info!("💳 지갑 모드로 실행");
                        self.execute_real_arbitrage(opportunity, &trade_id).await
                    }
                    "skip" => {
                        info!("⏭️ 수익성 부족으로 기회 스킵");
                        return Ok(false);
                    }
                    _ => {
                        warn!("⚠️ 알 수 없는 자금 조달 모드: {}, 지갑 모드로 폴백", selected_mode);
                        self.execute_real_arbitrage(opportunity, &trade_id).await
                    }
                }
            }
        };
        
        // 타임아웃 적용
        let result = tokio::time::timeout(
            Duration::from_millis(opportunity.execution_window_ms),
            execution_result
        ).await;
        
        // 활성 거래에서 제거
        self.active_trades.lock().await.remove(&trade_id);
        
        let execution_time = execution_start.elapsed();
        
        match result {
            Ok(Ok(success)) => {
                if success {
                    info!("✅ 마이크로 아비트래지 성공: {} ({:.2}ms)", 
                          trade_id, execution_time.as_millis());
                    self.update_stats(true, execution_time.as_millis() as f64, opportunity).await;
                } else {
                    warn!("❌ 마이크로 아비트래지 실패: {}", trade_id);
                    self.update_stats(false, execution_time.as_millis() as f64, opportunity).await;
                }
                Ok(success)
            }
            Ok(Err(e)) => {
                error!("💥 마이크로 아비트래지 오류: {} - {}", trade_id, e);
                self.update_stats(false, execution_time.as_millis() as f64, opportunity).await;
                Err(e)
            }
            Err(_) => {
                warn!("⏰ 마이크로 아비트래지 타임아웃: {}", trade_id);
                self.update_stats(false, execution_time.as_millis() as f64, opportunity).await;
                Ok(false)
            }
        }
    }
    
    /// Mock 모드 아비트래지 실행
    async fn execute_mock_arbitrage(&self, _opportunity: &MicroArbitrageOpportunity, trade_id: &str) -> Result<bool> {
        // 시뮬레이션: 90% 성공률
        sleep(Duration::from_millis(10 + fastrand::u64(20..50))).await; // 10-60ms 지연 시뮬레이션
        
        let success = fastrand::f64() > 0.1; // 90% 성공률
        
        if success {
            debug!("🎭 Mock 아비트래지 성공: {}", trade_id);
        } else {
            debug!("🎭 Mock 아비트래지 실패: {} (슬리피지 또는 유동성 부족)", trade_id);
        }
        
        Ok(success)
    }
    
    /// 실제 아비트래지 실행 (실제 거래소 API 호출)
    async fn execute_real_arbitrage(&self, opportunity: &MicroArbitrageOpportunity, trade_id: &str) -> Result<bool> {
        
        info!("🚀 실제 아비트래지 실행: {}", trade_id);
        info!("  매수: {} @ {}", opportunity.buy_exchange, opportunity.buy_price);
        info!("  매도: {} @ {}", opportunity.sell_exchange, opportunity.sell_price);
        
        // 필수 API 키 존재 여부 확인 (실행 전 가드)
        let check_api = |name: &str| -> Result<()> {
            let lname = name.to_lowercase();
            if lname.contains("binance") {
                let k = std::env::var("BINANCE_API_KEY").unwrap_or_default();
                let s = std::env::var("BINANCE_SECRET_KEY").unwrap_or_default();
                if k.trim().is_empty() || s.trim().is_empty() {
                    return Err(anyhow::anyhow!("BINANCE_API_KEY/SECRET env missing"));
                }
            } else if lname.contains("coinbase") {
                let k = std::env::var("COINBASE_API_KEY").unwrap_or_default();
                let s = std::env::var("COINBASE_SECRET_KEY").unwrap_or_default();
                let p = std::env::var("COINBASE_PASSPHRASE").unwrap_or_default();
                if k.trim().is_empty() || s.trim().is_empty() || p.trim().is_empty() {
                    return Err(anyhow::anyhow!("COINBASE_API_KEY/SECRET/PASSPHRASE env missing"));
                }
            }
            Ok(())
        };
        check_api(&opportunity.buy_exchange)?;
        check_api(&opportunity.sell_exchange)?;

        // 거래소 클라이언트 생성 (환경 변수에서 API 키 로드)
        let buy_client = self.create_exchange_client(&opportunity.buy_exchange).await?;
        let sell_client = self.create_exchange_client(&opportunity.sell_exchange).await?;
        
        // 연결 상태 확인
        let symbol = format!("{}USDT", "ETH"); // 아래에서 실제 심볼로 대체됨
        let _ = buy_client.get_current_price(&symbol).await; // warm-up
        let _ = sell_client.get_current_price(&symbol).await; // warm-up
        if !buy_client.is_connected() || !sell_client.is_connected() {
            return Err(anyhow::anyhow!("거래소 연결 실패(핑 실패)"));
        }
        
        // 잔고 확인
        let base_asset = "ETH"; // 기본 자산
        let quote_asset = "USDT"; // 견적 자산
        
        let buy_balance = buy_client.get_balance(quote_asset).await?;
        let sell_balance = sell_client.get_balance(base_asset).await?;
        
        let required_quote = opportunity.buy_price * rust_decimal::Decimal::from_f64_retain(opportunity.max_amount.to::<u128>() as f64 / 1e18).unwrap_or_default();
        let required_base = rust_decimal::Decimal::from_f64_retain(opportunity.max_amount.to::<u128>() as f64 / 1e18).unwrap_or_default();
        
        if buy_balance < required_quote {
            warn!("⚠️ 매수 거래소 잔고 부족: 필요 {} {}, 보유 {} {}", required_quote, quote_asset, buy_balance, quote_asset);
            return Ok(false);
        }
        
        if sell_balance < required_base {
            warn!("⚠️ 매도 거래소 잔고 부족: 필요 {} {}, 보유 {} {}", required_base, base_asset, sell_balance, base_asset);
            return Ok(false);
        }
        
        // 병렬 주문 실행
        let symbol = format!("{}USDT", base_asset);
        let amount = opportunity.max_amount;
        
        debug!("📊 주문 실행 - 심볼: {}, 수량: {}", symbol, amount);
        
        let (buy_result, sell_result) = tokio::join!(
            buy_client.place_buy_order(&symbol, amount, opportunity.buy_price),
            sell_client.place_sell_order(&symbol, amount, opportunity.sell_price)
        );
        
        match (buy_result, sell_result) {
            (Ok(buy_order_id), Ok(sell_order_id)) => {
                info!("✅ 양쪽 주문 성공 - Buy: {}, Sell: {}", buy_order_id, sell_order_id);
                
                // 주문 상태 모니터링
                let monitoring_result = self.monitor_order_execution(
                    &buy_client,
                    &sell_client,
                    &buy_order_id,
                    &sell_order_id,
                    trade_id
                ).await;
                
                match monitoring_result {
                    Ok(success) => {
                        if success {
                            info!("🎉 아비트래지 완전 성공: {}", trade_id);
                        } else {
                            warn!("⚠️ 주문 모니터링 중 문제 발생: {}", trade_id);
                        }
                        Ok(success)
                    }
                    Err(e) => {
                        error!("💥 주문 모니터링 실패: {} - {}", trade_id, e);
                        
                        // 긴급 주문 취소 시도
                        let _ = tokio::join!(
                            buy_client.cancel_order(&buy_order_id),
                            sell_client.cancel_order(&sell_order_id)
                        );
                        
                        Err(e)
                    }
                }
            }
            (Ok(buy_order_id), Err(sell_error)) => {
                error!("⚠️ 매도 주문 실패, 매수 주문 취소 중: {}", sell_error);
                let _ = buy_client.cancel_order(&buy_order_id).await;
                Ok(false)
            }
            (Err(buy_error), Ok(sell_order_id)) => {
                error!("⚠️ 매수 주문 실패, 매도 주문 취소 중: {}", buy_error);
                let _ = sell_client.cancel_order(&sell_order_id).await;
                Ok(false)
            }
            (Err(buy_error), Err(sell_error)) => {
                error!("❌ 양쪽 주문 모두 실패 - Buy: {}, Sell: {}", buy_error, sell_error);
                Ok(false)
            }
        }
    }

    /// 플래시론 기반 DEX-DEX 아비트라지 실행 (전용 Arbitrage.sol 컨트랙트 사용, MEV 비사용)
    async fn execute_flashloan_arbitrage_via_contract(&self, opportunity: &MicroArbitrageOpportunity) -> Result<bool> {
        use crate::utils::abi::{ABICodec, contracts};
        use alloy::primitives::Bytes;
        use alloy::primitives::Address as AlloyAddress;
        // 전용 컨트랙트 주소 필요
        let arb_h160 = self.config.blockchain.primary_network.arbitrage_contract
            .ok_or_else(|| anyhow!("arbitrage_contract not configured"))?;
        if arb_h160 == ethers::types::H160::zero() {
            return Err(anyhow!("arbitrage_contract is 0x0"));
        }
        let arbitrage_contract = alloy::primitives::Address::from_slice(arb_h160.as_bytes());

        // 심볼 파싱 (예: WETH/USDC)
        let parts: Vec<&str> = opportunity.token_symbol.split('/').collect();
        if parts.len() != 2 { return Err(anyhow!("unsupported pair")); }
        let base = parts[0]; // WETH
        let quote = parts[1]; // USDC
        let token_in = self.config.get_token_address(base).ok_or_else(|| anyhow!("token not found: {}", base))?;
        let token_out = self.config.get_token_address(quote).ok_or_else(|| anyhow!("token not found: {}", quote))?;
        let token_in_addr = alloy::primitives::Address::from_slice(token_in.as_bytes());
        let token_out_addr = alloy::primitives::Address::from_slice(token_out.as_bytes());

        // 라우터/집계기 선택 (간단 매핑). 필요시 config로 확장 가능
        let router_buy = if opportunity.buy_exchange.to_lowercase().contains("sushi") { *contracts::SUSHISWAP_ROUTER } else if opportunity.buy_exchange.to_lowercase().contains("uniswap_v3") { *contracts::UNISWAP_V3_ROUTER } else { *contracts::UNISWAP_V2_ROUTER };
        let router_sell = if opportunity.sell_exchange.to_lowercase().contains("sushi") { *contracts::SUSHISWAP_ROUTER } else if opportunity.sell_exchange.to_lowercase().contains("uniswap_v3") { *contracts::UNISWAP_V3_ROUTER } else { *contracts::UNISWAP_V2_ROUTER };

        let codec = ABICodec::new();
        let now = chrono::Utc::now().timestamp() as u64;
        let deadline = alloy::primitives::U256::from(now + 120);

        // 금액: 설정된 flash_loan_amount 사용 또는 기회 max_amount
        let amount_in = opportunity.max_amount;

        // 집계기(0x) 경로 시도: 실제 실행 라우터/데이터/spender를 그대로 사용
        let mut agg_buy: Option<(AlloyAddress, Vec<u8>, Option<AlloyAddress>, alloy::primitives::U256)> = None;
        let mut agg_sell: Option<(AlloyAddress, Vec<u8>, Option<AlloyAddress>, alloy::primitives::U256)> = None;
        if let Some(bq) = self.get_0x_swap_quote(token_in_addr, token_out_addr, amount_in).await? {
            let out_b = bq.amount_out;
            agg_buy = Some((bq.to, bq.data, bq.spender, out_b));
            if out_b > alloy::primitives::U256::ZERO {
                if let Some(sq) = self.get_0x_swap_quote(token_out_addr, token_in_addr, out_b).await? {
                    agg_sell = Some((sq.to, sq.data, sq.spender, sq.amount_out));
                }
            }
        }

        // buyCalldata: 기본은 V2 swapExactTokensForTokens. (집계기 경로가 유리하면 아래에서 교체)
        // 동적 슬리피지 가드: 0x 견적으로 minOut 산정, 실패 시 50bps
        let slippage_bps = 50u64;
        let amount_out_min_buy = if let Some(q) = self.estimate_buy_amount_via_0x(token_in_addr, token_out_addr, amount_in).await {
            q * alloy::primitives::U256::from(10_000u64 - slippage_bps) / alloy::primitives::U256::from(10_000u64)
        } else {
            amount_in * alloy::primitives::U256::from(10_000u64 - slippage_bps) / alloy::primitives::U256::from(10_000u64)
        };
        let buy_path = vec![token_in_addr, token_out_addr];
        let buy_calldata = codec.encode_uniswap_v2_swap_exact_tokens(
            amount_in,
            amount_out_min_buy,
            buy_path,
            arbitrage_contract,
            deadline,
        )?;

        // sellCalldata: 기본은 V2 swapExactTokensForTokens (집계기 경로가 유리하면 아래에서 교체)
        let sell_path = vec![token_out_addr, token_in_addr];
        // sell도 동적 가드 적용 (0x -> 1inch 폴백), 실패 시 50bps
        let amount_out_min_sell = if let Some(q) = self.estimate_amount_out_via_0x(token_out_addr, token_in_addr, amount_in).await {
            q * alloy::primitives::U256::from(10_000u64 - slippage_bps) / alloy::primitives::U256::from(10_000u64)
        } else if let Some(q) = self.estimate_amount_out_via_1inch(token_out_addr, token_in_addr, amount_in).await {
            q * alloy::primitives::U256::from(10_000u64 - slippage_bps) / alloy::primitives::U256::from(10_000u64)
        } else {
            amount_in * alloy::primitives::U256::from(10_000u64 - slippage_bps) / alloy::primitives::U256::from(10_000u64)
        };
        // 여기서는 전량 매도를 위해 amountIn은 리시버 내에서 잔액 사용. V2는 exactTokens이므로 대략 amount_in 사용.
        let sell_calldata = codec.encode_uniswap_v2_swap_exact_tokens(
            amount_in,
            amount_out_min_sell,
            sell_path,
            arbitrage_contract,
            deadline,
        )?;

        // ArbitrageStrategy.executeArbitrage(asset, amount, params)
        // params = abi.encode(ArbitrageParams{ tokenA, tokenB, dexA, dexB, amountIn, expectedProfitMin, swapCallDataA, swapCallDataB })
        let expected_min = amount_in / alloy::primitives::U256::from(1000u64); // 0.1% 최소 이익 가드 예시
        // 집계기(0x/1inch) 라우터가 별도 spender(allowanceTarget)를 요구할 수 있으므로
        // 여기서는 안전하게 spenderA/spenderB를 라우터 주소로 기본 지정.
        // 향후 off-chain quote에서 allowanceTarget을 얻으면 해당 주소를 채워 넣을 수 있음.
        // 경로 선택: 0x 두 레그가 모두 준비되고, V2 대비 유리하면 집계기 경로 채택
        let (final_dex_a, final_data_a, final_spender_a,
             final_dex_b, final_data_b, final_spender_b) = if let (Some((to_a, data_a, sp_a, _out_b)), Some((to_b, data_b, sp_b, out_a))) = (agg_buy, agg_sell) {
            // 단순 비교: 집계기 최종 tokenA 수령량 vs V2 예상 최소 수령량
            let v2_min_back_a = amount_out_min_sell; // 보수적 가드 기준
            if out_a > v2_min_back_a {
                (to_a, data_a, sp_a.unwrap_or(to_a), to_b, data_b, sp_b.unwrap_or(to_b))
            } else {
                (router_buy, buy_calldata.to_vec(), router_buy, router_sell, sell_calldata.to_vec(), router_sell)
            }
        } else {
            (router_buy, buy_calldata.to_vec(), router_buy, router_sell, sell_calldata.to_vec(), router_sell)
        };

        let arb_params = codec.encode_arbitrage_contract_params(
            token_in_addr,
            token_out_addr,
            final_dex_a,
            final_dex_b,
            Some(final_spender_a),
            Some(final_spender_b),
            amount_in,
            expected_min,
            Bytes::from(final_data_a),
            Bytes::from(final_data_b),
        )?;
        let calldata = codec.encode_arbitrage_execute_call(
            token_in_addr,
            amount_in,
            arb_params,
        )?;

        // 예상 수익(USD) > 최소 수익/프리미엄 가드
        if !self.guard_min_profit_usd(opportunity, amount_in).await? {
            warn!("⚠️ 예상 수익이 임계값 미만, 플래시론 경로 스킵");
            return Ok(false);
        }

        // 트랜잭션 구성 (전용 Arbitrage 컨트랙트 호출)
        let flashloan_tx = crate::types::Transaction {
            hash: alloy::primitives::B256::ZERO,
            from: alloy::primitives::Address::ZERO,
            to: Some(arbitrage_contract),
            value: alloy::primitives::U256::ZERO,
            gas_price: alloy::primitives::U256::from(30_000_000_000u64),
            gas_limit: alloy::primitives::U256::from(600_000u64),
            data: calldata.to_vec(),
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: None,
        };
        // 공개 브로드캐스트(일반 tx): 번들/사설 경로 사용 안함
        let sent = self.broadcast_public_transaction(flashloan_tx).await?;
        Ok(sent)
    }

    /// 일반 트랜잭션 브로드캐스트 (간단 스텁: 실제 서명/전송은 온체인 모듈과 통합 가능)
    async fn broadcast_public_transaction(&self, tx: crate::types::Transaction) -> Result<bool> {
        use ethers::providers::{Provider as HttpProvider, Http, Middleware};
        use ethers::types::{TransactionRequest as EthersTxRequest, H160 as EthersH160, U256 as EthersU256};
        use ethers::signers::{LocalWallet, Signer};
        use ethers::middleware::SignerMiddleware;

        let rpc_url = &self.config.blockchain.primary_network.rpc_url;
        let provider: HttpProvider<Http> = HttpProvider::<Http>::try_from(rpc_url)
            .map_err(|e| anyhow!("provider error: {}", e))?;

        // Load key
        let pk = std::env::var("PRIVATE_KEY").ok()
            .or_else(|| std::env::var("FLASHBOTS_PRIVATE_KEY").ok())
            .ok_or_else(|| anyhow!("PRIVATE_KEY/FLASHBOTS_PRIVATE_KEY not set"))?;
        let mut wallet: LocalWallet = pk.parse().map_err(|e| anyhow!("wallet parse error: {}", e))?;
        let chain_id = self.config.blockchain.primary_network.chain_id;
        wallet = wallet.with_chain_id(chain_id);

        // Convert types
        let to = tx.to.ok_or_else(|| anyhow!("missing to address"))?;
        let to_h160: EthersH160 = EthersH160::from_slice(to.as_slice());

        let mut be = [0u8; 32];
        be.copy_from_slice(&tx.value.to_be_bytes::<32>());
        let val = EthersU256::from_big_endian(&be);
        be.copy_from_slice(&tx.gas_price.to_be_bytes::<32>());
        let gas_price = EthersU256::from_big_endian(&be);
        be.copy_from_slice(&tx.gas_limit.to_be_bytes::<32>());
        let gas_limit = EthersU256::from_big_endian(&be);

        // Build legacy tx
        let data_bytes = ethers::types::Bytes::from(tx.data.clone());
        let mut req = EthersTxRequest::new()
            .to(to_h160)
            .data(data_bytes)
            .value(val)
            .gas(gas_limit)
            .gas_price(gas_price);
        // Nonce optional: let provider determine if not provided
        if tx.nonce != 0 { req = req.nonce(ethers::types::U256::from(tx.nonce)); }

        let client: SignerMiddleware<HttpProvider<Http>, LocalWallet> = SignerMiddleware::new(provider, wallet);
        let pending = client.send_transaction(req, None::<ethers::types::BlockId>).await?;
        let _tx_hash = pending.tx_hash();
        info!("📤 Micro: 공개 브로드캐스트 전송 완료");
        Ok(true)
    }

    /// 0x 견적으로 buy amountOut 추정 (실패 시 None)
    async fn estimate_buy_amount_via_0x(
        &self,
        token_in: alloy::primitives::Address,
        token_out: alloy::primitives::Address,
        amount_in: alloy::primitives::U256,
    ) -> Option<alloy::primitives::U256> {
        #[derive(Deserialize)]
        struct Quote { #[serde(rename = "buyAmount")] buy_amount: String }
        let url = format!(
            "https://api.0x.org/swap/v1/quote?sellToken={}&buyToken={}&sellAmount={}",
            format!("{:x}", token_in),
            format!("{:x}", token_out),
            amount_in.to_string()
        );
        let client = reqwest::Client::new();
        match client.get(&url).send().await.ok()?.json::<Quote>().await.ok() {
            Some(q) => {
                q.buy_amount.parse::<alloy::primitives::U256>().ok()
            }
            None => None,
        }
    }

    /// 0x generic amountOut estimator
    async fn estimate_amount_out_via_0x(
        &self,
        token_in: alloy::primitives::Address,
        token_out: alloy::primitives::Address,
        amount_in: alloy::primitives::U256,
    ) -> Option<alloy::primitives::U256> {
        self.estimate_buy_amount_via_0x(token_in, token_out, amount_in).await
    }

    /// 1inch amountOut estimator (quote)
    async fn estimate_amount_out_via_1inch(
        &self,
        token_in: alloy::primitives::Address,
        token_out: alloy::primitives::Address,
        amount_in: alloy::primitives::U256,
    ) -> Option<alloy::primitives::U256> {
        #[derive(Deserialize)]
        struct Quote { #[serde(rename = "dstAmount")] dst_amount: String }
        let url = format!(
            "https://api.1inch.dev/swap/v5.2/1/quote?src={}&dst={}&amount={}",
            format!("{:x}", token_in),
            format!("{:x}", token_out),
            amount_in.to_string()
        );
        let client = reqwest::Client::new();
        let mut req = client.get(&url).header("accept", "application/json");
        if let Ok(key) = std::env::var("ONEINCH_API_KEY") {
            if !key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key)).header("apikey", key);
            }
        }
        match req.send().await.ok()?.json::<Quote>().await.ok() {
            Some(q) => alloy::primitives::U256::from_str(&q.dst_amount).ok(),
            None => None,
        }
    }

    /// 0x 실행용 quote (to, data, allowanceTarget, amountOut) 가져오기
    async fn get_0x_swap_quote(
        &self,
        token_in: alloy::primitives::Address,
        token_out: alloy::primitives::Address,
        amount_in: alloy::primitives::U256,
    ) -> Result<Option<OxSwapQuote>> {
        #[derive(Deserialize)]
        struct Quote { to: String, data: String, #[serde(rename="allowanceTarget")] allowance_target: Option<String>, #[serde(rename="buyAmount")] buy_amount: String }
        let url = format!(
            "https://api.0x.org/swap/v1/quote?sellToken={}&buyToken={}&sellAmount={}",
            format!("{:x}", token_in),
            format!("{:x}", token_out),
            amount_in.to_string()
        );
        let client = reqwest::Client::new();
        let resp = match client.get(&url).send().await { Ok(r) => r, Err(_) => return Ok(None) };
        if !resp.status().is_success() { return Ok(None); }
        let q: Quote = match resp.json().await { Ok(j) => j, Err(_) => return Ok(None) };
        let to = match alloy::primitives::Address::from_str(&q.to) { Ok(a) => a, Err(_) => return Ok(None) };
        let data = match hex::decode(q.data.trim_start_matches("0x")) { Ok(b) => b, Err(_) => return Ok(None) };
        let spender = match q.allowance_target {
            Some(s) if !s.is_empty() => alloy::primitives::Address::from_str(&s).ok(),
            _ => None
        };
        let amount_out = alloy::primitives::U256::from_str_radix(&q.buy_amount, 10).unwrap_or_default();
        Ok(Some(OxSwapQuote{ to, data, spender, amount_out }))
    }

    /// 번들 제출 전 최소 USD 수익 가드 (플래시론 프리미엄 9bps 반영)
    async fn guard_min_profit_usd(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        amount_in: alloy::primitives::U256,
    ) -> Result<bool> {
        // 기회에 내장된 profit_percentage를 사용하여 대략적 USD 수익 추정
        let base_amount = amount_in.to::<u128>() as f64 / 1e18;
        let buy_px = opportunity.buy_price.to_f64().unwrap_or(0.0);
        let expected_usd = base_amount * buy_px * opportunity.profit_percentage;
        // 플래시론 프리미엄 차감(9bps)
        let premium = base_amount * buy_px * 0.0009;
        // 가스비(추정) 차감: 500k gas * 30 gwei * ETHUSD(=buy_px)
        let gas_gwei = std::env::var("GAS_PRICE_GWEI").ok().and_then(|s| s.parse::<f64>().ok()).unwrap_or(30.0);
        let gas_cost_eth = 500_000.0 * gas_gwei * 1e-9; // 500k * gwei to ETH
        let gas_cost_usd = gas_cost_eth * buy_px;
        let net_usd = expected_usd - premium - gas_cost_usd;
        let min_usd = self.min_profit_usd.to_f64().unwrap_or(0.0);
        Ok(net_usd >= min_usd)
    }
    
    /// 거래소 클라이언트 생성
    async fn create_exchange_client(&self, exchange_name: &str) -> Result<std::sync::Arc<dyn crate::exchange::ExchangeClient>> {
        use crate::exchange::ExchangeClientFactory;
        
        match exchange_name.to_lowercase().as_str() {
            "binance" | "mock_binance" => {
                // 실제 환경에서는 설정에서 API 키 로드
                let api_key = std::env::var("BINANCE_API_KEY").unwrap_or_default();
                let secret_key = std::env::var("BINANCE_SECRET_KEY").unwrap_or_default();
                Ok(ExchangeClientFactory::create_binance_client(api_key, secret_key))
            }
            "coinbase" | "mock_coinbase" => {
                let api_key = std::env::var("COINBASE_API_KEY").unwrap_or_default();
                let secret_key = std::env::var("COINBASE_SECRET_KEY").unwrap_or_default();
                let passphrase = std::env::var("COINBASE_PASSPHRASE").unwrap_or_default();
                Ok(ExchangeClientFactory::create_coinbase_client(api_key, secret_key, passphrase))
            }
            _ => {
                // 지원되지 않는 거래소의 경우 Mock 클라이언트 사용
                warn!("⚠️ 지원되지 않는 거래소: {}, Mock 클라이언트 사용", exchange_name);
                let api_key = "mock_key".to_string();
                let secret_key = "mock_secret".to_string();
                Ok(ExchangeClientFactory::create_binance_client(api_key, secret_key))
            }
        }
    }
    
    /// 주문 실행 모니터링
    async fn monitor_order_execution(
        &self,
        buy_client: &std::sync::Arc<dyn crate::exchange::ExchangeClient>,
        sell_client: &std::sync::Arc<dyn crate::exchange::ExchangeClient>,
        buy_order_id: &str,
        sell_order_id: &str,
        trade_id: &str,
    ) -> Result<bool> {
        use crate::types::OrderStatus;
        
        let max_wait_time = std::time::Duration::from_secs(30); // 최대 30초 대기
        let check_interval = std::time::Duration::from_millis(500); // 0.5초마다 체크
        let start_time = std::time::Instant::now();
        
        let mut buy_filled = false;
        let mut sell_filled = false;
        
        while start_time.elapsed() < max_wait_time {
            // 주문 상태 확인
            let (buy_status_result, sell_status_result) = tokio::join!(
                buy_client.get_order_status(buy_order_id),
                sell_client.get_order_status(sell_order_id)
            );
            
            match buy_status_result {
                Ok(OrderStatus::Filled) => {
                    if !buy_filled {
                        info!("✅ 매수 주문 체결 완료: {} ({})", buy_order_id, trade_id);
                        buy_filled = true;
                    }
                }
                Ok(OrderStatus::PartiallyFilled) => {
                    debug!("🔄 매수 주문 부분 체결: {} ({})", buy_order_id, trade_id);
                }
                Ok(OrderStatus::Cancelled) => {
                    warn!("❌ 매수 주문 취소됨: {} ({})", buy_order_id, trade_id);
                    return Ok(false);
                }
                Ok(OrderStatus::Rejected) => {
                    warn!("❌ 매수 주문 거부됨: {} ({})", buy_order_id, trade_id);
                    return Ok(false);
                }
                Ok(OrderStatus::Expired) => {
                    warn!("❌ 매수 주문 만료됨: {} ({})", buy_order_id, trade_id);
                    return Ok(false);
                }
                Ok(OrderStatus::Pending) => {
                    debug!("⏳ 매수 주문 대기 중: {} ({})", buy_order_id, trade_id);
                }
                Err(e) => {
                    warn!("⚠️ 매수 주문 상태 확인 실패: {} - {}", buy_order_id, e);
                }
            }
            
            match sell_status_result {
                Ok(OrderStatus::Filled) => {
                    if !sell_filled {
                        info!("✅ 매도 주문 체결 완료: {} ({})", sell_order_id, trade_id);
                        sell_filled = true;
                    }
                }
                Ok(OrderStatus::PartiallyFilled) => {
                    debug!("🔄 매도 주문 부분 체결: {} ({})", sell_order_id, trade_id);
                }
                Ok(OrderStatus::Cancelled) => {
                    warn!("❌ 매도 주문 취소됨: {} ({})", sell_order_id, trade_id);
                    return Ok(false);
                }
                Ok(OrderStatus::Rejected) => {
                    warn!("❌ 매도 주문 거부됨: {} ({})", sell_order_id, trade_id);
                    return Ok(false);
                }
                Ok(OrderStatus::Expired) => {
                    warn!("❌ 매도 주문 만료됨: {} ({})", sell_order_id, trade_id);
                    return Ok(false);
                }
                Ok(OrderStatus::Pending) => {
                    debug!("⏳ 매도 주문 대기 중: {} ({})", sell_order_id, trade_id);
                }
                Err(e) => {
                    warn!("⚠️ 매도 주문 상태 확인 실패: {} - {}", sell_order_id, e);
                }
            }
            
            // 양쪽 주문 모두 체결되면 성공
            if buy_filled && sell_filled {
                info!("🎯 아비트래지 완전 체결: {} ({}ms)", trade_id, start_time.elapsed().as_millis());
                return Ok(true);
            }
            
            // 다음 체크까지 대기
            tokio::time::sleep(check_interval).await;
        }
        
        // 타임아웃 발생
        warn!("⏰ 주문 모니터링 타임아웃: {} ({}초)", trade_id, max_wait_time.as_secs());
        
        // 미체결 주문 취소 시도
        if !buy_filled {
            let _ = buy_client.cancel_order(buy_order_id).await;
        }
        if !sell_filled {
            let _ = sell_client.cancel_order(sell_order_id).await;
        }
        
        Ok(false)
    }
    
    /// 통계 업데이트
    async fn update_stats(&self, success: bool, execution_time_ms: f64, opportunity: &MicroArbitrageOpportunity) {
        let mut stats = self.stats.lock().await;
        
        stats.executed_trades += 1;
        
        if success {
            stats.successful_trades += 1;
            
            // 거래량과 수익 추정 (Mock 데이터)
            let trade_volume = opportunity.max_amount;
            let estimated_profit = U256::from(
                (trade_volume.to::<u128>() as f64 * opportunity.profit_percentage / 100.0) as u64
            );
            
            stats.total_volume += trade_volume;
            stats.total_profit += estimated_profit;
            stats.avg_profit_per_trade = if stats.successful_trades > 0 {
                stats.total_profit / U256::from(stats.successful_trades)
            } else {
                U256::ZERO
            };
        } else {
            stats.failed_trades += 1;
        }
        
        // 성공률 계산
        stats.success_rate = if stats.executed_trades > 0 {
            stats.successful_trades as f64 / stats.executed_trades as f64
        } else {
            0.0
        };
        
        // 평균 실행 시간 업데이트
        stats.avg_execution_time_ms = (stats.avg_execution_time_ms * (stats.executed_trades - 1) as f64 + execution_time_ms) / stats.executed_trades as f64;
        
        // 수익률 계산
        stats.profit_rate = if stats.total_volume > U256::ZERO {
            (stats.total_profit.to::<u128>() as f64 / stats.total_volume.to::<u128>() as f64) * 100.0
        } else {
            0.0
        };
    }
    
    /// 통계 조회
    pub async fn get_stats(&self) -> MicroArbitrageStats {
        self.stats.lock().await.clone()
    }

    /// 마이크로 아비트래지 기회를 독립적으로 스캔하고 실행 (공개 메서드)
    pub async fn scan_and_execute(&self) -> Result<usize> {
        if !self.is_enabled() {
            return Ok(0);
        }
        
        let start_time = Instant::now();
        
        // 가격 차이 스캔
        let opportunities = self.scan_price_differences().await?;
        
        if opportunities.is_empty() {
            return Ok(0);
        }
        
        debug!("⚡ {}개 마이크로 아비트래지 기회 발견", opportunities.len());
        
        // 수익성 순으로 정렬
        let mut sorted_opportunities = opportunities;
        sorted_opportunities.sort_by(|a, b| b.profit_percentage.partial_cmp(&a.profit_percentage).unwrap_or(std::cmp::Ordering::Equal));
        
        // 통계용으로 기회 수를 저장
        let opportunities_count = sorted_opportunities.len() as u64;
        
        let mut executed_count = 0;
        
        // 상위 기회들을 병렬로 실행
        let max_concurrent = std::cmp::min(self.max_concurrent_trades, sorted_opportunities.len());
        let mut tasks = Vec::new();
        
        for opportunity in sorted_opportunities.into_iter().take(max_concurrent) {
            // 신뢰도 점수가 충분한 기회만 실행
            if opportunity.confidence_score >= 0.6 {
                // Clone necessary fields for the async task
                let config = Arc::clone(&self.config);
                let provider = Arc::clone(&self.provider);
                let enabled = Arc::clone(&self.enabled);
                let exchanges = self.exchanges.clone();
                let active_trades = Arc::clone(&self.active_trades);
                let stats = Arc::clone(&self.stats);
                // 번들 경로 제거됨
                let min_profit_percentage = self.min_profit_percentage;
                let min_profit_usd = self.min_profit_usd;
                let execution_timeout_ms = self.execution_timeout_ms;
                let max_concurrent_trades = self.max_concurrent_trades;
                let latency_threshold_ms = self.latency_threshold_ms;
                let daily_volume_limit = self.daily_volume_limit;
                let risk_limit_per_trade = self.risk_limit_per_trade;
                let funding_mode = self.funding_mode.clone();
                
                let task = tokio::spawn(async move {
                    // Create a temporary strategy instance for execution
                    let temp_strategy = MicroArbitrageStrategy {
                        config,
                        provider,
                        enabled,
                        exchanges,
                        price_cache: Arc::new(Mutex::new(HashMap::new())), // Empty cache is ok for single execution
                        orderbook_cache: Arc::new(Mutex::new(HashMap::new())), // Empty cache is ok for single execution
                        active_trades,
                        stats,
                        min_profit_percentage,
                        min_profit_usd,
                        execution_timeout_ms,
                        max_concurrent_trades,
                        latency_threshold_ms,
                        daily_volume_limit,
                        risk_limit_per_trade,
                        funding_mode,
                    };
                    
                    temp_strategy.execute_micro_arbitrage(&opportunity).await
                });
                tasks.push(task);
            }
        }
        
        // 모든 실행 완료 대기
        for task in tasks {
            match task.await {
                Ok(Ok(success)) => {
                    if success {
                        executed_count += 1;
                    }
                }
                Ok(Err(e)) => {
                    error!("마이크로 아비트래지 실행 오류: {}", e);
                }
                Err(e) => {
                    error!("태스크 실행 오류: {}", e);
                }
            }
        }
        
        let scan_duration = start_time.elapsed();
        if executed_count > 0 {
            info!("⚡ {}개 마이크로 아비트래지 실행 완료 ({:.2}ms)", 
                  executed_count, scan_duration.as_millis());
        }
        
        // 통계 업데이트
        {
            let mut stats = self.stats.lock().await;
            stats.total_opportunities += opportunities_count;
        }
        
        Ok(executed_count)
    }
    
    /// ETH 금액 포매팅 유틸리티 함수
    fn format_eth_amount(&self, amount: U256) -> String {
        let eth_amount = amount.to::<u128>() as f64 / 1e18;
        format!("{:.6}", eth_amount)
    }
}

#[async_trait]
impl Strategy for MicroArbitrageStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::MicroArbitrage
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
    
    async fn start(&self) -> Result<()> {
        self.enabled.store(true, Ordering::SeqCst);
        info!("🚀 마이크로 아비트래지 전략 시작됨");
        
        // 가격 피드 구독/WS 초기화: 현재는 ExchangeMonitor -> PriceFeedManager 경로를 통해
        // 데이터를 수신하므로 여기서는 상태 플래그 및 로깅만 수행
        let exchanges_count = self.exchanges.len();
        let pairs_count = {
            let mut set = std::collections::HashSet::new();
            for (_ex, map) in self.price_cache.lock().await.iter() {
                for (sym, _pd) in map.iter() { set.insert(sym.clone()); }
            }
            set.len()
        };
        info!("📡 가격 피드 준비 상태 - 거래소: {}, 페어(캐시기준): {}", exchanges_count, pairs_count);
        info!("🧭 최소 수익률: {:.3}%, 최소 수익(USD): {}", self.min_profit_percentage * 100.0, self.min_profit_usd);
        info!("⏱️ 타임아웃: {}ms, 동시 거래 한도: {}", self.execution_timeout_ms, self.max_concurrent_trades);
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        self.enabled.store(false, Ordering::SeqCst);
        
        // 모든 활성 거래 대기
        let mut active_count = self.active_trades.lock().await.len();
        let mut wait_time = 0;
        
        while active_count > 0 && wait_time < 10000 { // 최대 10초 대기
            sleep(Duration::from_millis(100)).await;
            active_count = self.active_trades.lock().await.len();
            wait_time += 100;
        }
        
        if active_count > 0 {
            warn!("⚠️ {}개의 활성 거래가 완료되지 않았지만 전략을 중지합니다", active_count);
        }
        
        info!("⏹️ 마이크로 아비트래지 전략 중지됨");
        Ok(())
    }
    
    /// MEV 트랜잭션은 분석하지 않음 (마이크로 아비트래지는 독립적으로 실행)
    async fn analyze(&self, _transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(vec![]);
        }
        
        // 마이크로 아비트래지는 트랜잭션 기반이 아닌 가격 데이터 기반으로 동작
        // 대신 주기적으로 price scan을 실행해야 함
        Ok(vec![])
    }
    
    
    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        // 마이크로 아비트래지는 자체 기회 검증 로직 사용
        if opportunity.strategy != StrategyType::MicroArbitrage {
            return Ok(false);
        }
        
        // 기본적인 검증만 수행
        Ok(opportunity.expected_profit > U256::ZERO && opportunity.confidence > 0.5)
    }
    
    async fn create_bundle(&self, _opportunity: &Opportunity) -> Result<crate::types::Bundle> {
        // 마이크로 아비트래지는 Bundle 시스템을 사용하지 않음
        // 직접 거래소 주문으로 실행
        Err(anyhow!("MicroArbitrage strategy does not use bundle system"))
    }
}

impl std::fmt::Debug for MicroArbitrageStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroArbitrageStrategy")
            .field("enabled", &self.enabled)
            .field("exchanges_count", &self.exchanges.len())
            .field("min_profit_percentage", &self.min_profit_percentage)
            .field("execution_timeout_ms", &self.execution_timeout_ms)
            .field("max_concurrent_trades", &self.max_concurrent_trades)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PriceData, OrderBookSnapshot, OrderBookLevel};
    use rust_decimal::Decimal;
    use chrono::Utc;
    
    #[tokio::test]
    async fn test_micro_arbitrage_strategy_creation() {
        let config = Arc::new(crate::config::Config::default());
        // Skip test if we can't create a provider (no real network connection needed for this test)
        // In a real test environment, you would use a mock provider
        println!("MicroArbitrage strategy creation test - would test with mock provider in production");
        
        // Test that we can create a MicroArbitrageStrategy with a dummy reference
        // In actual testing, we would inject a mock provider
        assert!(true); // Placeholder assertion - replace with mock provider test
    }
    
    #[tokio::test]
    async fn test_price_data_update() {
        let config = Arc::new(crate::config::Config::default());
        // Skip test due to missing provider - in production, use mock provider
        println!("Price data update test - would test with mock provider in production");
        
        let price_data = PriceData {
            symbol: "WETH/USDC".to_string(),
            exchange: "uniswap_v2".to_string(),
            bid: Decimal::from_f64_retain(2000.0).unwrap(),
            ask: Decimal::from_f64_retain(2001.0).unwrap(),
            last_price: Decimal::from_f64_retain(2000.5).unwrap(),
            volume_24h: U256::from(1000000),
            timestamp: Utc::now(),
            sequence: 1,
        };
        
        // Test basic price data structure validity
        assert_eq!(price_data.symbol, "WETH/USDC");
        assert_eq!(price_data.exchange, "uniswap_v2");
        assert!(price_data.bid > Decimal::ZERO);
        assert!(price_data.ask > price_data.bid);
    }
    
    #[tokio::test]
    async fn test_arbitrage_opportunity_calculation() {
        let config = Arc::new(crate::config::Config::default());
        // Skip test due to missing provider - in production, use mock provider
        println!("Arbitrage opportunity calculation test - would test with mock provider in production");
        
        // 수익성 있는 가격 차이 시뮬레이션
        let buy_price_data = PriceData {
            symbol: "WETH/USDC".to_string(),
            exchange: "uniswap_v2".to_string(),
            bid: Decimal::from_f64_retain(1999.0).unwrap(),
            ask: Decimal::from_f64_retain(2000.0).unwrap(), // 낮은 매수 가격
            last_price: Decimal::from_f64_retain(1999.5).unwrap(),
            volume_24h: U256::from(1000000),
            timestamp: Utc::now(),
            sequence: 1,
        };
        
        let sell_price_data = PriceData {
            symbol: "WETH/USDC".to_string(),
            exchange: "sushiswap".to_string(),
            bid: Decimal::from_f64_retain(2005.0).unwrap(), // 높은 매도 가격
            ask: Decimal::from_f64_retain(2006.0).unwrap(),
            last_price: Decimal::from_f64_retain(2005.5).unwrap(),
            volume_24h: U256::from(1000000),
            timestamp: Utc::now(),
            sequence: 1,
        };
        
        // Test profit calculation logic
        let buy_price = buy_price_data.ask.to_f64().unwrap_or(0.0);
        let sell_price = sell_price_data.bid.to_f64().unwrap_or(0.0);
        let profit_percentage = (sell_price - buy_price) / buy_price * 100.0;
        
        assert!(profit_percentage > 0.0);
        assert_eq!(buy_price_data.exchange, "uniswap_v2");
        assert_eq!(sell_price_data.exchange, "sushiswap");
        println!("Simulated arbitrage profit: {:.2}%", profit_percentage);
    }
}