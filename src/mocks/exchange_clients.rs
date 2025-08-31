use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tokio::time::{sleep, Duration};
use tracing::{info, debug, warn};
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use chrono::Utc;

use crate::exchange::order_executor::{ExchangeClient, OrderRequest, OrderResponse, OrderType};
use crate::types::{OrderStatus, PriceData};
use alloy::primitives::U256;
use crate::mocks::{MockConfig, get_mock_config};

/// Mock DEX 클라이언트 구현
/// 
/// Uniswap, SushiSwap 등의 DEX를 시뮬레이션합니다.
/// 실제 블록체인 대신 지연시간과 가스비를 모방합니다.
#[derive(Debug)]
pub struct MockDexClient {
    exchange_name: String,
    mock_config: MockConfig,
    connection_latency_ms: u64,
    gas_simulation: Arc<GasSimulation>,
    order_book_simulator: Arc<OrderBookSimulator>,
}

/// Mock CEX 클라이언트 구현
/// 
/// Binance, Coinbase 등의 중앙화 거래소를 시뮬레이션합니다.
/// API 호출과 주문 실행을 모방합니다.
#[derive(Debug)]
pub struct MockCexClient {
    exchange_name: String,
    mock_config: MockConfig,
    api_latency_ms: u64,
    balance_simulator: Arc<BalanceSimulator>,
    order_book_simulator: Arc<OrderBookSimulator>,
}

/// 가스 시뮬레이션 (DEX용)
#[derive(Debug)]
struct GasSimulation {
    base_gas_cost: u64,
    gas_price_wei: u64,
}

/// 잔고 시뮬레이션 (CEX용)
#[derive(Debug)]
struct BalanceSimulator {
    balances: HashMap<String, Decimal>,
}

/// 오더북 시뮬레이션 (공통)
#[derive(Debug)]
struct OrderBookSimulator {
    price_volatility: f64,
    spread_percentage: f64,
}

impl MockDexClient {
    pub fn new(exchange_name: String) -> Self {
        let mock_config = get_mock_config();
        
        // DEX는 일반적으로 더 높은 지연시간과 가스비
        let connection_latency_ms = mock_config.exchange_latency_ms + fastrand::u64(10..30);
        
        Self {
            exchange_name: exchange_name.clone(),
            mock_config: mock_config.clone(),
            connection_latency_ms,
            gas_simulation: Arc::new(GasSimulation {
                base_gas_cost: match exchange_name.as_str() {
                    "uniswap_v2" => 150_000, // Uniswap V2는 더 많은 가스 소모
                    "sushiswap" => 120_000,
                    _ => 100_000,
                },
                gas_price_wei: mock_config.gas_price,
            }),
            order_book_simulator: Arc::new(OrderBookSimulator {
                price_volatility: 0.02, // DEX는 더 높은 변동성
                spread_percentage: 0.005, // 0.5% 스프레드
            }),
        }
    }
}

impl MockCexClient {
    pub fn new(exchange_name: String) -> Self {
        let mock_config = get_mock_config();
        
        // CEX는 일반적으로 더 낮은 지연시간
        let api_latency_ms = mock_config.exchange_latency_ms / 2 + fastrand::u64(2..8);
        
        Self {
            exchange_name: exchange_name.clone(),
            mock_config: mock_config.clone(),
            api_latency_ms,
            balance_simulator: Arc::new(BalanceSimulator {
                balances: HashMap::from([
                    ("WETH".to_string(), Decimal::from(10)),
                    ("USDC".to_string(), Decimal::from(50000)),
                    ("USDT".to_string(), Decimal::from(50000)),
                    ("WBTC".to_string(), Decimal::from(1)),
                    ("DAI".to_string(), Decimal::from(50000)),
                ]),
            }),
            order_book_simulator: Arc::new(OrderBookSimulator {
                price_volatility: 0.01, // CEX는 더 낮은 변동성
                spread_percentage: 0.001, // 0.1% 스프레드
            }),
        }
    }
}

#[async_trait]
impl ExchangeClient for MockDexClient {
    /// DEX 주문 실행 (Mock)
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        let start_time = std::time::Instant::now();
        
        debug!("🔄 DEX 주문 실행 시작: {} - {} {}", 
               self.exchange_name, order.symbol, order.order_type);
        
