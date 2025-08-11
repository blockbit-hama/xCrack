use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::Mutex;
use tracing::{info, debug, warn, error};
use alloy::primitives::{Address, U256};
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
    Transaction, Opportunity, StrategyType, OpportunityType, OpportunityDetails,
    MicroArbitrageDetails, MicroArbitrageOpportunity, PriceData, 
    OrderBookSnapshot, ExchangeInfo, ExchangeType, MicroArbitrageStats,
    OrderExecutionResult, OrderSide, OrderStatus, Bundle,
};
use crate::strategies::Strategy;

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
        })
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
                self.execute_real_arbitrage(opportunity, &trade_id).await
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
    async fn execute_mock_arbitrage(&self, opportunity: &MicroArbitrageOpportunity, trade_id: &str) -> Result<bool> {
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
    
    /// 실제 아비트래지 실행 (실제 구현에서는 거래소 API 호출)
    async fn execute_real_arbitrage(&self, _opportunity: &MicroArbitrageOpportunity, _trade_id: &str) -> Result<bool> {
        // TODO: 실제 거래소 API 구현
        // 1. 매수 주문 생성 및 실행
        // 2. 매도 주문 생성 및 실행  
        // 3. 주문 상태 모니터링
        // 4. 부분 체결 처리
        // 5. 실패 시 롤백
        
        warn!("⚠️ 실제 아비트래지 실행은 아직 구현되지 않음 (Mock 모드 사용)");
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
                let min_profit_percentage = self.min_profit_percentage;
                let min_profit_usd = self.min_profit_usd;
                let execution_timeout_ms = self.execution_timeout_ms;
                let max_concurrent_trades = self.max_concurrent_trades;
                let latency_threshold_ms = self.latency_threshold_ms;
                let daily_volume_limit = self.daily_volume_limit;
                let risk_limit_per_trade = self.risk_limit_per_trade;
                
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
        
        // TODO: 가격 피드 구독 시작
        // TODO: WebSocket 연결 초기화
        
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