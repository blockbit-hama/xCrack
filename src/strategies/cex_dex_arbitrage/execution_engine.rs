//! 아비트리지 실행 엔진
//! 
//! 이 모듈은 아비트리지 기회를 실제로 실행하고
//! 주문을 관리하는 실행 엔진을 제공합니다.

use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, debug, warn, error};
use ethers::prelude::*;
use ethers::types::{Address, U256, H256};
use rust_decimal::Decimal;
use chrono::Utc;

use crate::config::Config;
use crate::exchange::{ExchangeClient, ExchangeClientFactory};
use crate::types::OrderSide;
use super::types::{
    MicroArbitrageOpportunity, ArbitrageExecutionResult, OrderInfo, OrderStatus,
    FundingMode, FundingMetrics, ExecutionPriority, MicroArbitrageConfig
};
use super::aave_flashloan::AaveFlashLoanExecutor;

/// 아비트리지 실행 엔진
pub struct ExecutionEngine {
    config: Arc<Config>,
    exchange_clients: Arc<RwLock<HashMap<String, Arc<dyn ExchangeClient>>>>,
    active_orders: Arc<Mutex<HashMap<String, OrderInfo>>>,
    execution_history: Arc<Mutex<Vec<ArbitrageExecutionResult>>>,
    flashloan_executor: Option<Arc<AaveFlashLoanExecutor>>,
    provider: Arc<Provider<Ws>>,
    wallet: LocalWallet,

    // 설정
    max_concurrent_trades: usize,
    execution_timeout_ms: u64,
    max_slippage_percentage: f64,
    funding_mode: FundingMode,
}

impl ExecutionEngine {
    /// 새로운 실행 엔진 생성
    pub async fn new(
        config: Arc<Config>,
        provider: Arc<Provider<Ws>>,
        wallet: LocalWallet,
    ) -> Result<Self> {
        info!("⚡ 아비트리지 실행 엔진 초기화 중...");

        let micro_config = &config.strategies.micro_arbitrage;
        let mut exchange_clients = HashMap::new();

        // 거래소 클라이언트 초기화
        for exchange_config in &micro_config.exchanges {
            if exchange_config.enabled {
                let client = Self::create_exchange_client(exchange_config).await?;
                exchange_clients.insert(exchange_config.name.clone(), client);
            }
        }

        let funding_mode = match micro_config.funding_mode.to_lowercase().as_str() {
            "wallet" => FundingMode::Wallet,
            "flashloan" => FundingMode::FlashLoan,
            "auto" => FundingMode::Auto,
            _ => {
                warn!("⚠️ Unknown funding_mode: {}, defaulting to Auto", micro_config.funding_mode);
                FundingMode::Auto
            }
        };

        // FlashLoan Executor 초기화 (선택사항)
        let flashloan_executor = match AaveFlashLoanExecutor::new(
            provider.clone(),
            wallet.clone()
        ) {
            Ok(executor) => {
                info!("✅ FlashLoan Executor 초기화 완료");
                Some(Arc::new(executor))
            }
            Err(e) => {
                warn!("⚠️ FlashLoan Executor 초기화 실패 (Wallet 모드만 사용): {}", e);
                None
            }
        };

        info!("✅ 아비트리지 실행 엔진 초기화 완료 - {}개 거래소", exchange_clients.len());

        Ok(Self {
            config,
            exchange_clients: Arc::new(RwLock::new(exchange_clients)),
            active_orders: Arc::new(Mutex::new(HashMap::new())),
            execution_history: Arc::new(Mutex::new(Vec::new())),
            flashloan_executor,
            provider,
            wallet,
            max_concurrent_trades: micro_config.max_concurrent_trades,
            execution_timeout_ms: micro_config.execution_timeout_ms,
            max_slippage_percentage: 0.01, // 1% 기본값
            funding_mode,
        })
    }
    
