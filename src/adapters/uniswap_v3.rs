use super::traits::*;
use anyhow::Result;
use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

/// Uniswap V3 라우터 어댑터
pub struct UniswapV3Adapter {
    config: AdapterConfig,
    router_address: Address,
    quoter_address: Address,
    factory_address: Address,
    metrics: AdapterMetrics,
}

impl UniswapV3Adapter {
    pub fn new(config: AdapterConfig) -> Self {
        Self {
            router_address: Address::from_slice(&[
                0xe5, 0x92, 0x42, 0x7a, 0x0a, 0xec, 0xe9, 0x2d, 0xe3, 0xed, 0xee, 0x1f, 0x18, 0xe0, 0x15, 0x7c, 0x05, 0x86, 0x15, 0x64
            ]),
            quoter_address: Address::from_slice(&[
                0xb2, 0x73, 0x08, 0xf9, 0xf3, 0xcf, 0x30, 0x3d, 0x44, 0x4a, 0x27, 0x9b, 0x22, 0x04, 0x5b, 0x5a, 0x5e, 0x5a, 0x5e, 0x5a
            ]),
            factory_address: Address::from_slice(&[
                0x1f, 0x98, 0x43, 0x41, 0xa8, 0x85, 0x8c, 0x2d, 0x65, 0x9e, 0x2e, 0xbd, 0x4d, 0x5b, 0x4c, 0x6f, 0x96, 0x8a, 0xde, 0x4d
            ]),
            config,
            metrics: AdapterMetrics::default(),
        }
    }
    
    /// Uniswap V3 풀 주소 계산
    async fn get_pool_address(&self, token_a: Address, token_b: Address, fee: u32) -> Result<Address> {
        // 실제 구현에서는 팩토리에서 풀 주소 조회
        // 여기서는 Mock 구현
        let mut pool_bytes = [0u8; 20];
        pool_bytes[0] = 0x13;
        pool_bytes[1..4].copy_from_slice(&fee.to_be_bytes()[1..]);
        pool_bytes[4..].copy_from_slice(&token_a.as_slice()[4..]);
        Ok(Address::from_slice(&pool_bytes))
    }
    
    /// Uniswap V3 견적 계산 (Quoter 시뮬레이션)
    async fn quote_exact_input_single(&self, token_in: Address, token_out: Address, amount_in: U256, fee: u32) -> Result<U256> {
        // Mock 계산: 실제로는 Quoter 컨트랙트 호출 필요
        let output_amount = amount_in * U256::from(995) / U256::from(1000); // 0.5% 수수료
        Ok(output_amount)
    }
    
    /// exactInputSingle calldata 생성
    fn encode_exact_input_single(
        &self,
        token_in: Address,
        token_out: Address,
        fee: u32,
        recipient: Address,
        deadline: u64,
        amount_in: U256,
        amount_out_minimum: U256,
        sqrt_price_limit_x96: U256,
    ) -> Result<Vec<u8>> {
        use alloy::sol_types::SolCall;
        
        // Uniswap V3 Router ABI
        alloy::sol! {
            interface IUniswapV3Router {
                struct ExactInputSingleParams {
                    address tokenIn;
                    address tokenOut;
                    uint24 fee;
                    address recipient;
                    uint256 deadline;
                    uint256 amountIn;
                    uint256 amountOutMinimum;
                    uint160 sqrtPriceLimitX96;
                }
                
                function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
            }
        }
        
        let params = IUniswapV3Router::ExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee: alloy::primitives::Uint::<24, 1>::from(fee),
            recipient,
            deadline: U256::from(deadline),
            amountIn: amount_in,
            amountOutMinimum: amount_out_minimum,
            sqrtPriceLimitX96: alloy::primitives::Uint::<160, 3>::from_limbs(sqrt_price_limit_x96.as_limbs()[0..3].try_into().unwrap()),
        };
        
        let call = IUniswapV3Router::exactInputSingleCall { params };
        Ok(call.abi_encode().to_vec())
    }
    
    /// 최적 수수료 레벨 선택
    async fn select_best_fee(&self, token_in: Address, token_out: Address, amount_in: U256) -> Result<u32> {
        // Uniswap V3 수수료 레벨: 100, 500, 3000, 10000 (0.01%, 0.05%, 0.3%, 1%)
        let fees = [100, 500, 3000, 10000];
        let mut best_fee = 3000; // 기본값
        let mut best_output = U256::ZERO;
        
        for &fee in &fees {
            if let Ok(output) = self.quote_exact_input_single(token_in, token_out, amount_in, fee).await {
                if output > best_output {
                    best_output = output;
                    best_fee = fee;
                }
            }
        }
        
        Ok(best_fee)
    }
}

#[async_trait]
impl DexAdapter for UniswapV3Adapter {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    
    fn name(&self) -> &str {
        "uniswap_v3"
    }
    
    fn dex_type(&self) -> DexType {
        DexType::UniswapV3
    }
    
    async fn quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        let start_time = std::time::Instant::now();
        
        // 최적 수수료 레벨 선택
        let fee = self.select_best_fee(token_in, token_out, amount_in)
            .await
            .map_err(|e| AdapterError::QuoteFailed { message: e.to_string() })?;
        
