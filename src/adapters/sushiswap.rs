use super::traits::*;
use anyhow::Result;
use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

/// SushiSwap 라우터 어댑터
pub struct SushiswapAdapter {
    config: AdapterConfig,
    router_address: Address,
    factory_address: Address,
    metrics: AdapterMetrics,
}

impl SushiswapAdapter {
    pub fn new(config: AdapterConfig) -> Self {
        Self {
            router_address: Address::from_slice(&[
                0xd9, 0xe1, 0xce, 0x17, 0xf2, 0x64, 0x1f, 0x24, 0xae, 0x83, 0x63, 0x7a, 0xb6, 0x6a, 0x2c, 0xca, 0x9c, 0x37, 0x8b, 0x9f
            ]),
            factory_address: Address::from_slice(&[
                0xc0, 0xae, 0xcb, 0x04, 0x6c, 0x5b, 0x40, 0x5a, 0x5e, 0x33, 0x5a, 0x5e, 0x33, 0x5a, 0x5e, 0x33, 0x5a, 0x5e, 0x33, 0x5a
            ]),
            config,
            metrics: AdapterMetrics::default(),
        }
    }
    
    /// SushiSwap 팩토리에서 페어 주소 조회
    async fn get_pair_address(&self, token_a: Address, token_b: Address) -> Result<Address> {
        // 실제 구현에서는 컨트랙트 호출 필요
        // 여기서는 Mock 구현
        let mut pair_bytes = [0u8; 20];
        pair_bytes[0] = 0x14;
        pair_bytes[1..].copy_from_slice(&token_a.as_slice()[1..]);
        Ok(Address::from_slice(&pair_bytes))
    }
    
    /// SushiSwap 견적 계산 (getAmountsOut 시뮬레이션)
    async fn calculate_amounts_out(&self, amount_in: U256, path: &[Address]) -> Result<Vec<U256>> {
        if path.len() < 2 {
            return Err(anyhow::anyhow!("Invalid path length"));
        }
        
        let mut amounts = vec![amount_in];
        
        for i in 0..path.len() - 1 {
            // Mock 계산: SushiSwap은 Uniswap V2와 유사하지만 약간 다른 수수료
            let current_amount = amounts[i];
            let output_amount = current_amount * U256::from(997) / U256::from(1000); // 0.3% 수수료
            amounts.push(output_amount);
        }
        
        Ok(amounts)
    }
    
    /// swapExactTokensForTokens calldata 생성 (SushiSwap은 Uniswap V2와 동일한 인터페이스)
    fn encode_swap_exact_tokens_for_tokens(
        &self,
        amount_in: U256,
        amount_out_min: U256,
        path: &[Address],
        to: Address,
        deadline: u64,
    ) -> Result<Vec<u8>> {
        use alloy::sol_types::SolCall;
        
        // SushiSwap Router ABI (Uniswap V2와 동일)
        alloy::sol! {
            interface ISushiSwapRouter {
                function swapExactTokensForTokens(
                    uint amountIn,
                    uint amountOutMin,
                    address[] calldata path,
                    address to,
                    uint deadline
                ) external returns (uint[] memory amounts);
            }
        }
        
        let call = ISushiSwapRouter::swapExactTokensForTokensCall {
            amountIn: amount_in,
            amountOutMin: amount_out_min,
            path: path.to_vec(),
            to,
            deadline: U256::from(deadline),
        };
        
        Ok(call.abi_encode().to_vec())
    }
}

#[async_trait]
impl DexAdapter for SushiswapAdapter {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    
    fn name(&self) -> &str {
        "sushiswap"
    }
    
    fn dex_type(&self) -> DexType {
        DexType::SushiSwap
    }
    
    async fn quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        let start_time = std::time::Instant::now();
        
        // 페어 존재 확인
        if !self.supports_pair(token_in, token_out).await? {
            return Err(AdapterError::UnsupportedTokenPair { token_in, token_out });
        }
        
        // 경로 구성
        let path = vec![token_in, token_out];
        