    /// 아비트리지 기회 실행
    pub async fn execute_arbitrage(
        &self,
        opportunity: MicroArbitrageOpportunity,
    ) -> Result<ArbitrageExecutionResult> {
        let start_time = std::time::Instant::now();
        let execution_id = opportunity.id.clone();
        
        info!("🚀 아비트리지 실행 시작: {}", execution_id);
        info!("  📈 {}에서 매수: ${}", opportunity.buy_exchange, opportunity.buy_price);
        info!("  📉 {}에서 매도: ${}", opportunity.sell_exchange, opportunity.sell_price);
        info!("  💰 예상 수익: {:.4}%", opportunity.profit_percentage * 100.0);
        
        // 동시 실행 제한 확인
        if self.get_active_order_count().await >= self.max_concurrent_trades {
            return Ok(ArbitrageExecutionResult::failure(
                execution_id,
                "최대 동시 거래 수 초과".to_string(),
                start_time.elapsed().as_millis() as u64,
            ));
        }
        
        // 자금 조달 방식 결정
        let funding_metrics = self.determine_funding_mode(&opportunity).await?;
        
        // 실행 방식에 따른 분기
        let result = match funding_metrics.mode {
            FundingMode::Wallet => {
                self.execute_with_wallet(&opportunity, &funding_metrics).await
            }
            FundingMode::FlashLoan => {
                self.execute_with_flashloan(&opportunity, &funding_metrics).await
            }
            FundingMode::Auto => {
                // 자동 선택 로직
                if funding_metrics.net_profit > U256::zero() {
                    if funding_metrics.mode == FundingMode::FlashLoan {
                        self.execute_with_flashloan(&opportunity, &funding_metrics).await
                    } else {
                        self.execute_with_wallet(&opportunity, &funding_metrics).await
                    }
                } else {
                    Ok(ArbitrageExecutionResult::failure(
                        execution_id,
                        "수익성 부족으로 실행 취소".to_string(),
                        start_time.elapsed().as_millis() as u64,
                    ))
                }
            }
        };
        
        let execution_time = start_time.elapsed();
        let mut execution_result = result?;
        execution_result.execution_time_ms = execution_time.as_millis() as u64;
        
        // 실행 결과 기록
        self.record_execution_result(execution_result.clone()).await;
        
        if execution_result.success {
            info!("✅ 아비트리지 실행 성공: {} ({:.2}ms)", 
                  execution_id, execution_time.as_millis());
        } else {
            warn!("❌ 아비트리지 실행 실패: {} - {}", 
                  execution_id, execution_result.error_message.as_deref().unwrap_or("알 수 없는 오류"));
        }
        
        Ok(execution_result)
    }
    
    /// 지갑을 사용한 실행
    async fn execute_with_wallet(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        funding_metrics: &FundingMetrics,
    ) -> Result<ArbitrageExecutionResult> {
        let execution_id = opportunity.id.clone();
        
        info!("💳 지갑 모드로 아비트리지 실행");
        
        // 거래소 클라이언트 가져오기
        let clients = self.exchange_clients.read().await;
        let buy_client = clients.get(&opportunity.buy_exchange)
            .ok_or_else(|| anyhow!("매수 거래소 클라이언트를 찾을 수 없습니다: {}", opportunity.buy_exchange))?;
        let sell_client = clients.get(&opportunity.sell_exchange)
            .ok_or_else(|| anyhow!("매도 거래소 클라이언트를 찾을 수 없습니다: {}", opportunity.sell_exchange))?;
        
        // 잔고 확인
        self.check_balances(opportunity, buy_client, sell_client).await?;
        
        // 주문 실행
        let (buy_order, sell_order) = self.place_orders(opportunity, buy_client, sell_client).await?;
        
        // 주문 모니터링
        let execution_result = self.monitor_orders(
            execution_id,
            buy_order,
            sell_order,
            opportunity,
        ).await?;
        
        Ok(execution_result)
    }
    
    /// 플래시론을 사용한 실행
    async fn execute_with_flashloan(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        funding_metrics: &FundingMetrics,
    ) -> Result<ArbitrageExecutionResult> {
        let execution_id = opportunity.id.clone();
        
        info!("⚡ 플래시론 모드로 아비트리지 실행");
        
        // 플래시론 컨트랙트를 통한 실행
        // 실제 구현에서는 플래시론 컨트랙트 호출
        let flashloan_result = self.execute_flashloan_arbitrage(opportunity).await?;
        
        Ok(ArbitrageExecutionResult::success(
            execution_id,
            vec![H256::zero()], // 실제 트랜잭션 해시
            flashloan_result.actual_profit.unwrap_or(U256::zero()),
            flashloan_result.gas_used,
            0, // 실행 시간은 상위에서 설정
            flashloan_result.slippage,
            flashloan_result.fees_paid,
        ))
    }
    