        // 견적 계산
        let amount_out = self.quote_exact_input_single(token_in, token_out, amount_in, fee)
            .await
            .map_err(|e| AdapterError::QuoteFailed { message: e.to_string() })?;
        
        let amount_out_min = amount_out * U256::from(10000 - slippage_bps) / U256::from(10000);
        
        // 가격 영향 계산 (Mock)
        let price_impact = 0.005; // 0.5% (V3는 일반적으로 더 낮음)
        
        let response_time = start_time.elapsed().as_millis() as f64;
        
        // 메트릭 업데이트
        let mut metrics = self.metrics.clone();
        metrics.total_quotes += 1;
        metrics.successful_quotes += 1;
        metrics.avg_response_time_ms = (metrics.avg_response_time_ms * (metrics.total_quotes - 1) as f64 + response_time) / metrics.total_quotes as f64;
        metrics.last_success = Some(chrono::Utc::now().timestamp() as u64);
        metrics.consecutive_failures = 0;
        
        debug!("Uniswap V3 quote: {} -> {} = {} (fee: {})", amount_in, amount_out, response_time, fee);
        
        Ok(Quote {
            token_in,
            token_out,
            amount_in,
            amount_out,
            amount_out_min,
            price_impact,
            gas_estimate: 200000, // Uniswap V3 일반적인 가스 사용량
            valid_for: 60, // 1분
            timestamp: chrono::Utc::now().timestamp() as u64,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("router".to_string(), format!("{:?}", self.router_address));
                meta.insert("quoter".to_string(), format!("{:?}", self.quoter_address));
                meta.insert("fee".to_string(), fee.to_string());
                meta
            },
        })
    }
    
    async fn build_swap_calldata(
        &self,
        quote: &Quote,
        recipient: Address,
        deadline: u64,
    ) -> Result<CalldataBundle, AdapterError> {
        // 메타데이터에서 수수료 레벨 추출
        let fee = quote.metadata.get("fee")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(3000); // 기본값
        
        let calldata = self.encode_exact_input_single(
            quote.token_in,
            quote.token_out,
            fee,
            recipient,
            deadline,
            quote.amount_in,
            quote.amount_out_min,
            U256::ZERO, // sqrtPriceLimitX96 = 0 (무제한)
        ).map_err(|e| AdapterError::CalldataGenerationFailed { message: e.to_string() })?;
        
        Ok(CalldataBundle {
            to: self.router_address,
            spender: None, // 네이티브 라우터는 spender가 라우터와 동일
            data: calldata,
            value: U256::ZERO,
            gas_estimate: quote.gas_estimate,
            deadline,
        })
    }
    
    async fn validate_quote(&self, quote: &Quote) -> Result<bool, AdapterError> {
        // 견적 유효성 검증
        if quote.amount_out == U256::ZERO {
            return Ok(false);
        }
        
        if quote.price_impact > 0.02 { // 2% 이상 가격 영향 (V3는 더 엄격)
            return Ok(false);
        }
        
        // 시간 유효성 확인
        let now = chrono::Utc::now().timestamp() as u64;
        if now - quote.timestamp > quote.valid_for {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn supports_pair(&self, token_in: Address, token_out: Address) -> Result<bool, AdapterError> {
        // 실제 구현에서는 팩토리에서 풀 존재 확인
        // 여기서는 Mock: 주요 토큰 페어만 지원
        let supported_tokens = [
            Address::from_slice(&[0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e, 0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2]), // WETH
            Address::from_slice(&[0xa0, 0xb8, 0x69, 0x91, 0xc6, 0x21, 0x8b, 0x36, 0xc1, 0xd1, 0x9d, 0x4a, 0x2e, 0x9e, 0xb0, 0xce, 0x36, 0x06, 0xeb, 0x48]), // USDC
            Address::from_slice(&[0x6b, 0x17, 0x54, 0x74, 0xe8, 0x90, 0x94, 0xc4, 0x4d, 0xa9, 0x8b, 0x95, 0x4e, 0xed, 0xea, 0xc4, 0x95, 0x27, 0x1d, 0x0f]), // DAI
        ];
        
        Ok(supported_tokens.contains(&token_in) && supported_tokens.contains(&token_out))
    }
    
    async fn get_min_amount(&self, _token: Address) -> Result<U256, AdapterError> {
        // Uniswap V3 최소 거래 수량 (Mock)
        Ok(U256::from(100)) // 0.0001 토큰 (더 정밀함)
    }
    
    async fn get_fee_info(&self, token_in: Address, token_out: Address) -> Result<FeeInfo, AdapterError> {
        // 최적 수수료 레벨 조회
        let fee = self.select_best_fee(token_in, token_out, U256::from(1000000))
            .await
            .unwrap_or(3000);
        
        Ok(FeeInfo {
            trading_fee_bps: fee,
            platform_fee_bps: 0,
            total_fee_bps: fee,
            fee_recipient: None,
        })
    }
}

impl UniswapV3Adapter {
    /// 메트릭 조회
    pub fn get_metrics(&self) -> &AdapterMetrics {
        &self.metrics
    }
    
    /// 메트릭 리셋
    pub fn reset_metrics(&mut self) {
        self.metrics = AdapterMetrics::default();
    }
}