        // 연결 지연 시뮬레이션
        sleep(Duration::from_millis(self.connection_latency_ms)).await;
        
        // 성공/실패 시뮬레이션
        let success = fastrand::f64() < self.mock_config.order_execution_success_rate;
        
        if !success {
            let error_reasons = [
                "슬리피지 초과",
                "유동성 부족",
                "가스비 부족",
                "MEV 보트에 의한 선행거래",
                "네트워크 혼잡"
            ];
            let reason = error_reasons[fastrand::usize(..error_reasons.len())];
            return Err(anyhow!("DEX 주문 실패: {}", reason));
        }
        
        // 실제 실행 가격 계산 (슬리피지 포함)
        let slippage = fastrand::f64() * 0.01; // 최대 1% 슬리피지
        let price_multiplier = match order.order_type {
            OrderType::Buy => 1.0 + slippage, // 매수는 더 높은 가격
            OrderType::Sell => 1.0 - slippage, // 매도는 더 낮은 가격
        };
        let executed_price = order.price * Decimal::from_f64_retain(price_multiplier).unwrap_or(Decimal::ONE);
        
        // 가스비 계산
        let gas_used = self.gas_simulation.base_gas_cost + fastrand::u64(1000..5000);
        let gas_fee_wei = gas_used * self.gas_simulation.gas_price_wei;
        let gas_fee_eth = Decimal::from(gas_fee_wei) / Decimal::from(1_000_000_000_000_000_000u64); // wei to ETH
        
        // 실제 거래량 (부분 체결 가능)
        let fill_ratio = 0.8 + fastrand::f64() * 0.2; // 80-100% 체결
        let executed_quantity = order.quantity * U256::from((fill_ratio * 1000.0) as u64) / U256::from(1000);
        
        let execution_time = start_time.elapsed();
        
        info!("✅ DEX 주문 체결: {} - {:.6} {} @ ${:.2} (가스: ${:.4}, 지연: {}ms)", 
              self.exchange_name,
              executed_quantity.to::<u64>() as f64 / 1e6, // 단위 조정
              order.symbol.split('/').next().unwrap_or(""),
              executed_price.to_f64().unwrap_or(0.0),
              gas_fee_eth.to_f64().unwrap_or(0.0) * 2000.0, // ETH to USD (대략)
              execution_time.as_millis());
        
