use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, anyhow};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use tokio::time::{sleep, Duration, Instant, timeout};
use rust_decimal::Decimal;
use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use std::str::FromStr;

use crate::config::{Config, ExchangeConfig, ExchangeType};
use crate::types::{
    MicroArbitrageOpportunity, OrderExecutionResult, OrderSide, OrderStatus,
    ExchangeInfo, PriceData, ArbitrageError,
};
use alloy::primitives::U256;

/// 주문 요청 정보
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub order_type: OrderType,
    pub quantity: U256,
    pub price: Decimal,
    pub timeout_ms: u64,
}

/// 주문 응답 정보
#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: OrderStatus,
    pub executed_price: Decimal,
    pub executed_quantity: U256,
    pub timestamp: chrono::DateTime<Utc>,
    pub transaction_hash: Option<String>,
    pub gas_used: Option<u64>,
    pub gas_price: Option<u64>,
}

/// 주문 타입
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderType {
    Buy,
    Sell,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Buy => write!(f, "Buy"),
            OrderType::Sell => write!(f, "Sell"),
        }
    }
}

/// 주문 실행 통계
#[derive(Debug, Clone)]
pub struct OrderExecutorStats {
    pub total_orders: u64,
    pub successful_orders: u64,
    pub failed_orders: u64,
    pub success_rate: f64,
    pub average_execution_time_ms: f64,
    pub total_volume: U256,
    pub uptime_percentage: f64,
}

/// 초고속 주문 실행 시스템
/// 
/// 마이크로아비트래지 기회가 발생했을 때 
/// 여러 거래소에서 동시에 주문을 실행하여
/// 최소 지연시간으로 수익을 실현합니다.
#[derive(Debug)]
pub struct OrderExecutor {
    config: Arc<Config>,
    is_running: Arc<AtomicBool>,
    
    // 거래소별 연결 정보
    exchange_clients: HashMap<String, Arc<dyn ExchangeClient>>,
    // 거래소 설정 조회용 메타
    exchange_config_by_name: HashMap<String, ExchangeConfig>,
    
    // 동시 실행 제한
    execution_semaphore: Arc<Semaphore>,
    
    // 주문 추적
    active_orders: Arc<Mutex<HashMap<String, OrderExecutionContext>>>,
    order_history: Arc<Mutex<Vec<OrderExecutionResult>>>,
    // 런타임 블랙리스트 (동적) — TTL 지원 위해 삽입 시각 저장
    runtime_blacklist: Arc<Mutex<HashMap<String, Instant>>>,
    
    // 성능 통계
    stats: Arc<Mutex<ExecutionStats>>,
    
    // 실행 매개변수
    execution_timeout_ms: u64,
    max_retry_attempts: u32,
    latency_threshold_ms: u64,
}

/// 주문 실행 컨텍스트
#[derive(Debug, Clone)]
struct OrderExecutionContext {
    order_id: String,
    opportunity: MicroArbitrageOpportunity,
    buy_order_id: Option<String>,
    sell_order_id: Option<String>,
    execution_start: Instant,
    status: ExecutionStatus,
    retry_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
enum ExecutionStatus {
    Pending,
    BuyOrderPlaced,
    SellOrderPlaced,
    BothOrdersPlaced,
    PartiallyFilled,
    Completed,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq)]
enum RiskLevel {
    Low,      // 소액 - 계속 진행
    High,     // 중간 - 해당 페어만 중단
    Critical, // 고액 - 시스템 중단
}

/// 실행 통계
#[derive(Debug, Clone)]
struct ExecutionStats {
    total_executions: u64,
    successful_executions: u64,
    failed_executions: u64,
    timed_out_executions: u64,
    partial_executions: u64,  // 부분 체결 카운트 추가
    total_volume: U256,
    total_profit: U256,
    total_fees: U256,
    avg_execution_time_ms: f64,
    avg_latency_ms: f64,
    success_rate: f64,
    profit_rate: f64,
    // 거래소별 성능
    exchange_success_rates: HashMap<String, f64>,
    exchange_avg_latencies: HashMap<String, f64>,
}

/// 거래소 클라이언트 트레이트
#[async_trait::async_trait]
pub trait ExchangeClient: Send + Sync + std::fmt::Debug {
    /// 통합 주문 실행
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse>;
    
    /// 토큰 잔고 조회
    async fn get_balance(&self, token: &str) -> Result<Decimal>;
    
    /// 현재 가격 조회
    async fn get_current_price(&self, symbol: &str) -> Result<crate::types::PriceData>;
    
    /// 매수 주문 (호환성을 위해 유지)
    async fn place_buy_order(
        &self,
        symbol: &str,
        amount: U256,
        price: Decimal,
    ) -> Result<String> {
        let order = OrderRequest {
            symbol: symbol.to_string(),
            order_type: OrderType::Buy,
            quantity: amount,
            price,
            timeout_ms: 5000,
        };
        let response = self.place_order(order).await?;
        Ok(response.order_id)
    }
    
    /// 매도 주문 (호환성을 위해 유지)
    async fn place_sell_order(
        &self,
        symbol: &str,
        amount: U256,
        price: Decimal,
    ) -> Result<String> {
        let order = OrderRequest {
            symbol: symbol.to_string(),
            order_type: OrderType::Sell,
            quantity: amount,
            price,
            timeout_ms: 5000,
        };
        let response = self.place_order(order).await?;
        Ok(response.order_id)
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<bool>;
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;
    
    async fn get_order_fills(&self, order_id: &str) -> Result<Vec<OrderFill>>;
    
    fn get_exchange_name(&self) -> &str;
    fn get_average_latency(&self) -> u64;
    fn is_connected(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct OrderFill {
    pub fill_id: String,
    pub order_id: String,
    pub filled_amount: U256,
    pub filled_price: Decimal,
    pub fee: U256,
    pub timestamp: chrono::DateTime<Utc>,
}

impl OrderExecutor {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        info!("🚀 주문 실행 시스템 초기화 중...");
        
        let max_concurrent = config.strategies.micro_arbitrage.max_concurrent_trades;
        let execution_timeout = config.strategies.micro_arbitrage.execution_timeout_ms;
        let latency_threshold = config.strategies.micro_arbitrage.latency_threshold_ms;
        
        // 거래소 클라이언트 초기화
        let mut exchange_clients = HashMap::new();
        let mut exchange_config_by_name = HashMap::new();
        
        for exchange_config in &config.strategies.micro_arbitrage.exchanges {
            if exchange_config.enabled {
                let client = Self::create_exchange_client(exchange_config).await?;
                exchange_clients.insert(exchange_config.name.clone(), client);
                exchange_config_by_name.insert(exchange_config.name.clone(), exchange_config.clone());
            }
        }
        
        info!("✅ 주문 실행 시스템 초기화 완료");
        info!("  🏪 연결된 거래소: {}개", exchange_clients.len());
        info!("  🔀 최대 동시 실행: {}개", max_concurrent);
        info!("  ⏱️ 실행 타임아웃: {}ms", execution_timeout);
        info!("  📡 지연 임계값: {}ms", latency_threshold);
        
        Ok(Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            exchange_clients,
            exchange_config_by_name,
            execution_semaphore: Arc::new(Semaphore::new(max_concurrent)),
            active_orders: Arc::new(Mutex::new(HashMap::new())),
            order_history: Arc::new(Mutex::new(Vec::new())),
            runtime_blacklist: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(ExecutionStats {
                total_executions: 0,
                successful_executions: 0,
                failed_executions: 0,
                timed_out_executions: 0,
                partial_executions: 0,
                total_volume: U256::ZERO,
                total_profit: U256::ZERO,
                total_fees: U256::ZERO,
                avg_execution_time_ms: 0.0,
                avg_latency_ms: 0.0,
                success_rate: 0.0,
                profit_rate: 0.0,
                exchange_success_rates: HashMap::new(),
                exchange_avg_latencies: HashMap::new(),
            })),
            execution_timeout_ms: execution_timeout,
            max_retry_attempts: 3,
            latency_threshold_ms: latency_threshold,
        })
    }
    