        // 견적 계산
        let amounts = self.calculate_amounts_out(amount_in, &path)
            .await
            .map_err(|e| AdapterError::QuoteFailed { message: e.to_string() })?;
        
        let amount_out = amounts.last().unwrap().clone();
        let amount_out_min = amount_out * U256::from(10000 - slippage_bps) / U256::from(10000);
        
        // 가격 영향 계산 (Mock)
        let price_impact = 0.008; // 0.8% (SushiSwap은 보통 Uniswap V2보다 약간 높음)
        
        let response_time = start_time.elapsed().as_millis() as f64;
        
        // 메트릭 업데이트
        let mut metrics = self.metrics.clone();
        metrics.total_quotes += 1;
        metrics.successful_quotes += 1;
        metrics.avg_response_time_ms = (metrics.avg_response_time_ms * (metrics.total_quotes - 1) as f64 + response_time) / metrics.total_quotes as f64;
        metrics.last_success = Some(chrono::Utc::now().timestamp() as u64);
        metrics.consecutive_failures = 0;
        
        debug!("SushiSwap quote: {} -> {} = {}", amount_in, amount_out, response_time);
        
        Ok(Quote {
            token_in,
            token_out,
            amount_in,
            amount_out,
            amount_out_min,
            price_impact,
            gas_estimate: 160000, // SushiSwap 일반적인 가스 사용량
            valid_for: 60, // 1분
            timestamp: chrono::Utc::now().timestamp() as u64,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("router".to_string(), format!("{:?}", self.router_address));
                meta.insert("path_length".to_string(), path.len().to_string());
                meta.insert("protocol".to_string(), "sushiswap".to_string());
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
        let path = vec![quote.token_in, quote.token_out];
        
        let calldata = self.encode_swap_exact_tokens_for_tokens(
            quote.amount_in,
            quote.amount_out_min,
            &path,
            recipient,
            deadline,
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
        
        if quote.price_impact > 0.05 { // 5% 이상 가격 영향
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
        // 실제 구현에서는 팩토리에서 페어 존재 확인
        // 여기서는 Mock: 주요 토큰 페어만 지원
        let supported_tokens = [
            Address::from_slice(&[0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e, 0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2]), // WETH
            Address::from_slice(&[0xa0, 0xb8, 0x69, 0x91, 0xc6, 0x21, 0x8b, 0x36, 0xc1, 0xd1, 0x9d, 0x4a, 0x2e, 0x9e, 0xb0, 0xce, 0x36, 0x06, 0xeb, 0x48]), // USDC
            Address::from_slice(&[0x6b, 0x17, 0x54, 0x74, 0xe8, 0x90, 0x94, 0xc4, 0x4d, 0xa9, 0x8b, 0x95, 0x4e, 0xed, 0xea, 0xc4, 0x95, 0x27, 0x1d, 0x0f]), // DAI
            Address::from_slice(&[0x22, 0x60, 0xfa, 0xce, 0x1e, 0xe9, 0x44, 0x42, 0x6f, 0x15, 0xda, 0xaf, 0x3c, 0x83, 0x96, 0x5f, 0x5c, 0x54, 0xdf, 0x2b]), // WBTC
        ];
        
        Ok(supported_tokens.contains(&token_in) && supported_tokens.contains(&token_out))
    }
    
    async fn get_min_amount(&self, _token: Address) -> Result<U256, AdapterError> {
        // SushiSwap 최소 거래 수량 (Mock)
        Ok(U256::from(1000)) // 0.001 토큰
    }
    
    async fn get_fee_info(&self, _token_in: Address, _token_out: Address) -> Result<FeeInfo, AdapterError> {
        Ok(FeeInfo {
            trading_fee_bps: 30, // 0.3% (Uniswap V2와 동일)
            platform_fee_bps: 0,
            total_fee_bps: 30,
            fee_recipient: None,
        })
    }
}

impl SushiswapAdapter {
    /// 메트릭 조회
    pub fn get_metrics(&self) -> &AdapterMetrics {
        &self.metrics
    }
    
    /// 메트릭 리셋
    pub fn reset_metrics(&mut self) {
        self.metrics = AdapterMetrics::default();
    }
}