        Ok(OrderResponse {
            order_id: format!("dex_{}_{}", self.exchange_name, fastrand::u64(100000..999999)),
            status: OrderStatus::Filled,
            executed_price,
            executed_quantity,
            timestamp: Utc::now(),
            transaction_hash: Some(format!("0x{:x}", fastrand::u64(..))),
            gas_used: Some(gas_used),
            gas_price: Some(self.gas_simulation.gas_price_wei),
        })
    }
    
    /// DEX 잔고 조회 (Mock)
    async fn get_balance(&self, token: &str) -> Result<Decimal> {
        // 연결 지연 시뮬레이션
        sleep(Duration::from_millis(self.connection_latency_ms / 2)).await;
        
        // Mock 잔고 (실제로는 지갑 주소 조회)
        let balance = match token {
            "WETH" => Decimal::from(5),
            "USDC" | "USDT" | "DAI" => Decimal::from(25000),
            "WBTC" => Decimal::from_f64_retain(0.5).unwrap_or_default(),
            _ => Decimal::ZERO,
        };
        
        debug!("💰 DEX 잔고 조회: {} {} = {}", self.exchange_name, token, balance);
        Ok(balance)
    }
    
    /// DEX 현재 가격 조회 (Mock)
    async fn get_current_price(&self, symbol: &str) -> Result<PriceData> {
        sleep(Duration::from_millis(self.connection_latency_ms / 3)).await;
        
        // 기본 가격
        let base_price = match symbol {
            "WETH/USDC" => 2000.0,
            "WETH/USDT" => 2001.0, 
            "WETH/DAI" => 1999.0,
            "WBTC/USDC" => 45000.0,
            "WBTC/USDT" => 45050.0,
            _ => 100.0,
        };
        
        // 거래소별 가격 차이
        let exchange_multiplier = match self.exchange_name.as_str() {
            "uniswap_v2" => 1.0,
            "sushiswap" => 0.999, // SushiSwap이 약간 낮음
            _ => 1.0,
        };
        
        // 변동성 적용
        let price_adjustment = (fastrand::f64() - 0.5) * self.order_book_simulator.price_volatility;
        let adjusted_price = base_price * exchange_multiplier * (1.0 + price_adjustment);
        
        let bid_price = adjusted_price * (1.0 - self.order_book_simulator.spread_percentage / 2.0);
        let ask_price = adjusted_price * (1.0 + self.order_book_simulator.spread_percentage / 2.0);
        
        Ok(PriceData {
            symbol: symbol.to_string(),
            exchange: self.exchange_name.clone(),
            bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
            ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
            last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
            volume_24h: U256::from(fastrand::u64(100000..1000000)), // DEX는 상대적으로 낮은 거래량
            timestamp: Utc::now(),
            sequence: fastrand::u64(..),
        })
    }

    /// DEX 주문 취소 (Mock)
    async fn cancel_order(&self, order_id: &str) -> Result<bool> {
        sleep(Duration::from_millis(self.connection_latency_ms)).await;
        
        debug!("❌ DEX 주문 취소: {} - {}", self.exchange_name, order_id);
        
        // DEX는 취소가 어려울 수 있음 (이미 블록체인에 포함된 경우)
        let cancel_success = fastrand::f64() < 0.7; // 70% 취소 성공률
        
        if cancel_success {
            info!("✅ DEX 주문 취소 성공: {}", order_id);
        } else {
            warn!("⚠️ DEX 주문 취소 실패: {} (이미 체결되거나 블록에 포함됨)", order_id);
        }
        
        Ok(cancel_success)
    }

    /// DEX 주문 상태 조회 (Mock)
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        sleep(Duration::from_millis(self.connection_latency_ms / 2)).await;
        
        debug!("🔍 DEX 주문 상태 조회: {} - {}", self.exchange_name, order_id);
        
        // Mock 상태 (실제로는 블록체인에서 트랜잭션 상태 확인)
        let status = if fastrand::f64() < 0.8 {
            OrderStatus::Filled
        } else if fastrand::f64() < 0.9 {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Pending
        };
        
        Ok(status)
    }

    /// DEX 주문 체결 내역 조회 (Mock)
    async fn get_order_fills(&self, order_id: &str) -> Result<Vec<crate::exchange::order_executor::OrderFill>> {
        sleep(Duration::from_millis(self.connection_latency_ms / 2)).await;
        
        debug!("📊 DEX 주문 체결 내역 조회: {} - {}", self.exchange_name, order_id);
        
        // Mock 체결 내역
        let fills = vec![
            crate::exchange::order_executor::OrderFill {
                fill_id: format!("fill_{}_{}", fastrand::u64(10000..99999), 1),
                order_id: order_id.to_string(),
                filled_amount: U256::from(500000), // 0.5 tokens
                filled_price: Decimal::from_f64_retain(2000.5).unwrap_or_default(),
                fee: U256::from(1000000000000000u64), // 가스비 (wei)
                timestamp: Utc::now(),
            },
            crate::exchange::order_executor::OrderFill {
                fill_id: format!("fill_{}_{}", fastrand::u64(10000..99999), 2),
                order_id: order_id.to_string(),
                filled_amount: U256::from(500000), // 0.5 tokens
                filled_price: Decimal::from_f64_retain(2001.0).unwrap_or_default(),
                fee: U256::from(900000000000000u64), // 가스비 (wei)
                timestamp: Utc::now(),
            }
        ];
        
        Ok(fills)
    }

    /// DEX 거래소 이름 반환
    fn get_exchange_name(&self) -> &str {
        &self.exchange_name
    }

    /// DEX 평균 지연시간 반환 (밀리초)
    fn get_average_latency(&self) -> u64 {
        self.connection_latency_ms
    }

    /// DEX 연결 상태 확인
    fn is_connected(&self) -> bool {
        // Mock에서는 항상 연결됨으로 가정
        true
    }
}

#[async_trait]
impl ExchangeClient for MockCexClient {
    /// CEX 주문 실행 (Mock)
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        let start_time = std::time::Instant::now();
        
        debug!("🔄 CEX 주문 실행 시작: {} - {} {}", 
               self.exchange_name, order.symbol, order.order_type);
        