    /// 자금 조달 방식 결정
    async fn determine_funding_mode(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<FundingMetrics> {
        match self.funding_mode {
            FundingMode::Wallet => {
                self.calculate_wallet_metrics(opportunity).await
            }
            FundingMode::FlashLoan => {
                self.calculate_flashloan_metrics(opportunity).await
            }
            FundingMode::Auto => {
                // 두 방식 모두 계산하여 비교
                let wallet_metrics = self.calculate_wallet_metrics(opportunity).await?;
                let flashloan_metrics = self.calculate_flashloan_metrics(opportunity).await?;
                
                // 더 수익성이 높은 방식 선택
                if flashloan_metrics.net_profit > wallet_metrics.net_profit {
                    Ok(flashloan_metrics)
                } else {
                    Ok(wallet_metrics)
                }
            }
        }
    }
    
    /// 지갑 방식 메트릭 계산
    async fn calculate_wallet_metrics(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<FundingMetrics> {
        let gross_profit = opportunity.expected_profit;
        
        // 가스 비용 계산
        let gas_cost = self.estimate_gas_cost().await?;
        
        // 총 비용 (가스만)
        let total_cost = gas_cost;
        let net_profit = if gross_profit > total_cost {
            gross_profit - total_cost
        } else {
            U256::zero()
        };
        
        // 잔고 확인
        let liquidity_available = self.check_wallet_balance(opportunity).await?;
        
        // 성공 확률 계산
        let success_probability = if liquidity_available { 0.95 } else { 0.0 };
        
        Ok(FundingMetrics {
            mode: FundingMode::Wallet,
            gross_profit,
            total_cost,
            net_profit,
            gas_cost,
            premium_cost: U256::zero(),
            success_probability,
            liquidity_available,
            estimated_execution_time_ms: 2000, // 2초
        })
    }
    
    /// 플래시론 방식 메트릭 계산
    async fn calculate_flashloan_metrics(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<FundingMetrics> {
        let gross_profit = opportunity.expected_profit;
        
        // 플래시론 수수료 계산 (9 bps)
        let flash_fee_bps = 9;
        let flash_premium = opportunity.buy_amount * U256::from(flash_fee_bps) / U256::from(10000);
        
        // 가스 비용 계산 (플래시론 경로)
        let gas_cost = self.estimate_flashloan_gas_cost().await?;
        
        // 총 비용
        let total_cost = flash_premium + gas_cost;
        let net_profit = if gross_profit > total_cost {
            gross_profit - total_cost
        } else {
            U256::zero()
        };
        
        // 플래시론 유동성 확인
        let liquidity_available = self.check_flashloan_liquidity(opportunity).await?;
        
        // 성공 확률 계산
        let mut success_probability = 0.85;
        if !liquidity_available {
            success_probability *= 0.3;
        }
        
        Ok(FundingMetrics {
            mode: FundingMode::FlashLoan,
            gross_profit,
            total_cost,
            net_profit,
            gas_cost,
            premium_cost: flash_premium,
            success_probability,
            liquidity_available,
            estimated_execution_time_ms: 5000, // 5초
        })
    }
    
    /// 잔고 확인
    async fn check_balances(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        buy_client: &Arc<dyn ExchangeClient>,
        sell_client: &Arc<dyn ExchangeClient>,
    ) -> Result<()> {
        // 매수 거래소에서 견적 자산 잔고 확인
        let required_quote = opportunity.buy_price * 
            Decimal::from_f64_retain(opportunity.buy_amount.as_u128() as f64 / 1e18)
                .unwrap_or_default();
        
        let quote_balance = buy_client.get_balance(&opportunity.quote_asset).await?;
        if quote_balance < required_quote {
            return Err(anyhow!("매수 거래소 잔고 부족: 필요 {} {}, 보유 {} {}", 
                              required_quote, opportunity.quote_asset, quote_balance, opportunity.quote_asset));
        }
        
        // 매도 거래소에서 기본 자산 잔고 확인
        let required_base = Decimal::from_f64_retain(opportunity.buy_amount.as_u128() as f64 / 1e18)
            .unwrap_or_default();
        
        let base_balance = sell_client.get_balance(&opportunity.base_asset).await?;
        if base_balance < required_base {
            return Err(anyhow!("매도 거래소 잔고 부족: 필요 {} {}, 보유 {} {}", 
                              required_base, opportunity.base_asset, base_balance, opportunity.base_asset));
        }
        
        Ok(())
    }
    
    /// 주문 실행
    async fn place_orders(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        buy_client: &Arc<dyn ExchangeClient>,
        sell_client: &Arc<dyn ExchangeClient>,
    ) -> Result<(String, String)> {
        let symbol = &opportunity.token_symbol;
        let amount = opportunity.buy_amount;
        
        // 병렬 주문 실행
        let (buy_result, sell_result) = tokio::join!(
            buy_client.place_buy_order(symbol, amount, opportunity.buy_price),
            sell_client.place_sell_order(symbol, amount, opportunity.sell_price)
        );
        
        let buy_order_id = buy_result?;
        let sell_order_id = sell_result?;
        
        info!("✅ 주문 실행 완료 - Buy: {}, Sell: {}", buy_order_id, sell_order_id);
        
        Ok((buy_order_id, sell_order_id))
    }
    
    /// 주문 모니터링
    async fn monitor_orders(
        &self,
        execution_id: String,
        buy_order_id: String,
        sell_order_id: String,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<ArbitrageExecutionResult> {
        let clients = self.exchange_clients.read().await;
        let buy_client = clients.get(&opportunity.buy_exchange).unwrap();
        let sell_client = clients.get(&opportunity.sell_exchange).unwrap();
        
        let max_wait_time = std::time::Duration::from_millis(opportunity.execution_window_ms);
        let check_interval = std::time::Duration::from_millis(500);
        let start_time = std::time::Instant::now();
        
        let mut buy_filled = false;
        let mut sell_filled = false;
        let mut buy_filled_amount = U256::zero();
        let mut sell_filled_amount = U256::zero();
        
        while start_time.elapsed() < max_wait_time {
            // 주문 상태 확인
            let (buy_status_result, sell_status_result) = tokio::join!(
                buy_client.get_order_status(&buy_order_id),
                sell_client.get_order_status(&sell_order_id)
            );
            
            match buy_status_result {
                Ok(OrderStatus::Filled) => {
                    if !buy_filled {
                        buy_filled = true;
                        buy_filled_amount = opportunity.buy_amount; // 실제로는 주문에서 가져와야 함
                        info!("✅ 매수 주문 체결: {}", buy_order_id);
                    }
                }
                Ok(OrderStatus::Cancelled) | Ok(OrderStatus::Rejected) => {
                    return Ok(ArbitrageExecutionResult::failure(
                        execution_id,
                        format!("매수 주문 실패: {:?}", buy_status_result.unwrap()),
                        start_time.elapsed().as_millis() as u64,
                    ));
                }
                _ => {}
            }
            
            match sell_status_result {
                Ok(OrderStatus::Filled) => {
                    if !sell_filled {
                        sell_filled = true;
                        sell_filled_amount = opportunity.buy_amount; // 실제로는 주문에서 가져와야 함
                        info!("✅ 매도 주문 체결: {}", sell_order_id);
                    }
                }
                Ok(OrderStatus::Cancelled) | Ok(OrderStatus::Rejected) => {
                    return Ok(ArbitrageExecutionResult::failure(
                        execution_id,
                        format!("매도 주문 실패: {:?}", sell_status_result.unwrap()),
                        start_time.elapsed().as_millis() as u64,
                    ));
                }
                _ => {}
            }
            
            // 양쪽 주문 모두 체결되면 성공
            if buy_filled && sell_filled {
                let actual_profit = self.calculate_actual_profit(
                    opportunity,
                    buy_filled_amount,
                    sell_filled_amount,
                ).await?;
                
                return Ok(ArbitrageExecutionResult::success(
                    execution_id,
                    vec![H256::zero()], // 실제 트랜잭션 해시
                    actual_profit,
                    U256::from(300_000), // 가스 사용량
                    start_time.elapsed().as_millis() as u64,
                    0.0, // 슬리피지
                    U256::zero(), // 수수료
                ));
            }
            
            tokio::time::sleep(check_interval).await;
        }
        
        // 타임아웃 발생
        Ok(ArbitrageExecutionResult::failure(
            execution_id,
            "주문 실행 타임아웃".to_string(),
            start_time.elapsed().as_millis() as u64,
        ))
    }
    
    /// 실제 수익 계산
    async fn calculate_actual_profit(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        buy_amount: U256,
        sell_amount: U256,
    ) -> Result<U256> {
        // 실제 구현에서는 정확한 수익 계산
        // 여기서는 간단한 추정치 사용
        let profit_rate = opportunity.profit_percentage;
        let actual_profit = buy_amount * U256::from((profit_rate * 10000.0) as u64) / U256::from(10000);
        Ok(actual_profit)
    }
    
    /// 플래시론 아비트리지 실행
    async fn execute_flashloan_arbitrage(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<ArbitrageExecutionResult> {
        let start_time = std::time::Instant::now();

        info!("⚡ FlashLoan 아비트리지 실행");

        // FlashLoan Executor 확인
        let executor = self.flashloan_executor.as_ref()
            .ok_or_else(|| anyhow!("FlashLoan executor not available"))?;

        // FlashLoan 실행
        match executor.execute_flashloan(opportunity).await {
            Ok(tx_hash) => {
                info!("✅ FlashLoan 트랜잭션 성공: {:?}", tx_hash);

                // 트랜잭션 영수증 조회
                let receipt = self.provider
                    .get_transaction_receipt(tx_hash)
                    .await?
                    .ok_or_else(|| anyhow!("Transaction receipt not found"))?;

                // 가스 사용량
                let gas_used = receipt.gas_used.unwrap_or(U256::from(500_000));

                // 실제 수익 계산 (로그에서 추출하거나 추정)
                let actual_profit = self.calculate_flashloan_profit(
                    opportunity,
                    &receipt
                ).await?;

                let execution_time_ms = start_time.elapsed().as_millis() as u64;

                Ok(ArbitrageExecutionResult::success(
                    opportunity.id.clone(),
                    vec![tx_hash],
                    actual_profit,
                    gas_used,
                    execution_time_ms,
                    0.0, // 슬리피지 (추후 계산)
                    U256::zero(), // 수수료 (추후 계산)
                ))
            }
            Err(e) => {
                error!("❌ FlashLoan 실행 실패: {}", e);
                Ok(ArbitrageExecutionResult::failure(
                    opportunity.id.clone(),
                    format!("FlashLoan execution failed: {}", e),
                    start_time.elapsed().as_millis() as u64,
                ))
            }
        }
    }
    
    /// 지갑 잔고 확인
    async fn check_wallet_balance(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<bool> {
        // 실제 지갑 잔고 조회
        let clients = self.exchange_clients.read().await;
        
        // 매수 거래소에서 견적 자산 잔고 확인
        if let Some(buy_client) = clients.get(&opportunity.buy_exchange) {
            let required_quote = opportunity.buy_price * 
                Decimal::from_f64_retain(opportunity.buy_amount.as_u128() as f64 / 1e18)
                    .unwrap_or_default();
            
            let quote_balance = buy_client.get_balance(&opportunity.quote_asset).await?;
            if quote_balance < required_quote {
                warn!("⚠️ 매수 거래소 잔고 부족: 필요 {} {}, 보유 {} {}", 
                      required_quote, opportunity.quote_asset, quote_balance, opportunity.quote_asset);
                return Ok(false);
            }
        }
        
        // 매도 거래소에서 기본 자산 잔고 확인
        if let Some(sell_client) = clients.get(&opportunity.sell_exchange) {
            let required_base = Decimal::from_f64_retain(opportunity.buy_amount.as_u128() as f64 / 1e18)
                .unwrap_or_default();
            
            let base_balance = sell_client.get_balance(&opportunity.base_asset).await?;
            if base_balance < required_base {
                warn!("⚠️ 매도 거래소 잔고 부족: 필요 {} {}, 보유 {} {}", 
                      required_base, opportunity.base_asset, base_balance, opportunity.base_asset);
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// 플래시론 유동성 확인
    async fn check_flashloan_liquidity(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<bool> {
        // FlashLoan Executor가 있으면 실제 유동성 조회
        if let Some(executor) = &self.flashloan_executor {
            match executor.get_available_liquidity(&opportunity.base_asset).await {
                Ok(available_liquidity) => {
                    let required = opportunity.buy_amount;
                    let available = available_liquidity;

                    info!("   Aave 유동성: {} / {} wei", available, required);

                    // 필요 금액이 사용 가능한 유동성의 90% 이하인지 확인
                    let max_safe_amount = available * U256::from(90) / U256::from(100);
                    Ok(required <= max_safe_amount)
                }
                Err(e) => {
                    warn!("⚠️ Aave 유동성 조회 실패: {}, 보수적으로 false 반환", e);
                    Ok(false)
                }
            }
        } else {
            // FlashLoan Executor가 없으면 사용 불가
            Ok(false)
        }
    }

    /// FlashLoan 수익 계산 (트랜잭션 영수증에서)
    async fn calculate_flashloan_profit(
        &self,
        opportunity: &MicroArbitrageOpportunity,
        receipt: &TransactionReceipt,
    ) -> Result<U256> {
        // 실제로는 트랜잭션 로그에서 수익 이벤트를 파싱
        // 지금은 예상 수익에서 가스비와 플래시론 수수료를 차감
        let gas_cost = receipt.gas_used.unwrap_or_default() *
            receipt.effective_gas_price.unwrap_or_default();

        let flashloan_premium = if let Some(executor) = &self.flashloan_executor {
            match executor.get_flashloan_premium().await {
                Ok(premium_bps) => {
                    opportunity.buy_amount * premium_bps / U256::from(10000)
                }
                Err(_) => {
                    // 기본 0.09%
                    opportunity.buy_amount * U256::from(9) / U256::from(10000)
                }
            }
        } else {
            U256::zero()
        };

        let total_cost = gas_cost + flashloan_premium;
        let gross_profit = opportunity.expected_profit;

        if gross_profit > total_cost {
            Ok(gross_profit - total_cost)
        } else {
            Ok(U256::zero())
        }
    }
    
    /// 가스 비용 추정
    async fn estimate_gas_cost(&self) -> Result<U256> {
        // 실제 provider에서 가스 가격 조회
        let gas_price = self.provider.get_gas_price().await?;

        // 환경변수에서 최대 가스 가격 확인
        let max_gas_price_gwei = std::env::var("MICRO_ARB_MAX_GAS_PRICE_GWEI")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u64>()
            .unwrap_or(100);

        let max_gas_price = U256::from(max_gas_price_gwei) * U256::from(1_000_000_000u64);

        // 최대 가스 가격 초과 시 경고
        let final_gas_price = if gas_price > max_gas_price {
            warn!("⚠️ 현재 가스 가격({} gwei)이 최대값({} gwei)을 초과함",
                  gas_price / U256::from(1_000_000_000u64), max_gas_price_gwei);
            max_gas_price
        } else {
            gas_price
        };

        let gas_limit = U256::from(300_000u64); // 300k gas (일반 아비트리지)
        Ok(final_gas_price * gas_limit)
    }

    /// 플래시론 가스 비용 추정
    async fn estimate_flashloan_gas_cost(&self) -> Result<U256> {
        let gas_price = self.provider.get_gas_price().await?;

        // 환경변수에서 최대 가스 가격 확인
        let max_gas_price_gwei = std::env::var("MICRO_ARB_MAX_GAS_PRICE_GWEI")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u64>()
            .unwrap_or(100);

        let max_gas_price = U256::from(max_gas_price_gwei) * U256::from(1_000_000_000u64);

        let final_gas_price = if gas_price > max_gas_price {
            max_gas_price
        } else {
            gas_price
        };

        let gas_limit = U256::from(500_000u64); // 500k gas (플래시론 포함)
        Ok(final_gas_price * gas_limit)
    }
    
    /// 거래소 클라이언트 생성
    async fn create_exchange_client(
        exchange_config: &crate::config::ExchangeConfig,
    ) -> Result<Arc<dyn ExchangeClient>> {
        match exchange_config.exchange_type {
            crate::config::ExchangeType::CEX => {
                match exchange_config.name.to_lowercase().as_str() {
                    "binance" => {
                        let api_key = std::env::var("BINANCE_API_KEY")
                            .or_else(|_| exchange_config.api_key.as_ref().cloned().ok_or_else(|| anyhow!("BINANCE_API_KEY not found")))?;
                        let secret_key = std::env::var("BINANCE_SECRET_KEY")
                            .or_else(|_| exchange_config.secret_key.as_ref().cloned().ok_or_else(|| anyhow!("BINANCE_SECRET_KEY not found")))?;
                        Ok(ExchangeClientFactory::create_binance_client(api_key, secret_key))
                    }
                    "coinbase" => {
                        let api_key = std::env::var("COINBASE_API_KEY")
                            .or_else(|_| exchange_config.api_key.as_ref().cloned().ok_or_else(|| anyhow!("COINBASE_API_KEY not found")))?;
                        let secret_key = std::env::var("COINBASE_SECRET_KEY")
                            .or_else(|_| exchange_config.secret_key.as_ref().cloned().ok_or_else(|| anyhow!("COINBASE_SECRET_KEY not found")))?;
                        let passphrase = std::env::var("COINBASE_PASSPHRASE")
                            .or_else(|_| exchange_config.passphrase.as_ref().cloned().ok_or_else(|| anyhow!("COINBASE_PASSPHRASE not found")))?;
                        Ok(ExchangeClientFactory::create_coinbase_client(api_key, secret_key, passphrase))
                    }
                    _ => {
                        warn!("⚠️ 지원되지 않는 CEX: {}, 실제 클라이언트 생성 시도", exchange_config.name);
                        // 기본 Binance 클라이언트로 폴백
                        let api_key = std::env::var("BINANCE_API_KEY").unwrap_or_default();
                        let secret_key = std::env::var("BINANCE_SECRET_KEY").unwrap_or_default();
                        Ok(ExchangeClientFactory::create_binance_client(api_key, secret_key))
                    }
                }
            }
            crate::config::ExchangeType::DEX => {
                match exchange_config.name.to_lowercase().as_str() {
                    "uniswap_v2" => Ok(ExchangeClientFactory::create_uniswap_v2_client()),
                    "uniswap_v3" => Ok(ExchangeClientFactory::create_uniswap_v3_client()),
                    "sushiswap" => Ok(ExchangeClientFactory::create_sushiswap_client()),
                    _ => {
                        warn!("⚠️ 지원되지 않는 DEX: {}, Uniswap V2로 폴백", exchange_config.name);
                        Ok(ExchangeClientFactory::create_uniswap_v2_client())
                    }
                }
            }
        }
    }
    
    /// 활성 주문 수 가져오기
    async fn get_active_order_count(&self) -> usize {
        let orders = self.active_orders.lock().await;
        orders.len()
    }
    
    /// 실행 결과 기록
    async fn record_execution_result(&self, result: ArbitrageExecutionResult) {
        let mut history = self.execution_history.lock().await;
        history.push(result);
        
        // 최근 1000개만 유지
        if history.len() > 1000 {
            history.drain(0..history.len() - 1000);
        }
    }
    
    /// 실행 통계 가져오기
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        let history = self.execution_history.lock().await;
        let active_orders = self.active_orders.lock().await;
        
        let total_executions = history.len() as u64;
        let successful_executions = history.iter().filter(|r| r.success).count() as u64;
        let success_rate = if total_executions > 0 {
            successful_executions as f64 / total_executions as f64
        } else {
            0.0
        };
        
        let total_profit: U256 = history.iter()
            .filter_map(|r| r.actual_profit)
            .sum();
        
        let avg_execution_time = if total_executions > 0 {
            history.iter().map(|r| r.execution_time_ms).sum::<u64>() as f64 / total_executions as f64
        } else {
            0.0
        };
        
        ExecutionStats {
            total_executions,
            successful_executions,
            success_rate,
            total_profit,
            avg_execution_time_ms: avg_execution_time,
            active_orders: active_orders.len() as u32,
        }
    }
}

/// 실행 통계
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub success_rate: f64,
    pub total_profit: U256,
    pub avg_execution_time_ms: f64,
    pub active_orders: u32,
}