    /// 거래소 클라이언트 생성
    async fn create_exchange_client(
        exchange_config: &ExchangeConfig,
    ) -> Result<Arc<dyn ExchangeClient>> {
        match exchange_config.exchange_type {
            ExchangeType::DEX => {
                info!("🌐 DEX 클라이언트 생성: {}", exchange_config.name);
                Ok(Arc::new(DexClient::new(exchange_config.clone()).await?))
            }
            ExchangeType::CEX => {
                info!("🏛️ CEX 클라이언트 생성: {}", exchange_config.name);
                Ok(Arc::new(CexClient::new(exchange_config.clone()).await?))
            }
        }
    }
    
    /// 주문 실행 시스템 시작
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(anyhow!("OrderExecutor is already running"));
        }
        
        info!("🚀 주문 실행 시스템 시작");
        self.is_running.store(true, Ordering::SeqCst);
        
        // 주문 상태 모니터링 태스크 시작
        self.start_order_monitoring().await;
        
        // 통계 업데이트 태스크 시작
        self.start_stats_updater().await;
        
        // 주문 정리 태스크 시작
        self.start_order_cleanup().await;
        
        // 런타임 블랙리스트 만료 청소 태스크 시작
        self.start_blacklist_cleaner().await;
        
        info!("✅ 주문 실행 시스템 시작 완료");
        Ok(())
    }
    
    /// 주문 실행 시스템 중지
    pub async fn stop(&self) -> Result<()> {
        info!("⏹️ 주문 실행 시스템 중지 중...");
        self.is_running.store(false, Ordering::SeqCst);
        
        // 모든 활성 주문 취소
        self.cancel_all_active_orders().await?;
        
        // 모든 실행 완료 대기 (최대 10초)
        let mut wait_time = 0;
        while wait_time < 10000 {
            let active_count = self.active_orders.lock().await.len();
            if active_count == 0 {
                break;
            }
            
            sleep(Duration::from_millis(100)).await;
            wait_time += 100;
        }
        
        info!("⏹️ 주문 실행 시스템 중지됨");
        Ok(())
    }
    
    /// 마이크로 아비트래지 주문 실행
    pub async fn execute_arbitrage(&self, opportunity: MicroArbitrageOpportunity) -> Result<bool> {
        let execution_id = Uuid::new_v4().to_string();
        
        // 동시 실행 제한 적용
        let _permit = self.execution_semaphore.acquire().await?;
        
        let execution_start = Instant::now();
        
        info!("⚡ 아비트래지 실행 시작: {} ({} → {})", 
              execution_id, opportunity.buy_exchange, opportunity.sell_exchange);
        
        // 실행 컨텍스트 생성
        let context = OrderExecutionContext {
            order_id: execution_id.clone(),
            opportunity: opportunity.clone(),
            buy_order_id: None,
            sell_order_id: None,
            execution_start,
            status: ExecutionStatus::Pending,
            retry_count: 0,
        };
        
        // 활성 주문에 추가
        self.active_orders.lock().await.insert(execution_id.clone(), context);
        
        // 타임아웃 적용하여 실행
        let result = timeout(
            Duration::from_millis(self.execution_timeout_ms),
            self.execute_arbitrage_internal(execution_id.clone(), opportunity)
        ).await;
        
        let execution_time = execution_start.elapsed();
        
        match result {
            Ok(Ok(success)) => {
                if success {
                    info!("✅ 아비트래지 실행 성공: {} ({:.2}ms)", 
                          execution_id, execution_time.as_millis());
                    self.update_execution_stats(true, execution_time.as_millis() as f64).await;
                } else {
                    warn!("❌ 아비트래지 실행 실패: {}", execution_id);
                    self.update_execution_stats(false, execution_time.as_millis() as f64).await;
                }
                Ok(success)
            }
            Ok(Err(e)) => {
                error!("💥 아비트래지 실행 오류: {} - {}", execution_id, e);
                self.update_execution_stats(false, execution_time.as_millis() as f64).await;
                Err(e)
            }
            Err(_) => {
                warn!("⏰ 아비트래지 실행 타임아웃: {}", execution_id);
                
                // 타임아웃된 주문들 취소
                self.cancel_execution_orders(&execution_id).await?;
                
                self.update_timeout_stats(execution_time.as_millis() as f64).await;
                Ok(false)
            }
        }
    }
    
    /// 내부 아비트래지 실행 로직
    async fn execute_arbitrage_internal(
        &self,
        execution_id: String,
        opportunity: MicroArbitrageOpportunity,
    ) -> Result<bool> {
        // 매수/매도 거래소 클라이언트 획득
        let buy_client = self.exchange_clients.get(&opportunity.buy_exchange)
            .ok_or_else(|| anyhow!("Buy exchange client not found: {}", opportunity.buy_exchange))?;
        
        let sell_client = self.exchange_clients.get(&opportunity.sell_exchange)
            .ok_or_else(|| anyhow!("Sell exchange client not found: {}", opportunity.sell_exchange))?;
        
        // 거래소 연결 상태 확인
        if !buy_client.is_connected() || !sell_client.is_connected() {
            return Err(anyhow!("One or more exchanges are disconnected"));
        }
        
        // 지연시간 확인
        let buy_latency = buy_client.get_average_latency();
        let sell_latency = sell_client.get_average_latency();
        
        if buy_latency > self.latency_threshold_ms || sell_latency > self.latency_threshold_ms {
            return Err(anyhow!("Exchange latency too high: buy={}ms, sell={}ms", buy_latency, sell_latency));
        }
        
        // Mock 모드 처리
        if crate::mocks::is_mock_mode() {
            return self.execute_mock_arbitrage(&execution_id, &opportunity).await;
        }
        
        // 동시 주문 실행
        let (buy_result, sell_result) = tokio::join!(
            buy_client.place_buy_order(&opportunity.token_symbol, opportunity.max_amount, opportunity.buy_price),
            sell_client.place_sell_order(&opportunity.token_symbol, opportunity.max_amount, opportunity.sell_price)
        );
        
        // 주문 결과 처리
        match (buy_result, sell_result) {
            (Ok(buy_order_id), Ok(sell_order_id)) => {
                // 양쪽 주문 모두 성공
                self.update_execution_context(&execution_id, Some(buy_order_id.clone()), Some(sell_order_id.clone()), ExecutionStatus::BothOrdersPlaced).await;
                
                // 주문 체결 모니터링
                let filled = self.monitor_order_fills(&execution_id, &buy_order_id, &sell_order_id, buy_client.clone(), sell_client.clone()).await?;
                
                if filled {
                    self.update_execution_context(&execution_id, None, None, ExecutionStatus::Completed).await;
                    Ok(true)
                } else {
                    self.update_execution_context(&execution_id, None, None, ExecutionStatus::Failed).await;
                    Ok(false)
                }
            }
            (Ok(buy_order_id), Err(sell_err)) => {
                // 🚨 부분 체결: 매수만 성공
                error!("⚠️ 부분 체결 발생: 매수만 성공 - Order ID: {}, Exchange: {}, Amount: {}", 
                    buy_order_id, opportunity.buy_exchange, opportunity.max_amount);
                
                // 실행 컨텍스트 업데이트
                self.update_execution_context(&execution_id, Some(buy_order_id.clone()), None, ExecutionStatus::Failed).await;
                
                // 부분 체결 처리
                self.handle_partial_execution(
                    &execution_id,
                    Some((buy_order_id.clone(), buy_client.clone(), &opportunity.buy_exchange, opportunity.max_amount)),
                    None,
                    &opportunity
                ).await;
                
                // 통계 업데이트
                self.update_partial_execution_stats().await;
                
                Err(anyhow!("Partial execution: Buy succeeded, Sell failed - {}", sell_err))
            }
            (Err(buy_err), Ok(sell_order_id)) => {
                // 🚨 부분 체결: 매도만 성공
                error!("⚠️ 부분 체결 발생: 매도만 성공 - Order ID: {}, Exchange: {}, Amount: {}", 
                    sell_order_id, opportunity.sell_exchange, opportunity.max_amount);
                
                // 실행 컨텍스트 업데이트
                self.update_execution_context(&execution_id, None, Some(sell_order_id.clone()), ExecutionStatus::Failed).await;
                
                // 부분 체결 처리
                self.handle_partial_execution(
                    &execution_id,
                    None,
                    Some((sell_order_id.clone(), sell_client.clone(), &opportunity.sell_exchange, opportunity.max_amount)),
                    &opportunity
                ).await;
                
                // 통계 업데이트
                self.update_partial_execution_stats().await;
                
                Err(anyhow!("Partial execution: Sell succeeded, Buy failed - {}", buy_err))
            }
            (Err(buy_err), Err(sell_err)) => {
                // 양쪽 주문 모두 실패 - 안전한 상황
                warn!("Both orders failed - No position risk. Buy: {}, Sell: {}", buy_err, sell_err);
                self.update_execution_context(&execution_id, None, None, ExecutionStatus::Failed).await;
                Ok(false)
            }
        }
    }
    
    /// Mock 아비트래지 실행
    async fn execute_mock_arbitrage(
        &self,
        execution_id: &str,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<bool> {
        // 실제 거래소 지연시간 시뮬레이션
        let buy_delay = fastrand::u64(5..20); // 5-20ms
        let sell_delay = fastrand::u64(5..20); // 5-20ms
        
        sleep(Duration::from_millis(std::cmp::max(buy_delay, sell_delay))).await;
        
        // 90% 성공률 시뮬레이션
        let success = fastrand::f64() > 0.1;
        
        if success {
            debug!("🎭 Mock 아비트래지 성공: {} ({}→{})", 
                   execution_id, opportunity.buy_exchange, opportunity.sell_exchange);
                   
            // Mock 주문 체결 결과 생성
            let buy_order_result = OrderExecutionResult {
                order_id: format!("{}_buy", execution_id),
                exchange: opportunity.buy_exchange.clone(),
                symbol: opportunity.token_symbol.clone(),
                side: OrderSide::Buy,
                amount: opportunity.max_amount,
                price: opportunity.buy_price,
                filled_amount: opportunity.max_amount,
                filled_price: opportunity.buy_price,
                status: OrderStatus::Filled,
                execution_time: Utc::now(),
                latency_ms: buy_delay,
                fees: opportunity.max_amount / U256::from(1000), // 0.1% 수수료
                strategy_key: Some("micro_arbitrage".to_string()),
            };
            
            let sell_order_result = OrderExecutionResult {
                order_id: format!("{}_sell", execution_id),
                exchange: opportunity.sell_exchange.clone(),
                symbol: opportunity.token_symbol.clone(),
                side: OrderSide::Sell,
                amount: opportunity.max_amount,
                price: opportunity.sell_price,
                filled_amount: opportunity.max_amount,
                filled_price: opportunity.sell_price,
                status: OrderStatus::Filled,
                execution_time: Utc::now(),
                latency_ms: sell_delay,
                fees: opportunity.max_amount / U256::from(1000), // 0.1% 수수료
                strategy_key: Some("micro_arbitrage".to_string()),
            };
            
            // 주문 이력에 추가
            let mut history = self.order_history.lock().await;
            history.push(buy_order_result);
            history.push(sell_order_result);
            
            self.update_execution_context(execution_id, None, None, ExecutionStatus::Completed).await;
        } else {
            debug!("🎭 Mock 아비트래지 실패: {} (시장 상황 변화)", execution_id);
            self.update_execution_context(execution_id, None, None, ExecutionStatus::Failed).await;
        }
        
        Ok(success)
    }
    
    /// 주문 체결 모니터링
    async fn monitor_order_fills(
        &self,
        _execution_id: &str,
        _buy_order_id: &str,
        _sell_order_id: &str,
        _buy_client: Arc<dyn ExchangeClient>,
        _sell_client: Arc<dyn ExchangeClient>,
    ) -> Result<bool> {
        // 기본 모니터링 루프: 일정 주기로 상태 확인하여 완료 여부 판정
        let timeout = Duration::from_secs(30);
        let start_time = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            // 타임아웃 처리
            if start_time.elapsed() >= timeout {
                return Ok(false);
            }

            // 상태 조회 (에러는 실패로 간주하고 계속 재시도)
            let buy_status = _buy_client.get_order_status(_buy_order_id).await.unwrap_or(OrderStatus::Pending);
            let sell_status = _sell_client.get_order_status(_sell_order_id).await.unwrap_or(OrderStatus::Pending);

            // 둘 다 체결되면 성공
            if matches!(buy_status, OrderStatus::Filled) && matches!(sell_status, OrderStatus::Filled) {
                return Ok(true);
            }

            // 한쪽만 체결되고 다른 쪽이 취소/실패면 부분 체결로 간주
            let buy_terminal = matches!(buy_status, OrderStatus::Cancelled | OrderStatus::Filled | OrderStatus::PartiallyFilled);
            let sell_terminal = matches!(sell_status, OrderStatus::Cancelled | OrderStatus::Filled | OrderStatus::PartiallyFilled);
            if (matches!(buy_status, OrderStatus::Filled) && sell_terminal)
                || (matches!(sell_status, OrderStatus::Filled) && buy_terminal)
            {
                // 부분 체결 처리: 통계만 업데이트 (세부 회수 로직은 상위 핸들러)
                self.update_partial_execution_stats().await;
                return Ok(false);
            }
        }
    }
    
    /// 실행 컨텍스트 업데이트
    async fn update_execution_context(
        &self,
        execution_id: &str,
        buy_order_id: Option<String>,
        sell_order_id: Option<String>,
        status: ExecutionStatus,
    ) {
        let mut active_orders = self.active_orders.lock().await;
        if let Some(context) = active_orders.get_mut(execution_id) {
            if let Some(buy_id) = buy_order_id {
                context.buy_order_id = Some(buy_id);
            }
            if let Some(sell_id) = sell_order_id {
                context.sell_order_id = Some(sell_id);
            }
            context.status = status;
        }
    }
    
    /// 특정 실행의 주문들 취소
    async fn cancel_execution_orders(&self, execution_id: &str) -> Result<()> {
        let active_orders = self.active_orders.lock().await;
        
        if let Some(context) = active_orders.get(execution_id) {
            let mut cancel_tasks = Vec::new();
            
            // 매수 주문 취소
            if let Some(buy_order_id) = &context.buy_order_id {
                if let Some(buy_client) = self.exchange_clients.get(&context.opportunity.buy_exchange) {
                    let client = buy_client.clone();
                    let order_id = buy_order_id.clone();
                    cancel_tasks.push(tokio::spawn(async move {
                        client.cancel_order(&order_id).await
                    }));
                }
            }
            
            // 매도 주문 취소
            if let Some(sell_order_id) = &context.sell_order_id {
                if let Some(sell_client) = self.exchange_clients.get(&context.opportunity.sell_exchange) {
                    let client = sell_client.clone();
                    let order_id = sell_order_id.clone();
                    cancel_tasks.push(tokio::spawn(async move {
                        client.cancel_order(&order_id).await
                    }));
                }
            }
            
            // 모든 취소 작업 완료 대기
            for task in cancel_tasks {
                let _ = task.await;
            }
        }
        
        Ok(())
    }
    
    /// 모든 활성 주문 취소
    async fn cancel_all_active_orders(&self) -> Result<()> {
        let active_orders = self.active_orders.lock().await;
        let execution_ids: Vec<String> = active_orders.keys().cloned().collect();
        drop(active_orders);
        
        for execution_id in execution_ids {
            if let Err(e) = self.cancel_execution_orders(&execution_id).await {
                error!("주문 취소 실패: {} - {}", execution_id, e);
            }
        }
        
        Ok(())
    }
    
    /// 주문 모니터링 시작
    async fn start_order_monitoring(&self) {
        let is_running = Arc::clone(&self.is_running);
        let active_orders = Arc::clone(&self.active_orders);
        
        tokio::spawn(async move {
            let mut monitor_interval = tokio::time::interval(Duration::from_millis(100)); // 100ms마다 체크
            
            while is_running.load(Ordering::SeqCst) {
                monitor_interval.tick().await;
                
                let mut orders = active_orders.lock().await;
                let mut completed_orders = Vec::new();
                
                for (execution_id, context) in orders.iter_mut() {
                    // 타임아웃 체크
                    if context.execution_start.elapsed().as_millis() > 30000 { // 30초 타임아웃
                        context.status = ExecutionStatus::TimedOut;
                        completed_orders.push(execution_id.clone());
                    }
                    
                    // 완료된 주문 체크
                    if matches!(context.status, ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::TimedOut) {
                        if !completed_orders.contains(execution_id) {
                            completed_orders.push(execution_id.clone());
                        }
                    }
                }
                
                // 완료된 주문 제거
                for execution_id in completed_orders {
                    orders.remove(&execution_id);
                }
            }
        });
    }
    
    /// 통계 업데이트 시작
    async fn start_stats_updater(&self) {
        let is_running = Arc::clone(&self.is_running);
        let stats = Arc::clone(&self.stats);
        let order_history = Arc::clone(&self.order_history);
        
        tokio::spawn(async move {
            let mut update_interval = tokio::time::interval(Duration::from_secs(10)); // 10초마다 업데이트
            
            while is_running.load(Ordering::SeqCst) {
                update_interval.tick().await;
                
                let history = order_history.lock().await;
                let mut stats_guard = stats.lock().await;
                
                // 거래소별 성능 계산
                let mut exchange_stats: HashMap<String, (u64, u64, u64)> = HashMap::new(); // (success, total, total_latency)
                
                for order in history.iter() {
                    let entry = exchange_stats.entry(order.exchange.clone()).or_insert((0, 0, 0));
                    entry.1 += 1; // total
                    entry.2 += order.latency_ms; // total_latency
                    
                    if order.status == OrderStatus::Filled {
                        entry.0 += 1; // success
                    }
                }
                
                // 거래소별 성공률과 평균 지연시간 계산
                for (exchange, (success, total, total_latency)) in exchange_stats {
                    let success_rate = if total > 0 { success as f64 / total as f64 } else { 0.0 };
                    let avg_latency = if total > 0 { total_latency as f64 / total as f64 } else { 0.0 };
                    
                    stats_guard.exchange_success_rates.insert(exchange.clone(), success_rate);
                    stats_guard.exchange_avg_latencies.insert(exchange, avg_latency);
                }
                
                debug!("📊 주문 실행 통계 업데이트 - 총 실행: {}, 성공: {}, 실패: {}", 
                       stats_guard.total_executions, 
                       stats_guard.successful_executions, 
                       stats_guard.failed_executions);
            }
        });
    }
    
    /// 주문 정리 시작
    async fn start_order_cleanup(&self) {
        let is_running = Arc::clone(&self.is_running);
        let order_history = Arc::clone(&self.order_history);
        
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(Duration::from_secs(300)); // 5분마다 정리
            
            while is_running.load(Ordering::SeqCst) {
                cleanup_interval.tick().await;
                
                let mut history = order_history.lock().await;
                
                // 1시간 이상 된 주문 기록 제거 (최대 10000개 유지)
                let cutoff_time = Utc::now() - chrono::Duration::hours(1);
                history.retain(|order| order.execution_time > cutoff_time);
                
                if history.len() > 10000 {
                    let excess = history.len() - 10000;
                    history.drain(0..excess);
                }
                
                debug!("🧹 주문 이력 정리 완료 - 보관 중인 기록: {}개", history.len());
            }
        });
    }

    /// 런타임 블랙리스트 주기적 만료 정리
    async fn start_blacklist_cleaner(&self) {
        let is_running = Arc::clone(&self.is_running);
        let runtime_blacklist = Arc::clone(&self.runtime_blacklist);
        let ttl_secs = self.config.strategies.micro_arbitrage.runtime_blacklist_ttl_secs;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;
                let now = Instant::now();
                let mut guard = runtime_blacklist.lock().await;
                let before = guard.len();
                guard.retain(|symbol, inserted_at| {
                    let keep = now.duration_since(*inserted_at).as_secs() < ttl_secs;
                    if !keep {
                        debug!("🧯 블랙리스트 만료 해제: {}", symbol);
                    }
                    keep
                });
                let after = guard.len();
                if before != after {
                    info!("🧹 블랙리스트 정리: {} → {}", before, after);
                }
            }
        });
    }
    
    /// 실행 통계 업데이트
    async fn update_execution_stats(&self, success: bool, execution_time_ms: f64) {
        let mut stats = self.stats.lock().await;
        
        stats.total_executions += 1;
        
        if success {
            stats.successful_executions += 1;
        } else {
            stats.failed_executions += 1;
        }
        
        // 성공률 계산
        stats.success_rate = stats.successful_executions as f64 / stats.total_executions as f64;
        
        // 평균 실행 시간 업데이트
        stats.avg_execution_time_ms = (stats.avg_execution_time_ms * (stats.total_executions - 1) as f64 + execution_time_ms) / stats.total_executions as f64;
    }
    
    /// 타임아웃 통계 업데이트
    async fn update_timeout_stats(&self, execution_time_ms: f64) {
        let mut stats = self.stats.lock().await;
        
        stats.total_executions += 1;
        stats.timed_out_executions += 1;
        
        // 평균 실행 시간 업데이트
        stats.avg_execution_time_ms = (stats.avg_execution_time_ms * (stats.total_executions - 1) as f64 + execution_time_ms) / stats.total_executions as f64;
    }
    
    /// 활성 주문 수 조회
    pub async fn get_active_order_count(&self) -> usize {
        self.active_orders.lock().await.len()
    }
    
    /// 실행 통계 조회
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        self.stats.lock().await.clone()
    }
    
    /// 통계 조회 (오케스트레이터용)
    pub async fn get_stats(&self) -> OrderExecutorStats {
        let exec_stats = self.stats.lock().await;
        
        OrderExecutorStats {
            total_orders: exec_stats.total_executions,
            successful_orders: exec_stats.successful_executions,
            failed_orders: exec_stats.failed_executions,
            success_rate: exec_stats.success_rate,
            average_execution_time_ms: exec_stats.avg_execution_time_ms,
            total_volume: exec_stats.total_volume,
            uptime_percentage: if self.is_running.load(Ordering::SeqCst) { 100.0 } else { 0.0 },
        }
    }
    
    /// 주문 이력 조회
    pub async fn get_order_history(&self, limit: Option<usize>) -> Vec<OrderExecutionResult> {
        let history = self.order_history.lock().await;
        match limit {
            Some(n) => history.iter().rev().take(n).cloned().collect(),
            None => history.clone(),
        }
    }
    
    /// 부분 체결 통계 업데이트
    async fn update_partial_execution_stats(&self) {
        let mut stats = self.stats.lock().await;
        stats.partial_executions += 1;
        stats.total_executions += 1;
        stats.failed_executions += 1;  // 부분 체결도 실패로 간주
        
        // 성공률 재계산
        stats.success_rate = stats.successful_executions as f64 / stats.total_executions as f64;
        
        warn!("⚠️ 부분 체결 발생 - 총 {}건", stats.partial_executions);
    }
    
    /// 부분 체결 처리
    async fn handle_partial_execution(
        &self,
        execution_id: &str,
        buy_order: Option<(String, Arc<dyn ExchangeClient>, &str, U256)>,
        sell_order: Option<(String, Arc<dyn ExchangeClient>, &str, U256)>,
        opportunity: &MicroArbitrageOpportunity,
    ) {
        warn!("⚠️ 부분 체결 감지: {}", execution_id);
        
        // 1. 시스템 중단 대신 경고만 (시스템은 계속 실행)
        warn!("⚠️ 부분 체결 발생 - 포지션 불균형 주의");
        
        // 주문 존재 여부를 미리 저장
        let has_buy_order = buy_order.is_some();
        let has_sell_order = sell_order.is_some();
        
        // 2. 체결된 주문 취소 시도 (베스트 에포트)
        if let Some((order_id, client, exchange, amount)) = buy_order {
            warn!("📌 매수 주문 취소 시도: {} @ {}", order_id, exchange);
            match client.cancel_order(&order_id).await {
                Ok(_) => info!("✅ 매수 주문 취소 성공"),
                Err(e) => {
                    error!("❌ 매수 주문 취소 실패: {} - 수동 개입 필요", e);
                    error!("⚠️ 노출된 포지션: {} {} @ {}", amount, opportunity.token_symbol, exchange);
                }
            }
        }
        
        if let Some((order_id, client, exchange, amount)) = sell_order {
            warn!("📌 매도 주문 취소 시도: {} @ {}", order_id, exchange);
            match client.cancel_order(&order_id).await {
                Ok(_) => info!("✅ 매도 주문 취소 성공"),
                Err(e) => {
                    error!("❌ 매도 주문 취소 실패: {} - 수동 개입 필요", e);
                    error!("⚠️ 노출된 포지션: -{} {} @ {}", amount, opportunity.token_symbol, exchange);
                }
            }
        }
        
        // 3. 위험도 평가 및 조건부 대응
        let risk_level = self.evaluate_partial_execution_risk(opportunity).await;
        
        match risk_level {
            RiskLevel::Critical => {
                // 큰 금액이거나 위험한 토큰인 경우만 시스템 일시 중단
                error!("🚨 심각: 고위험 부분 체결 - 시스템 일시 중단");
                self.is_running.store(false, Ordering::SeqCst);
            },
            RiskLevel::High => {
                // 중간 위험 - 해당 토큰쌍만 거래 중단 (블랙리스트)
                warn!("⚠️ 경고: {} 거래쌍 일시 중단", opportunity.token_symbol);
                // 런타임 블랙리스트에 TTL과 함께 추가
                {
                    let mut bl = self.runtime_blacklist.lock().await;
                    bl.insert(opportunity.token_symbol.clone(), Instant::now());
                }
                info!("🛑 런타임 블랙리스트 추가됨: {} (TTL {}s)", opportunity.token_symbol, self.config.strategies.micro_arbitrage.runtime_blacklist_ttl_secs);
            },
            RiskLevel::Low => {
                // 낮은 위험 - 로깅만 하고 계속 진행
                info!("ℹ️ 부분 체결 기록 - 시스템 정상 운영");
            }
        }
        
        // 4. 부분 체결 이력 저장
        let result = OrderExecutionResult {
            order_id: execution_id.to_string(),
            exchange: if has_buy_order { 
                opportunity.buy_exchange.clone() 
            } else { 
                opportunity.sell_exchange.clone() 
            },
            symbol: opportunity.token_symbol.clone(),
            side: if has_buy_order { OrderSide::Buy } else { OrderSide::Sell },
            amount: opportunity.max_amount,
            price: if has_buy_order { opportunity.buy_price } else { opportunity.sell_price },
            filled_amount: U256::ZERO,  // 부분 체결이므로 정확한 체결량은 알 수 없음
            filled_price: Decimal::ZERO,
            status: OrderStatus::PartiallyFilled,
            execution_time: Utc::now(),
            latency_ms: 0,
            fees: U256::ZERO,
            strategy_key: Some("micro_arbitrage".to_string()),
        };
        
        self.order_history.lock().await.push(result);
        
        info!("✅ 부분 체결 처리 완료");
    }

    /// 런타임 블랙리스트 여부 확인
    async fn is_pair_blacklisted_runtime(&self, symbol: &str) -> bool {
        // TTL 기반 만료 처리
        {
            let ttl_secs = self.config.strategies.micro_arbitrage.runtime_blacklist_ttl_secs;
            let mut guard = self.runtime_blacklist.lock().await;
            if let Some(inserted_at) = guard.get(symbol).cloned() {
                let elapsed = inserted_at.elapsed().as_secs();
                if elapsed >= ttl_secs {
                    // TTL 초과 → 블랙리스트에서 제거
                    guard.remove(symbol);
                }
            }
            if guard.contains_key(symbol) {
                return true;
            }
        }
        // 정적 구성 블랙리스트도 참조
        let upper = symbol.to_uppercase();
        for t in &self.config.strategies.micro_arbitrage.blacklist_tokens {
            if upper.contains(&t.to_uppercase()) {
                return true;
            }
        }
        false
    }
    
    /// 부분 체결 위험도 평가
    async fn evaluate_partial_execution_risk(&self, opportunity: &MicroArbitrageOpportunity) -> RiskLevel {
        // USD 가치 계산 (예시: 1 ETH = $2000)
        let position_value_usd = opportunity.max_amount.to::<u64>() * 2000 / 10u64.pow(18);
        
        // 위험도 판단 기준
        if position_value_usd > 10000 {
            // $10,000 이상: 심각
            RiskLevel::Critical
        } else if position_value_usd > 1000 {
            // $1,000 - $10,000: 높음
            RiskLevel::High
        } else {
            // $1,000 미만: 낮음
            RiskLevel::Low
        }
    }
    
    /// 실행 중인지 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// 단일 주문 실행 (예측기반 전략용)
    pub async fn execute_order(&self, order: crate::types::Order) -> Result<String> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            let order_id = format!("order_{}_{}", order.side as u8, Uuid::new_v4().to_string()[..8].to_string());
            
            // Mock 실행 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(50 + fastrand::u64(50..150))).await;
            
            tracing::info!("✅ Mock 주문 실행 성공: {} {} {} @ {}", 
                order_id, 
                match order.side {
                    crate::types::OrderSide::Buy => "매수",
                    crate::types::OrderSide::Sell => "매도",
                },
                order.quantity,
                order.symbol
            );
            
            return Ok(order_id);
        }
        
        // 실제 주문 실행 골격
        // 심볼 매핑 규칙: "BASE/QUOTE" 또는 "BASEQUOTE" 모두 허용
        let symbol = order.symbol.to_uppercase();
        let unified_symbol = if symbol.contains('/') { symbol } else { format!("{}/USDC", symbol) };

        // 런타임/정적 블랙리스트 확인 (TTL 적용 포함)
        if self.is_pair_blacklisted_runtime(&unified_symbol).await {
            return Err(anyhow!("symbol blacklisted: {}", unified_symbol));
        }

        // 우선순위: priority_tokens 우선, 이후 거래소 선호도(DEX 우선 후 CEX 폴백)
        let mut last_err: Option<anyhow::Error> = None;
        let is_priority = self
            .config
            .strategies
            .micro_arbitrage
            .priority_tokens
            .iter()
            .any(|t| unified_symbol.contains(&t.to_uppercase()));

        // 거래소 순회 순서 구성 (DEX 우선, 우선순위 토큰이면 fast 지원 우선)
        let mut entries: Vec<(&String, &ExchangeConfig)> = self
            .exchange_clients
            .keys()
            .filter_map(|name| self.exchange_config_by_name.get(name).map(|cfg| (name, cfg)))
            .collect();
        entries.sort_by_key(|(_name, cfg)| {
            let dex_rank = match cfg.exchange_type {
                ExchangeType::DEX => 0,
                ExchangeType::CEX => 1,
            };
            let fast_rank = if is_priority {
                if cfg.supports_fast_execution { 0 } else { 1 }
            } else { 1 };
            (dex_rank, fast_rank)
        });

        for (name, _cfg) in entries {
            let client = match self.exchange_clients.get(name) { Some(c) => c, None => continue };
            // 연결 상태가 아니면 스킵
            if !client.is_connected() { continue; }

            // 현재가 조회로 기초 점검
            let price_check = client.get_current_price(&unified_symbol).await;
            let pd = match price_check {
                Ok(pd) => pd,
                Err(_) => { last_err = Some(anyhow!("price check failed")); continue; }
            };
            // 기본 가격 품질 체크
            if pd.bid <= Decimal::ZERO || pd.ask <= pd.bid {
                last_err = Some(anyhow!("price check failed"));
                continue;
            }

            // 주문 타입 변환
            let side = match order.side {
                crate::types::OrderSide::Buy => OrderType::Buy,
                crate::types::OrderSide::Sell => OrderType::Sell,
            };

            let quantity = U256::from((order.quantity.max(0.0)) as u128);
            let price = rust_decimal::Decimal::from_f64_retain(order.price.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);

            let request = OrderRequest {
                symbol: unified_symbol.clone(),
                order_type: side,
                quantity,
                price,
                timeout_ms: 5_000,
            };

            match client.place_order(request).await {
                Ok(resp) => {
                    info!("✅ 실거래 주문 제출: {} @ {} (status={:?})", unified_symbol, resp.executed_price, resp.status);
                    return Ok(resp.order_id);
                }
                Err(e) => {
                    warn!("❌ 주문 실패: {} - {}", unified_symbol, e);
                    last_err = Some(e);
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("No exchange accepted order")))
    }
}