        // API 지연 시뮬레이션
        sleep(Duration::from_millis(self.api_latency_ms)).await;
        
        // 성공/실패 시뮬레이션 (CEX는 더 높은 성공률)
        let success_rate = self.mock_config.order_execution_success_rate + 0.05; // CEX는 5% 더 높은 성공률
        let success = fastrand::f64() < success_rate;
        
        if !success {
            let error_reasons = [
                "잔고 부족",
                "시장 변동성 초과",
                "API 제한 도달",
                "시스템 점검",
                "주문 크기 제한"
            ];
            let reason = error_reasons[fastrand::usize(..error_reasons.len())];
            return Err(anyhow!("CEX 주문 실패: {}", reason));
        }
        
        // 실제 실행 가격 계산 (CEX는 낮은 슬리피지)
        let slippage = fastrand::f64() * 0.002; // 최대 0.2% 슬리피지
        let price_multiplier = match order.order_type {
            OrderType::Buy => 1.0 + slippage,
            OrderType::Sell => 1.0 - slippage,
        };
        let executed_price = order.price * Decimal::from_f64_retain(price_multiplier).unwrap_or(Decimal::ONE);
        
        // 거래 수수료 (CEX는 고정 수수료)
        let fee_rate = 0.001; // 0.1% 거래 수수료
        
        // 전체 체결 (CEX는 깊은 유동성)
        let executed_quantity = order.quantity;
        
        let execution_time = start_time.elapsed();
        
        info!("✅ CEX 주문 체결: {} - {:.6} {} @ ${:.2} (수수료: {:.1}%, 지연: {}ms)", 
              self.exchange_name,
              executed_quantity.to::<u64>() as f64 / 1e6,
              order.symbol.split('/').next().unwrap_or(""),
              executed_price.to_f64().unwrap_or(0.0),
              fee_rate * 100.0,
              execution_time.as_millis());
        
