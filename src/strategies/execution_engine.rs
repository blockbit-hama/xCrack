use anyhow::Result;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
// tokio::sync imports removed as unused
use uuid::Uuid;

use crate::{
    exchange::{order_executor::OrderExecutor, price_feed_manager::PriceFeedManager},
    types::{Order, OrderSide, OrderType, TimeInForce},
};

/// xQuant 스타일 주문 실행 엔진
/// VWAP, TWAP, Iceberg, Trailing Stop 전략을 xCrack에 통합
#[derive(Debug)]
pub struct QuantExecutionEngine {
    /// 실행 엔진 ID
    id: Uuid,
    /// 주문 실행기
    order_executor: Arc<OrderExecutor>,
    /// 가격 피드 매니저
    price_feed: Arc<PriceFeedManager>,
    /// 활성 실행 작업들
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionTask>>>,
    /// 실행 설정
    config: ExecutionConfig,
}

/// 실행 작업
#[derive(Debug, Clone)]
pub struct ExecutionTask {
    /// 작업 ID
    pub id: Uuid,
    /// 심볼
    pub symbol: String,
    /// 총 주문 크기
    pub total_size: f64,
    /// 실행된 크기
    pub executed_size: f64,
    /// 실행 전략
    pub strategy: ExecutionStrategy,
    /// 생성 시간
    pub created_at: u64,
    /// 상태
    pub status: ExecutionStatus,
}

/// 실행 전략 유형
#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    /// VWAP (Volume Weighted Average Price) 전략
    Vwap {
        duration_minutes: u32,
        max_participation_rate: f64,
        side: OrderSide,
    },
    /// TWAP (Time Weighted Average Price) 전략
    Twap {
        duration_minutes: u32,
        slice_count: u32,
        side: OrderSide,
    },
    /// Iceberg 주문 전략
    Iceberg {
        visible_size: f64,
        side: OrderSide,
        price_limit: Option<f64>,
    },
    /// Trailing Stop 전략
    TrailingStop {
        trigger_price: f64,
        trail_amount: f64,
        side: OrderSide,
    },
}

/// 실행 상태
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    /// 대기 중
    Pending,
    /// 실행 중
    Running,
    /// 완료
    Completed,
    /// 취소됨
    Cancelled,
    /// 실패
    Failed(String),
}

/// 실행 엔진 설정
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// 최소 주문 크기
    pub min_order_size: f64,
    /// 최대 슬라이스 수
    pub max_slices: u32,
    /// 주문 간 최소 간격 (초)
    pub min_order_interval: u64,
    /// 시장가 주문 허용 여부
    pub allow_market_orders: bool,
}

/// VWAP 실행기
#[derive(Debug)]
pub struct VwapExecutor {
    price_feed: Arc<PriceFeedManager>,
    order_executor: Arc<OrderExecutor>,
}

/// TWAP 실행기
#[derive(Debug)]
pub struct TwapExecutor {
    order_executor: Arc<OrderExecutor>,
}

/// Iceberg 실행기
#[derive(Debug)]
pub struct IcebergExecutor {
    price_feed: Arc<PriceFeedManager>,
    order_executor: Arc<OrderExecutor>,
    active_orders: Arc<RwLock<HashMap<Uuid, Order>>>,
}

/// Trailing Stop 실행기
#[derive(Debug)]
pub struct TrailingStopExecutor {
    price_feed: Arc<PriceFeedManager>,
    order_executor: Arc<OrderExecutor>,
    trailing_levels: Arc<RwLock<HashMap<String, f64>>>,
}