/// DEX 클라이언트 구현
#[derive(Debug)]
struct DexClient {
    exchange_name: String,
    config: ExchangeConfig,
    average_latency: Arc<Mutex<u64>>,
    is_connected: Arc<AtomicBool>,
}

impl DexClient {
    async fn new(config: ExchangeConfig) -> Result<Self> {
        Ok(Self {
            exchange_name: config.name.clone(),
            config,
            average_latency: Arc::new(Mutex::new(20)), // 기본 20ms
            is_connected: Arc::new(AtomicBool::new(true)),
        })
    }
    
    async fn fetch_dexscreener_price(symbol: &str) -> Result<Decimal> {
        let parts: Vec<&str> = symbol.split('/').collect();
        if parts.len() != 2 { return Err(anyhow!("invalid pair")); }
        let base = parts[0].to_uppercase();
        let url = format!("https://api.dexscreener.com/latest/dex/search?q={}", base);
        let http = reqwest::Client::builder().timeout(Duration::from_millis(1500)).build()?;
        let resp = http.get(&url).send().await?;
        if !resp.status().is_success() { return Err(anyhow!("HTTP {}", resp.status())); }
        #[derive(Debug, Deserialize)]
        struct Pair { #[serde(rename = "priceUsd")] price_usd: Option<String> }
        #[derive(Debug, Deserialize)]
        struct Resp { pairs: Option<Vec<Pair>> }
        let body: Resp = resp.json().await?;
        if let Some(ps) = body.pairs {
            for p in ps {
                if let Some(s) = p.price_usd { if let Ok(d) = Decimal::from_str(&s) { return Ok(d); } }
            }
        }
        Err(anyhow!("price not found"))
    }
}

#[async_trait::async_trait]
impl ExchangeClient for DexClient {
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(10 + fastrand::u64(10..30))).await; // 10-40ms 지연
            return Ok(OrderResponse {
                order_id: format!("dex_{}_{}", self.exchange_name, Uuid::new_v4().to_string()[..8].to_string()),
                status: OrderStatus::Filled,
                executed_price: order.price,
                executed_quantity: order.quantity,
                timestamp: Utc::now(),
                transaction_hash: Some(format!("0x{:x}", fastrand::u64(..))),
                gas_used: Some(150000),
                gas_price: Some(20_000_000_000), // 20 gwei
            });
        }
        
        // TODO: 실제 DEX 주문 구현
        Err(anyhow!("Real DEX ordering not implemented"))
    }
    
    async fn get_balance(&self, token: &str) -> Result<Decimal> {
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(5 + fastrand::u64(5..15))).await;
            
            let balance = match token {
                "WETH" => Decimal::from(5),
                "USDC" | "USDT" | "DAI" => Decimal::from(25000),
                "WBTC" => Decimal::from_f64_retain(0.5).unwrap_or_default(),
                _ => Decimal::ZERO,
            };
            
            return Ok(balance);
        }
        
        // TODO: 실제 DEX 잔고 조회 구현
        Err(anyhow!("Real DEX balance check not implemented"))
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<PriceData> {
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(5 + fastrand::u64(5..15))).await;
            
            let base_price = match symbol {
                "WETH/USDC" => 2000.0,
                "WETH/USDT" => 2001.0, 
                "WETH/DAI" => 1999.0,
                "WBTC/USDC" => 45000.0,
                "WBTC/USDT" => 45050.0,
                _ => 100.0,
            };
            
            let adjusted_price = base_price * (1.0 + (fastrand::f64() - 0.5) * 0.02);
            let bid_price = adjusted_price * 0.9995;
            let ask_price = adjusted_price * 1.0005;
            
            return Ok(PriceData {
                symbol: symbol.to_string(),
                exchange: self.exchange_name.clone(),
                bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
                ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
                last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
                volume_24h: U256::from(fastrand::u64(100000..1000000)),
                timestamp: Utc::now(),
                sequence: fastrand::u64(..),
            });
        }
        // DexScreener 가격 사용 (USD 기준), synthetic bid/ask 구성
        let last = Self::fetch_dexscreener_price(symbol).await?;
        let spread = Decimal::from_str("0.001").unwrap_or(Decimal::ZERO); // 0.1%
        let one = Decimal::ONE;
        let bid = last * (one - spread/Decimal::from(2u8));
        let ask = last * (one + spread/Decimal::from(2u8));
        Ok(PriceData {
            symbol: symbol.to_string(),
            exchange: self.exchange_name.clone(),
            bid,
            ask,
            last_price: last,
            volume_24h: U256::from(0u64),
            timestamp: Utc::now(),
            sequence: fastrand::u64(..),
        })
    }
    
    async fn place_buy_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(10 + fastrand::u64(10..30))).await; // 10-40ms 지연
            return Ok(format!("dex_buy_{}_{}", self.exchange_name, Uuid::new_v4().to_string()[..8].to_string()));
        }
        
        // TODO: 실제 DEX 주문 구현
        Err(anyhow!("Real DEX ordering not implemented"))
    }
    
    async fn place_sell_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(10 + fastrand::u64(10..30))).await; // 10-40ms 지연
            return Ok(format!("dex_sell_{}_{}", self.exchange_name, Uuid::new_v4().to_string()[..8].to_string()));
        }
        
        // TODO: 실제 DEX 주문 구현
        Err(anyhow!("Real DEX ordering not implemented"))
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<bool> {
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(5 + fastrand::u64(5..15))).await; // 5-20ms 지연
            return Ok(true);
        }
        
        // TODO: 실제 DEX 주문 취소 구현
        Err(anyhow!("Real DEX order cancellation not implemented"))
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        if crate::mocks::is_mock_mode() {
            return Ok(OrderStatus::Filled); // Mock에서는 항상 체결됨
        }
        
        // TODO: 실제 DEX 주문 상태 조회 구현
        Err(anyhow!("Real DEX order status check not implemented"))
    }
    
    async fn get_order_fills(&self, order_id: &str) -> Result<Vec<OrderFill>> {
        if crate::mocks::is_mock_mode() {
            return Ok(vec![]); // Mock에서는 빈 배열
        }
        
        // TODO: 실제 DEX 주문 체결 내역 조회 구현
        Err(anyhow!("Real DEX order fills check not implemented"))
    }
    
    fn get_exchange_name(&self) -> &str {
        &self.exchange_name
    }
    
    fn get_average_latency(&self) -> u64 {
        if let Ok(latency) = self.average_latency.try_lock() {
            *latency
        } else {
            20 // 기본값
        }
    }
    
    fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }
}

