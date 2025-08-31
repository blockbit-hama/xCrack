use super::traits::*;
use anyhow::Result;
use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

/// 0x API 응답 구조체
#[derive(Debug, Clone, Deserialize)]
struct ZeroExQuote {
    #[serde(rename = "sellToken")]
    sell_token: String,
    #[serde(rename = "buyToken")]
    buy_token: String,
    #[serde(rename = "sellAmount")]
    sell_amount: String,
    #[serde(rename = "buyAmount")]
    buy_amount: String,
    to: String,
    data: String,
    #[serde(rename = "allowanceTarget")]
    allowance_target: Option<String>,
    value: String,
    gas: String,
    #[serde(rename = "gasPrice")]
    gas_price: String,
    #[serde(rename = "minimumProtocolFee")]
    minimum_protocol_fee: String,
    #[serde(rename = "protocolFee")]
    protocol_fee: String,
    #[serde(rename = "estimatedGas")]
    estimated_gas: String,
    #[serde(rename = "priceImpact")]
    price_impact: Option<String>,
    #[serde(rename = "sources")]
    sources: Option<Vec<Source>>,
}

#[derive(Debug, Clone, Deserialize)]
struct Source {
    name: String,
    proportion: String,
}

/// 0x 어댑터
pub struct ZeroExAdapter {
    config: AdapterConfig,
    base_url: String,
    metrics: AdapterMetrics,
}

impl ZeroExAdapter {
    pub fn new(config: AdapterConfig) -> Self {
        Self {
            base_url: "https://api.0x.org/swap/v1".to_string(),
            config,
            metrics: AdapterMetrics::default(),
        }
    }
    
    /// 0x API에서 견적 조회
    async fn fetch_quote(
        &self,
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        slippage_percentage: f64,
    ) -> Result<ZeroExQuote, AdapterError> {
        let url = format!(
            "{}/quote?sellToken={}&buyToken={}&sellAmount={}&slippagePercentage={}",
            self.base_url,
            format!("{:x}", sell_token),
            format!("{:x}", buy_token),
            sell_amount.to_string(),
            slippage_percentage
        );
        
        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        
        // API 키가 있으면 헤더에 추가
        if let Some(api_key) = &self.config.api_key {
            request = request.header("0x-api-key", api_key);
        }
        
        let response = request
            .timeout(std::time::Duration::from_secs(self.config.timeout_seconds))
            .send()
            .await
            .map_err(|e| AdapterError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AdapterError::InvalidResponse(
                format!("HTTP {}: {}", status, body)
            ));
        }
        
        let quote: ZeroExQuote = response
            .json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(quote)
    }
    
    /// 0x 견적을 내부 Quote 구조체로 변환
    fn convert_quote(
        &self,
        zeroex_quote: ZeroExQuote,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        let amount_out = U256::from_str_radix(&zeroex_quote.buy_amount, 10)
            .map_err(|e| AdapterError::InvalidResponse(format!("Invalid buyAmount: {}", e)))?;
        
        let amount_out_min = amount_out * U256::from(10000 - slippage_bps) / U256::from(10000);
        
        let price_impact = zeroex_quote.price_impact
            .and_then(|pi| pi.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let gas_estimate = zeroex_quote.estimated_gas
            .parse::<u64>()
            .unwrap_or(200000);
        
        let mut metadata = HashMap::new();
        metadata.insert("to".to_string(), zeroex_quote.to);
        metadata.insert("data".to_string(), zeroex_quote.data);
        metadata.insert("value".to_string(), zeroex_quote.value);
        metadata.insert("gas".to_string(), zeroex_quote.gas);
        metadata.insert("gas_price".to_string(), zeroex_quote.gas_price);
        metadata.insert("protocol_fee".to_string(), zeroex_quote.protocol_fee);
        
        if let Some(sources) = zeroex_quote.sources {
            let source_names: Vec<String> = sources.iter().map(|s| s.name.clone()).collect();
            metadata.insert("sources".to_string(), source_names.join(","));
        }
        
        Ok(Quote {
            token_in,
            token_out,
            amount_in,
            amount_out,
            amount_out_min,
            price_impact,
            gas_estimate,
            valid_for: 60, // 0x 견적은 보통 1분 유효
            timestamp: chrono::Utc::now().timestamp() as u64,
            metadata,
        })
    }
}

#[async_trait]
impl DexAdapter for ZeroExAdapter {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    
    fn name(&self) -> &str {
        "0x"
    }
    
    fn dex_type(&self) -> DexType {
        DexType::ZeroEx
    }
    
    async fn quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        let start_time = std::time::Instant::now();
        
        // 슬리피지를 퍼센트로 변환
        let slippage_percentage = slippage_bps as f64 / 100.0;
        
        // 0x API에서 견적 조회
        let zeroex_quote = self.fetch_quote(token_in, token_out, amount_in, slippage_percentage).await?;
        
        // 내부 구조체로 변환
        let quote = self.convert_quote(zeroex_quote, token_in, token_out, amount_in, slippage_bps)?;
        