impl QuantExecutionEngine {
    pub fn new(
        order_executor: Arc<OrderExecutor>,
        price_feed: Arc<PriceFeedManager>,
        config: ExecutionConfig,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            order_executor,
            price_feed,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// 실행 작업 시작
    pub async fn start_execution(&self, task: ExecutionTask) -> Result<Uuid> {
        let task_id = task.id;
        
        // 실행 작업 등록
        {
            let mut executions = self.active_executions.write().unwrap();
            executions.insert(task_id, task.clone());
        }

        // 전략별 실행 시작
        let task_clone = task.clone();
        match &task.strategy {
            ExecutionStrategy::Vwap { duration_minutes, max_participation_rate, side } => {
                let executor = VwapExecutor::new(
                    Arc::clone(&self.price_feed),
                    Arc::clone(&self.order_executor),
                );
                self.spawn_vwap_execution(executor, task_clone, *duration_minutes, *max_participation_rate, *side).await?;
            }
            ExecutionStrategy::Twap { duration_minutes, slice_count, side } => {
                let executor = TwapExecutor::new(Arc::clone(&self.order_executor));
                self.spawn_twap_execution(executor, task_clone, *duration_minutes, *slice_count, *side).await?;
            }
            ExecutionStrategy::Iceberg { visible_size, side, price_limit } => {
                let executor = IcebergExecutor::new(
                    Arc::clone(&self.price_feed),
                    Arc::clone(&self.order_executor),
                );
                self.spawn_iceberg_execution(executor, task_clone, *visible_size, *side, *price_limit).await?;
            }
            ExecutionStrategy::TrailingStop { trigger_price, trail_amount, side } => {
                let executor = TrailingStopExecutor::new(
                    Arc::clone(&self.price_feed),
                    Arc::clone(&self.order_executor),
                );
                self.spawn_trailing_stop_execution(executor, task_clone, *trigger_price, *trail_amount, *side).await?;
            }
        }

        Ok(task_id)
    }

    /// VWAP 실행 스폰
    async fn spawn_vwap_execution(
        &self,
        executor: VwapExecutor,
        mut task: ExecutionTask,
        duration_minutes: u32,
        max_participation_rate: f64,
        side: OrderSide,
    ) -> Result<()> {
        let executions = Arc::clone(&self.active_executions);
        let config = self.config.clone();

        tokio::spawn(async move {
            if let Err(e) = executor.execute_vwap(
                &mut task,
                duration_minutes,
                max_participation_rate,
                side,
                &config,
            ).await {
                tracing::error!("VWAP 실행 실패: {}", e);
                let mut executions_guard = executions.write().unwrap();
                if let Some(execution_task) = executions_guard.get_mut(&task.id) {
                    execution_task.status = ExecutionStatus::Failed(e.to_string());
                }
            }
        });

        Ok(())
    }

    /// TWAP 실행 스폰
    async fn spawn_twap_execution(
        &self,
        executor: TwapExecutor,
        mut task: ExecutionTask,
        duration_minutes: u32,
        slice_count: u32,
        side: OrderSide,
    ) -> Result<()> {
        let executions = Arc::clone(&self.active_executions);
        let config = self.config.clone();

        tokio::spawn(async move {
            if let Err(e) = executor.execute_twap(
                &mut task,
                duration_minutes,
                slice_count,
                side,
                &config,
            ).await {
                tracing::error!("TWAP 실행 실패: {}", e);
                let mut executions_guard = executions.write().unwrap();
                if let Some(execution_task) = executions_guard.get_mut(&task.id) {
                    execution_task.status = ExecutionStatus::Failed(e.to_string());
                }
            }
        });

        Ok(())
    }

    /// Iceberg 실행 스폰
    async fn spawn_iceberg_execution(
        &self,
        executor: IcebergExecutor,
        mut task: ExecutionTask,
        visible_size: f64,
        side: OrderSide,
        price_limit: Option<f64>,
    ) -> Result<()> {
        let executions = Arc::clone(&self.active_executions);
        let config = self.config.clone();

        tokio::spawn(async move {
            if let Err(e) = executor.execute_iceberg(
                &mut task,
                visible_size,
                side,
                price_limit,
                &config,
            ).await {
                tracing::error!("Iceberg 실행 실패: {}", e);
                let mut executions_guard = executions.write().unwrap();
                if let Some(execution_task) = executions_guard.get_mut(&task.id) {
                    execution_task.status = ExecutionStatus::Failed(e.to_string());
                }
            }
        });

        Ok(())
    }

    /// Trailing Stop 실행 스폰
    async fn spawn_trailing_stop_execution(
        &self,
        executor: TrailingStopExecutor,
        mut task: ExecutionTask,
        trigger_price: f64,
        trail_amount: f64,
        side: OrderSide,
    ) -> Result<()> {
        let executions = Arc::clone(&self.active_executions);
        let config = self.config.clone();

        tokio::spawn(async move {
            if let Err(e) = executor.execute_trailing_stop(
                &mut task,
                trigger_price,
                trail_amount,
                side,
                &config,
            ).await {
                tracing::error!("Trailing Stop 실행 실패: {}", e);
                let mut executions_guard = executions.write().unwrap();
                if let Some(execution_task) = executions_guard.get_mut(&task.id) {
                    execution_task.status = ExecutionStatus::Failed(e.to_string());
                }
            }
        });

        Ok(())
    }

    /// 실행 작업 상태 조회
    pub fn get_execution_status(&self, task_id: &Uuid) -> Option<ExecutionStatus> {
        let executions = self.active_executions.read().unwrap();
        executions.get(task_id).map(|task| task.status.clone())
    }

    /// 실행 작업 취소
    pub async fn cancel_execution(&self, task_id: &Uuid) -> Result<()> {
        let mut executions = self.active_executions.write().unwrap();
        if let Some(task) = executions.get_mut(task_id) {
            task.status = ExecutionStatus::Cancelled;
            tracing::info!("실행 작업 취소: {}", task_id);
        }
        Ok(())
    }
}

impl VwapExecutor {
    pub fn new(
        price_feed: Arc<PriceFeedManager>,
        order_executor: Arc<OrderExecutor>,
    ) -> Self {
        Self {
            price_feed,
            order_executor,
        }
    }