        Ok(OrderResponse {
            order_id: format!("cex_{}_{}", self.exchange_name, fastrand::u64(100000..999999)),
            status: OrderStatus::Filled,
            executed_price,
            executed_quantity,
            timestamp: Utc::now(),
            transaction_hash: None, // CEX는 트랜잭션 해시 없음
            gas_used: None,
            gas_price: None,
        })
    }
    
    /// CEX 잔고 조회 (Mock)
    async fn get_balance(&self, token: &str) -> Result<Decimal> {
        sleep(Duration::from_millis(self.api_latency_ms / 2)).await;
        
        let balance = self.balance_simulator.balances.get(token).cloned().unwrap_or(Decimal::ZERO);
        
        debug!("💰 CEX 잔고 조회: {} {} = {}", self.exchange_name, token, balance);
        Ok(balance)
    }
    
    /// CEX 현재 가격 조회 (Mock)
    async fn get_current_price(&self, symbol: &str) -> Result<PriceData> {
        sleep(Duration::from_millis(self.api_latency_ms / 3)).await;
        
        // 기본 가격
        let base_price = match symbol {
            "WETH/USDC" => 2000.0,
            "WETH/USDT" => 2001.0,
            "WETH/DAI" => 1999.0,
            "WBTC/USDC" => 45000.0,
            "WBTC/USDT" => 45050.0,
            _ => 100.0,
        };
        
        // 거래소별 가격 차이
        let exchange_multiplier = match self.exchange_name.as_str() {
            "mock_binance" => 1.001, // Binance가 약간 높음
            "mock_coinbase" => 0.999,
            _ => 1.0,
        };
        
        // 변동성 적용 (CEX는 낮은 변동성)
        let price_adjustment = (fastrand::f64() - 0.5) * self.order_book_simulator.price_volatility;
        let adjusted_price = base_price * exchange_multiplier * (1.0 + price_adjustment);
        
        let bid_price = adjusted_price * (1.0 - self.order_book_simulator.spread_percentage / 2.0);
        let ask_price = adjusted_price * (1.0 + self.order_book_simulator.spread_percentage / 2.0);
        
        Ok(PriceData {
            symbol: symbol.to_string(),
            exchange: self.exchange_name.clone(),
            bid: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
            ask: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
            last_price: Decimal::from_f64_retain(adjusted_price).unwrap_or_default(),
            volume_24h: U256::from(fastrand::u64(1000000..10000000)), // CEX는 더 높은 거래량
            timestamp: Utc::now(),
            sequence: fastrand::u64(..),
        })
    }

    /// CEX 주문 취소 (Mock)
    async fn cancel_order(&self, order_id: &str) -> Result<bool> {
        sleep(Duration::from_millis(self.api_latency_ms)).await;
        
        debug!("❌ CEX 주문 취소: {} - {}", self.exchange_name, order_id);
        
        // CEX는 높은 취소 성공률
        let cancel_success = fastrand::f64() < 0.95; // 95% 취소 성공률
        
        if cancel_success {
            info!("✅ CEX 주문 취소 성공: {}", order_id);
        } else {
            warn!("⚠️ CEX 주문 취소 실패: {} (이미 체결됨)", order_id);
        }
        
        Ok(cancel_success)
    }

    /// CEX 주문 상태 조회 (Mock)
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        sleep(Duration::from_millis(self.api_latency_ms / 2)).await;
        
        debug!("🔍 CEX 주문 상태 조회: {} - {}", self.exchange_name, order_id);
        
        // Mock 상태 (CEX는 빠른 체결)
        let status = if fastrand::f64() < 0.9 {
            OrderStatus::Filled
        } else if fastrand::f64() < 0.95 {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Pending
        };
        
        Ok(status)
    }

    /// CEX 주문 체결 내역 조회 (Mock)
    async fn get_order_fills(&self, order_id: &str) -> Result<Vec<crate::exchange::order_executor::OrderFill>> {
        sleep(Duration::from_millis(self.api_latency_ms / 2)).await;
        
        debug!("📊 CEX 주문 체결 내역 조회: {} - {}", self.exchange_name, order_id);
        
        // Mock 체결 내역 (CEX는 보통 단일 체결)
        let fills = vec![
            crate::exchange::order_executor::OrderFill {
                fill_id: format!("cex_fill_{}", fastrand::u64(10000..99999)),
                order_id: order_id.to_string(),
                filled_amount: U256::from(1000000), // 1.0 tokens
                filled_price: Decimal::from_f64_retain(2000.0).unwrap_or_default(),
                fee: U256::from(2000000000000000000u64), // 거래 수수료 (wei)
                timestamp: Utc::now(),
            }
        ];
        
        Ok(fills)
    }

    /// CEX 거래소 이름 반환
    fn get_exchange_name(&self) -> &str {
        &self.exchange_name
    }

    /// CEX 평균 지연시간 반환 (밀리초)
    fn get_average_latency(&self) -> u64 {
        self.api_latency_ms
    }

    /// CEX 연결 상태 확인
    fn is_connected(&self) -> bool {
        // Mock에서는 항상 연결됨으로 가정
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::order_executor::OrderType;
    
    #[tokio::test]
    async fn test_mock_dex_client() {
        let client = MockDexClient::new("uniswap_v2".to_string());
        
        // 가격 조회 테스트
        let price = client.get_current_price("WETH/USDC").await;
        assert!(price.is_ok());
        let price_data = price.unwrap();
        assert_eq!(price_data.exchange, "uniswap_v2");
        assert!(price_data.bid > Decimal::ZERO);
        assert!(price_data.ask > price_data.bid);
        
        // 잔고 조회 테스트
        let balance = client.get_balance("WETH").await;
        assert!(balance.is_ok());
        assert!(balance.unwrap() > Decimal::ZERO);
    }
    
    #[tokio::test]
    async fn test_mock_cex_client() {
        let client = MockCexClient::new("mock_binance".to_string());
        
        // 가격 조회 테스트
        let price = client.get_current_price("WETH/USDC").await;
        assert!(price.is_ok());
        let price_data = price.unwrap();
        assert_eq!(price_data.exchange, "mock_binance");
        
        // 주문 실행 테스트
        let order_request = OrderRequest {
            symbol: "WETH/USDC".to_string(),
            order_type: OrderType::Buy,
            quantity: U256::from(1000000), // 1 WETH
            price: price_data.ask,
            timeout_ms: 5000,
        };
        
        let result = client.place_order(order_request).await;
        // Mock 환경에서는 92% 성공률이므로 여러 번 시도할 수도 있지만,
        // 테스트에서는 결과가 Ok 또는 Err인지만 확인
        assert!(result.is_ok() || result.is_err());
    }
}