        let response_time = start_time.elapsed().as_millis() as f64;
        
        // 메트릭 업데이트
        let mut metrics = self.metrics.clone();
        metrics.total_quotes += 1;
        metrics.successful_quotes += 1;
        metrics.avg_response_time_ms = (metrics.avg_response_time_ms * (metrics.total_quotes - 1) as f64 + response_time) / metrics.total_quotes as f64;
        metrics.last_success = Some(chrono::Utc::now().timestamp() as u64);
        metrics.consecutive_failures = 0;
        
        debug!("0x quote: {} -> {} = {} ({}ms)", amount_in, quote.amount_out, response_time, quote.metadata.get("sources").unwrap_or(&"unknown".to_string()));
        
        Ok(quote)
    }
    
    async fn build_swap_calldata(
        &self,
        quote: &Quote,
        _recipient: Address, // 0x는 recipient를 calldata에 포함
        deadline: u64,
    ) -> Result<CalldataBundle, AdapterError> {
        // 0x 견적에서 직접 calldata 추출
        let to = quote.metadata.get("to")
            .ok_or_else(|| AdapterError::CalldataGenerationFailed { message: "Missing 'to' in quote metadata".to_string() })?;
        
        let data = quote.metadata.get("data")
            .ok_or_else(|| AdapterError::CalldataGenerationFailed { message: "Missing 'data' in quote metadata".to_string() })?;
        
        let value = quote.metadata.get("value")
            .and_then(|v| U256::from_str_radix(v, 10).ok())
            .unwrap_or(U256::ZERO);
        
        // allowance target 추출 (집계기 특성상 중요)
        let spender = quote.metadata.get("allowance_target")
            .and_then(|addr| addr.parse().ok());
        
        let to_address: Address = to.parse()
            .map_err(|e| AdapterError::CalldataGenerationFailed { message: format!("Invalid 'to' address: {}", e) })?;
        
        let calldata = hex::decode(data.trim_start_matches("0x"))
            .map_err(|e| AdapterError::CalldataGenerationFailed { message: format!("Invalid calldata: {}", e) })?;
        
        Ok(CalldataBundle {
            to: to_address,
            spender,
            data: calldata,
            value,
            gas_estimate: quote.gas_estimate,
            deadline,
        })
    }
    
    async fn validate_quote(&self, quote: &Quote) -> Result<bool, AdapterError> {
        // 견적 유효성 검증
        if quote.amount_out == U256::ZERO {
            return Ok(false);
        }
        
        if quote.price_impact > 0.1 { // 10% 이상 가격 영향
            return Ok(false);
        }
        
        // 시간 유효성 확인
        let now = chrono::Utc::now().timestamp() as u64;
        if now - quote.timestamp > quote.valid_for {
            return Ok(false);
        }
        
        // 필수 메타데이터 확인
        if !quote.metadata.contains_key("to") || !quote.metadata.contains_key("data") {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn supports_pair(&self, _token_in: Address, _token_out: Address) -> Result<bool, AdapterError> {
        // 0x는 대부분의 ERC-20 토큰 페어를 지원
        // 실제 구현에서는 토큰 리스트 API 호출 가능
        Ok(true)
    }
    
    async fn get_min_amount(&self, _token: Address) -> Result<U256, AdapterError> {
        // 0x 최소 거래 수량 (Mock)
        Ok(U256::from(100)) // 0.0001 토큰
    }
    
    async fn get_fee_info(&self, _token_in: Address, _token_out: Address) -> Result<FeeInfo, AdapterError> {
        // 0x는 프로토콜 수수료가 있음
        Ok(FeeInfo {
            trading_fee_bps: 0, // 0x 자체 수수료는 없음
            platform_fee_bps: 0, // 프로토콜 수수료는 별도
            total_fee_bps: 0,
            fee_recipient: None,
        })
    }
}

impl ZeroExAdapter {
    /// 메트릭 조회
    pub fn get_metrics(&self) -> &AdapterMetrics {
        &self.metrics
    }
    
    /// 메트릭 리셋
    pub fn reset_metrics(&mut self) {
        self.metrics = AdapterMetrics::default();
    }
    
    /// 0x API 키 설정
    pub fn set_api_key(&mut self, api_key: String) {
        self.config.api_key = Some(api_key);
    }
    
    /// 지원하는 소스 DEX 목록 조회
    pub async fn get_supported_sources(&self) -> Result<Vec<String>, AdapterError> {
        // 0x API에서 지원하는 소스 목록 조회
        let url = format!("{}/sources", self.base_url);
        
        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        
        if let Some(api_key) = &self.config.api_key {
            request = request.header("0x-api-key", api_key);
        }
        
        let response = request
            .timeout(std::time::Duration::from_secs(self.config.timeout_seconds))
            .send()
            .await
            .map_err(|e| AdapterError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(AdapterError::NetworkError("Failed to fetch sources".to_string()));
        }
        
        let sources: Vec<String> = response
            .json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(sources)
    }
}