    pub async fn execute_vwap(
        &self,
        task: &mut ExecutionTask,
        duration_minutes: u32,
        max_participation_rate: f64,
        side: OrderSide,
        config: &ExecutionConfig,
    ) -> Result<()> {
        tracing::info!("VWAP 실행 시작: {} ({}분)", task.symbol, duration_minutes);
        
        task.status = ExecutionStatus::Running;
        let total_intervals = (duration_minutes / 5).max(1); // 5분 간격
        let mut remaining_size = task.total_size;

        for interval in 0..total_intervals {
            if task.status == ExecutionStatus::Cancelled {
                break;
            }

            // 현재 시장 거래량 조회
            let current_volume = self.price_feed.get_current_volume(&task.symbol).await
                .unwrap_or(1000.0);
            
            // 최대 참여율에 따른 주문 크기 계산
            let max_allowed_size = current_volume * max_participation_rate;
            let interval_size = (remaining_size / (total_intervals - interval) as f64)
                .min(max_allowed_size)
                .max(config.min_order_size);

            if interval_size < config.min_order_size {
                tracing::warn!("주문 크기가 최소값 미만: {}", interval_size);
                continue;
            }

            // 주문 생성 및 실행
            let order = Order {
                id: Uuid::new_v4(),
                symbol: task.symbol.clone(),
                side,
                order_type: if config.allow_market_orders { OrderType::Market } else { OrderType::Limit },
                quantity: interval_size,
                price: if config.allow_market_orders { 
                    None 
                } else { 
                    Some(self.price_feed.get_current_price(&task.symbol).await?) 
                },
                time_in_force: TimeInForce::IOC,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            };

            self.order_executor.execute_order(order).await?;
            
            task.executed_size += interval_size;
            remaining_size -= interval_size;

            tracing::debug!("VWAP 주문 실행: {}/{} ({}%)", 
                task.executed_size, 
                task.total_size,
                (task.executed_size / task.total_size * 100.0) as u32
            );

            // 다음 인터벌까지 대기
            if interval < total_intervals - 1 && remaining_size > config.min_order_size {
                tokio::time::sleep(Duration::from_secs(5 * 60)).await;
            }
        }

        task.status = ExecutionStatus::Completed;
        tracing::info!("VWAP 실행 완료: {} ({}/{})", 
            task.symbol, task.executed_size, task.total_size);
        
        Ok(())
    }
}

impl TwapExecutor {
    pub fn new(order_executor: Arc<OrderExecutor>) -> Self {
        Self { order_executor }
    }