/// CEX 클라이언트 구현
#[derive(Debug)]
struct CexClient {
    exchange_name: String,
    config: ExchangeConfig,
    average_latency: Arc<Mutex<u64>>,
    is_connected: Arc<AtomicBool>,
}

impl CexClient {
    async fn new(config: ExchangeConfig) -> Result<Self> {
        Ok(Self {
            exchange_name: config.name.clone(),
            config,
            average_latency: Arc::new(Mutex::new(10)), // CEX는 더 빠른 기본값
            is_connected: Arc::new(AtomicBool::new(true)),
        })
    }
    
    fn to_binance_symbol(pair: &str) -> Option<String> {
        let parts: Vec<&str> = pair.split('/').collect();
        if parts.len() != 2 { return None; }
        let base_input = parts[0].to_uppercase();
        let base = match base_input.as_str() {
            "WETH" => "ETH",
            "WBTC" => "BTC",
            other => other,
        };
        let quote = parts[1].to_uppercase();
        Some(format!("{}{}", base, quote))
    }
    
    async fn fetch_binance_book_ticker(symbol: &str) -> Result<(Decimal, Decimal, Decimal)> {
        #[derive(Debug, Deserialize)]
        struct Ticker { #[serde(rename="bidPrice")] bid_price: String, #[serde(rename="askPrice")] ask_price: String }
        let url = format!("https://api.binance.com/api/v3/ticker/bookTicker?symbol={}", symbol);
        let http = reqwest::Client::builder().timeout(Duration::from_millis(1500)).build()?;
        let resp = http.get(&url).send().await?;
        if !resp.status().is_success() { return Err(anyhow!("HTTP {}", resp.status())); }
        let data: Ticker = resp.json().await?;
        let bid = Decimal::from_str(&data.bid_price).unwrap_or(Decimal::ZERO);
        let ask = Decimal::from_str(&data.ask_price).unwrap_or(Decimal::ZERO);
        let last = if bid > Decimal::ZERO && ask > Decimal::ZERO { (bid + ask)/Decimal::from(2u8) } else { bid.max(ask) };
        Ok((bid, ask, last))
    }
}

#[async_trait::async_trait]
impl ExchangeClient for CexClient {
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(5 + fastrand::u64(5..15))).await; // 5-20ms 지연 (CEX는 더 빠름)
            return Ok(OrderResponse {
                order_id: format!("cex_{}_{}", self.exchange_name, Uuid::new_v4().to_string()[..8].to_string()),
                status: OrderStatus::Filled,
                executed_price: order.price,
                executed_quantity: order.quantity,
                timestamp: Utc::now(),
                transaction_hash: None, // CEX는 트랜잭션 해시 없음
                gas_used: None,
                gas_price: None,
            });
        }
        
        // TODO: 실제 CEX 주문 구현
        Err(anyhow!("Real CEX ordering not implemented"))
    }
    
    async fn get_balance(&self, token: &str) -> Result<Decimal> {
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(3 + fastrand::u64(3..10))).await;
            
            let balance = match token {
                "WETH" => Decimal::from(10),
                "USDC" | "USDT" | "DAI" => Decimal::from(50000),
                "WBTC" => Decimal::from(1),
                _ => Decimal::ZERO,
            };
            
            return Ok(balance);
        }
        
        // TODO: 실제 CEX 잔고 조회 구현
        Err(anyhow!("Real CEX balance check not implemented"))
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<PriceData> {
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(3 + fastrand::u64(3..10))).await;
            
            let base_price = match symbol {
                "WETH/USDC" => 2000.0,
                "WETH/USDT" => 2001.0,
                "WETH/DAI" => 1999.0,
                "WBTC/USDC" => 45000.0,
                "WBTC/USDT" => 45050.0,
                _ => 100.0,
            };
            
            let exchange_multiplier = match self.exchange_name.as_str() {
                "binance" => 1.001,
                "coinbase" => 0.999,
                _ => 1.0,
            };
            
            let adjusted_price = base_price * exchange_multiplier * (1.0 + (fastrand::f64() - 0.5) * 0.01);
            let bid_price = adjusted_price * 0.9999;
            let ask_price = adjusted_price * 1.0001;
            
            return Ok(PriceData {
                symbol: symbol.to_string(),
                exchange: self.exchange_name.clone(),
                bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
                ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
                last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
                volume_24h: U256::from(fastrand::u64(1000000..10000000)),
                timestamp: Utc::now(),
                sequence: fastrand::u64(..),
            });
        }
        // Binance 호환 엔드포인트로 현재가 조회
        let binance_symbol = Self::to_binance_symbol(symbol).ok_or_else(|| anyhow!("invalid symbol"))?;
        let (bid, ask, last) = Self::fetch_binance_book_ticker(&binance_symbol).await?;
        Ok(PriceData {
            symbol: symbol.to_string(),
            exchange: self.exchange_name.clone(),
            bid,
            ask,
            last_price: last,
            volume_24h: U256::from(0u64),
            timestamp: Utc::now(),
            sequence: fastrand::u64(..),
        })
    }
    
    async fn place_buy_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(5 + fastrand::u64(5..15))).await; // 5-20ms 지연 (CEX는 더 빠름)
            return Ok(format!("cex_buy_{}_{}", self.exchange_name, Uuid::new_v4().to_string()[..8].to_string()));
        }
        
        // TODO: 실제 CEX 주문 구현
        Err(anyhow!("Real CEX ordering not implemented"))
    }
    
    async fn place_sell_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        // Mock 모드에서는 시뮬레이션
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(5 + fastrand::u64(5..15))).await; // 5-20ms 지연
            return Ok(format!("cex_sell_{}_{}", self.exchange_name, Uuid::new_v4().to_string()[..8].to_string()));
        }
        
        // TODO: 실제 CEX 주문 구현
        Err(anyhow!("Real CEX ordering not implemented"))
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<bool> {
        if crate::mocks::is_mock_mode() {
            sleep(Duration::from_millis(2 + fastrand::u64(3..8))).await; // 2-10ms 지연
            return Ok(true);
        }
        
        // TODO: 실제 CEX 주문 취소 구현
        Err(anyhow!("Real CEX order cancellation not implemented"))
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        if crate::mocks::is_mock_mode() {
            return Ok(OrderStatus::Filled);
        }
        
        // TODO: 실제 CEX 주문 상태 조회 구현
        Err(anyhow!("Real CEX order status check not implemented"))
    }
    
    async fn get_order_fills(&self, order_id: &str) -> Result<Vec<OrderFill>> {
        if crate::mocks::is_mock_mode() {
            return Ok(vec![]);
        }
        
        // TODO: 실제 CEX 주문 체결 내역 조회 구현
        Err(anyhow!("Real CEX order fills check not implemented"))
    }
    
    fn get_exchange_name(&self) -> &str {
        &self.exchange_name
    }
    
    fn get_average_latency(&self) -> u64 {
        if let Ok(latency) = self.average_latency.try_lock() {
            *latency
        } else {
            10 // 기본값
        }
    }
    
    fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    
    #[tokio::test]
    async fn test_order_executor_creation() {
        let config = Arc::new(crate::config::Config::default());
        let executor = OrderExecutor::new(config).await;
        assert!(executor.is_ok());
        
        let executor = executor.unwrap();
        assert!(!executor.is_running());
        assert_eq!(executor.get_active_order_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_mock_arbitrage_execution() {
        let config = Arc::new(crate::config::Config::default());
        let executor = OrderExecutor::new(config).await.unwrap();
        
        let opportunity = MicroArbitrageOpportunity {
            token_symbol: "WETH/USDC".to_string(),
            buy_exchange: "uniswap_v2".to_string(),
            sell_exchange: "mock_binance".to_string(),
            buy_price: Decimal::from_f64_retain(2000.0).unwrap(),
            sell_price: Decimal::from_f64_retain(2005.0).unwrap(),
            price_spread: Decimal::from_f64_retain(5.0).unwrap(),
            profit_percentage: 0.0025, // 0.25%
            max_amount: U256::from(1000),
            execution_window_ms: 100,
            confidence_score: 0.8,
        };
        
        executor.start().await.unwrap();
        
        let result = executor.execute_arbitrage(opportunity).await;
        assert!(result.is_ok());
        
        executor.stop().await.unwrap();
    }
}