    pub async fn execute_twap(
        &self,
        task: &mut ExecutionTask,
        duration_minutes: u32,
        slice_count: u32,
        side: OrderSide,
        config: &ExecutionConfig,
    ) -> Result<()> {
        tracing::info!("TWAP 실행 시작: {} ({}분, {}슬라이스)", 
            task.symbol, duration_minutes, slice_count);
        
        task.status = ExecutionStatus::Running;
        let slice_size = task.total_size / slice_count as f64;
        let interval = Duration::from_secs((duration_minutes * 60 / slice_count) as u64);

        for slice in 0..slice_count {
            if task.status == ExecutionStatus::Cancelled {
                break;
            }

            let order = Order {
                id: Uuid::new_v4(),
                symbol: task.symbol.clone(),
                side,
                order_type: if config.allow_market_orders { OrderType::Market } else { OrderType::Limit },
                quantity: slice_size,
                price: None, // 시장가 또는 현재가
                time_in_force: TimeInForce::IOC,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            };

            self.order_executor.execute_order(order).await?;
            task.executed_size += slice_size;

            tracing::debug!("TWAP 슬라이스 실행: {}/{} ({}%)", 
                slice + 1, slice_count,
                ((slice + 1) as f64 / slice_count as f64 * 100.0) as u32
            );

            // 마지막 슬라이스가 아니면 대기
            if slice < slice_count - 1 {
                tokio::time::sleep(interval).await;
            }
        }

        task.status = ExecutionStatus::Completed;
        tracing::info!("TWAP 실행 완료: {}", task.symbol);
        
        Ok(())
    }
}

impl IcebergExecutor {
    pub fn new(
        price_feed: Arc<PriceFeedManager>,
        order_executor: Arc<OrderExecutor>,
    ) -> Self {
        Self {
            price_feed,
            order_executor,
            active_orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn execute_iceberg(
        &self,
        task: &mut ExecutionTask,
        visible_size: f64,
        side: OrderSide,
        price_limit: Option<f64>,
        config: &ExecutionConfig,
    ) -> Result<()> {
        tracing::info!("Iceberg 실행 시작: {} (가시크기: {})", task.symbol, visible_size);
        
        task.status = ExecutionStatus::Running;
        let mut remaining_size = task.total_size;

        while remaining_size > config.min_order_size && task.status != ExecutionStatus::Cancelled {
            let order_size = remaining_size.min(visible_size);
            let price = match price_limit {
                Some(limit) => Some(limit),
                None => Some(self.price_feed.get_current_price(&task.symbol).await?),
            };

            let order = Order {
                id: Uuid::new_v4(),
                symbol: task.symbol.clone(),
                side,
                order_type: OrderType::Limit,
                quantity: order_size,
                price,
                time_in_force: TimeInForce::GTC,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            };

            let order_id = order.id;
            self.order_executor.execute_order(order.clone()).await?;
            
            // 활성 주문 추가
            {
                let mut active_orders = self.active_orders.write().unwrap();
                active_orders.insert(order_id, order);
            }

            // 주문 체결 대기 (간소화된 로직)
            tokio::time::sleep(Duration::from_secs(30)).await;
            
            // 체결된 것으로 가정하고 업데이트
            task.executed_size += order_size;
            remaining_size -= order_size;

            tracing::debug!("Iceberg 주문 처리: {}/{} ({}%)", 
                task.executed_size, 
                task.total_size,
                (task.executed_size / task.total_size * 100.0) as u32
            );
        }

        task.status = ExecutionStatus::Completed;
        tracing::info!("Iceberg 실행 완료: {}", task.symbol);
        
        Ok(())
    }
}

impl TrailingStopExecutor {
    pub fn new(
        price_feed: Arc<PriceFeedManager>,
        order_executor: Arc<OrderExecutor>,
    ) -> Self {
        Self {
            price_feed,
            order_executor,
            trailing_levels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn execute_trailing_stop(
        &self,
        task: &mut ExecutionTask,
        trigger_price: f64,
        trail_amount: f64,
        side: OrderSide,
        _config: &ExecutionConfig,
    ) -> Result<()> {
        tracing::info!("Trailing Stop 실행 시작: {} (트리거: {}, 추적: {})", 
            task.symbol, trigger_price, trail_amount);
        
        task.status = ExecutionStatus::Running;
        let mut current_trail_level = trigger_price;

        // 초기 추적 레벨 설정
        {
            let mut levels = self.trailing_levels.write().unwrap();
            levels.insert(task.symbol.clone(), current_trail_level);
        }

        // 가격 모니터링 루프
        while task.status == ExecutionStatus::Running {
            let current_price = self.price_feed.get_current_price(&task.symbol).await?;
            
            // 추적 레벨 업데이트
            let mut trigger_order = false;
            match side {
                OrderSide::Sell => {
                    // 매도 trailing stop: 가격이 상승하면 stop level을 올림
                    if current_price > current_trail_level + trail_amount {
                        current_trail_level = current_price - trail_amount;
                        let mut levels = self.trailing_levels.write().unwrap();
                        levels.insert(task.symbol.clone(), current_trail_level);
                        tracing::debug!("Trailing Stop 레벨 업데이트: {} -> {}", 
                            task.symbol, current_trail_level);
                    }
                    // 현재 가격이 stop level 아래로 떨어지면 매도 실행
                    if current_price <= current_trail_level {
                        trigger_order = true;
                    }
                }
                OrderSide::Buy => {
                    // 매수 trailing stop: 가격이 하락하면 stop level을 내림
                    if current_price < current_trail_level - trail_amount {
                        current_trail_level = current_price + trail_amount;
                        let mut levels = self.trailing_levels.write().unwrap();
                        levels.insert(task.symbol.clone(), current_trail_level);
                        tracing::debug!("Trailing Stop 레벨 업데이트: {} -> {}", 
                            task.symbol, current_trail_level);
                    }
                    // 현재 가격이 stop level 위로 올라가면 매수 실행
                    if current_price >= current_trail_level {
                        trigger_order = true;
                    }
                }
            }

            if trigger_order {
                tracing::info!("Trailing Stop 트리거: {} (가격: {}, 레벨: {})", 
                    task.symbol, current_price, current_trail_level);

                let order = Order {
                    id: Uuid::new_v4(),
                    symbol: task.symbol.clone(),
                    side,
                    order_type: OrderType::Market,
                    quantity: task.total_size,
                    price: None,
                    time_in_force: TimeInForce::IOC,
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
                };

                self.order_executor.execute_order(order).await?;
                task.executed_size = task.total_size;
                task.status = ExecutionStatus::Completed;
                break;
            }

            // 1초마다 가격 체크
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // 추적 레벨 정리
        {
            let mut levels = self.trailing_levels.write().unwrap();
            levels.remove(&task.symbol);
        }

        tracing::info!("Trailing Stop 실행 완료: {}", task.symbol);
        Ok(())
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            min_order_size: 10.0,
            max_slices: 100,
            min_order_interval: 5,
            allow_market_orders: true,
        }
    }
}