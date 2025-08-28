use super::traits::*;
use anyhow::Result;
use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn};

/// 1inch API 응답 구조체
#[derive(Debug, Clone, Deserialize)]
struct OneInchQuote {
    #[serde(rename = "fromToken")]
    from_token: TokenInfo,
    #[serde(rename = "toToken")]
    to_token: TokenInfo,
    #[serde(rename = "toTokenAmount")]
    to_token_amount: String,
    #[serde(rename = "fromTokenAmount")]
    from_token_amount: String,
    protocols: Vec<Vec<Protocol>>,
    #[serde(rename = "estimatedGas")]
    estimated_gas: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenInfo {
    symbol: String,
    name: String,
    address: String,
    decimals: u8,
    #[serde(rename = "logoURI")]
    logo_uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Protocol {
    name: String,
    part: f64,
    #[serde(rename = "fromTokenAddress")]
    from_token_address: String,
    #[serde(rename = "toTokenAddress")]
    to_token_address: String,
}

/// 1inch 스왑 응답 구조체
#[derive(Debug, Clone, Deserialize)]
struct OneInchSwap {
    #[serde(rename = "fromToken")]
    from_token: TokenInfo,
    #[serde(rename = "toToken")]
    to_token: TokenInfo,
    #[serde(rename = "toTokenAmount")]
    to_token_amount: String,
    #[serde(rename = "fromTokenAmount")]
    from_token_amount: String,
    protocols: Vec<Vec<Protocol>>,
    tx: TransactionData,
}

#[derive(Debug, Clone, Deserialize)]
struct TransactionData {
    from: String,
    to: String,
    data: String,
    value: String,
    gas: String,
    #[serde(rename = "gasPrice")]
    gas_price: String,
}

/// 1inch 어댑터
pub struct OneInchAdapter {
    config: AdapterConfig,
    base_url: String,
    chain_id: u32,
    metrics: AdapterMetrics,
}

impl OneInchAdapter {
    pub fn new(config: AdapterConfig, chain_id: u32) -> Self {
        Self {
            base_url: "https://api.1inch.dev/swap/v5.2".to_string(),
            chain_id,
            config,
            metrics: AdapterMetrics::default(),
        }
    }
    
    /// 1inch API에서 견적 조회
    async fn fetch_quote(
        &self,
        from_token: Address,
        to_token: Address,
        amount: U256,
    ) -> Result<OneInchQuote, AdapterError> {
        let url = format!(
            "{}/{}/quote?fromTokenAddress={}&toTokenAddress={}&amount={}",
            self.base_url,
            self.chain_id,
            format!("{:x}", from_token),
            format!("{:x}", to_token),
            amount.to_string()
        );
        
        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        
        // API 키가 있으면 헤더에 추가
        if let Some(api_key) = &self.config.api_key {
            request = request
                .header("Authorization", format!("Bearer {}", api_key))
                .header("apikey", api_key);
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
        
        let quote: OneInchQuote = response
            .json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(quote)
    }
    
    /// 1inch API에서 스왑 데이터 조회
    async fn fetch_swap(
        &self,
        from_token: Address,
        to_token: Address,
        amount: U256,
        slippage: f64,
        recipient: Address,
    ) -> Result<OneInchSwap, AdapterError> {
        let url = format!(
            "{}/{}/swap?fromTokenAddress={}&toTokenAddress={}&amount={}&fromAddress={}&slippage={}",
            self.base_url,
            self.chain_id,
            format!("{:x}", from_token),
            format!("{:x}", to_token),
            amount.to_string(),
            format!("{:x}", recipient),
            slippage
        );
        
        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        
        // API 키가 있으면 헤더에 추가
        if let Some(api_key) = &self.config.api_key {
            request = request
                .header("Authorization", format!("Bearer {}", api_key))
                .header("apikey", api_key);
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
        
        let swap: OneInchSwap = response
            .json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(swap)
    }
    
    /// 1inch 견적을 내부 Quote 구조체로 변환
    fn convert_quote(
        &self,
        oneinch_quote: OneInchQuote,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        let amount_out = U256::from_str_radix(&oneinch_quote.to_token_amount, 10)
            .map_err(|e| AdapterError::InvalidResponse(format!("Invalid toTokenAmount: {}", e)))?;
        
        let amount_out_min = amount_out * U256::from(10000 - slippage_bps) / U256::from(10000);
        
        let gas_estimate = oneinch_quote.estimated_gas
            .parse::<u64>()
            .unwrap_or(200000);
        
        // 프로토콜 정보 추출
        let mut protocol_names = Vec::new();
        for protocol_group in &oneinch_quote.protocols {
            for protocol in protocol_group {
                protocol_names.push(protocol.name.clone());
            }
        }
        
        let mut metadata = HashMap::new();
        metadata.insert("protocols".to_string(), protocol_names.join(","));
        metadata.insert("from_token_symbol".to_string(), oneinch_quote.from_token.symbol);
        metadata.insert("to_token_symbol".to_string(), oneinch_quote.to_token.symbol);
        
        Ok(Quote {
            token_in,
            token_out,
            amount_in,
            amount_out,
            amount_out_min,
            price_impact: 0.0, // 1inch는 견적에서 가격 영향 정보를 제공하지 않음
            gas_estimate,
            valid_for: 60, // 1inch 견적은 보통 1분 유효
            timestamp: chrono::Utc::now().timestamp() as u64,
            metadata,
        })
    }
}

#[async_trait]
impl DexAdapter for OneInchAdapter {
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    
    fn name(&self) -> &str {
        "1inch"
    }
    
    fn dex_type(&self) -> DexType {
        DexType::OneInch
    }
    
    async fn quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError> {
        let start_time = std::time::Instant::now();
        
        // 1inch API에서 견적 조회
        let oneinch_quote = self.fetch_quote(token_in, token_out, amount_in).await?;
        
        // 내부 구조체로 변환
        let quote = self.convert_quote(oneinch_quote, token_in, token_out, amount_in, slippage_bps)?;
        
        let response_time = start_time.elapsed().as_millis() as f64;
        
        // 메트릭 업데이트
        let mut metrics = self.metrics.clone();
        metrics.total_quotes += 1;
        metrics.successful_quotes += 1;
        metrics.avg_response_time_ms = (metrics.avg_response_time_ms * (metrics.total_quotes - 1) as f64 + response_time) / metrics.total_quotes as f64;
        metrics.last_success = Some(chrono::Utc::now().timestamp() as u64);
        metrics.consecutive_failures = 0;
        
        debug!("1inch quote: {} -> {} = {} ({}ms)", amount_in, quote.amount_out, response_time, quote.metadata.get("protocols").unwrap_or(&"unknown".to_string()));
        
        Ok(quote)
    }
    
    async fn build_swap_calldata(
        &self,
        quote: &Quote,
        recipient: Address,
        deadline: u64,
    ) -> Result<CalldataBundle, AdapterError> {
        // 슬리피지를 퍼센트로 변환
        let slippage = (U256::from(10000) - quote.amount_out_min * U256::from(10000) / quote.amount_out).to::<u64>() as f64 / 100.0;
        
        // 1inch API에서 스왑 데이터 조회
        let swap = self.fetch_swap(
            quote.token_in,
            quote.token_out,
            quote.amount_in,
            slippage,
            recipient,
        ).await?;
        
        let to_address = swap.tx.to.parse::<Address>()
            .map_err(|e| AdapterError::CalldataGenerationFailed { message: format!("Invalid 'to' address: {}", e) })?;
        
        let calldata = hex::decode(swap.tx.data.trim_start_matches("0x"))
            .map_err(|e| AdapterError::CalldataGenerationFailed { message: format!("Invalid calldata: {}", e) })?;
        
        let value = U256::from_str_radix(&swap.tx.value, 10)
            .unwrap_or(U256::ZERO);
        
        let gas_estimate = swap.tx.gas.parse::<u64>().unwrap_or(quote.gas_estimate);
        
        Ok(CalldataBundle {
            to: to_address,
            spender: None, // 1inch는 일반적으로 별도의 allowance target이 없음
            data: calldata,
            value,
            gas_estimate,
            deadline,
        })
    }
    
    async fn validate_quote(&self, quote: &Quote) -> Result<bool, AdapterError> {
        // 견적 유효성 검증
        if quote.amount_out == U256::ZERO {
            return Ok(false);
        }
        
        // 시간 유효성 확인
        let now = chrono::Utc::now().timestamp() as u64;
        if now - quote.timestamp > quote.valid_for {
            return Ok(false);
        }
        
        // 필수 메타데이터 확인
        if !quote.metadata.contains_key("protocols") {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn supports_pair(&self, _token_in: Address, _token_out: Address) -> Result<bool, AdapterError> {
        // 1inch는 대부분의 ERC-20 토큰 페어를 지원
        // 실제 구현에서는 토큰 리스트 API 호출 가능
        Ok(true)
    }
    
    async fn get_min_amount(&self, _token: Address) -> Result<U256, AdapterError> {
        // 1inch 최소 거래 수량 (Mock)
        Ok(U256::from(100)) // 0.0001 토큰
    }
    
    async fn get_fee_info(&self, _token_in: Address, _token_out: Address) -> Result<FeeInfo, AdapterError> {
        // 1inch는 프로토콜 수수료가 있음
        Ok(FeeInfo {
            trading_fee_bps: 0, // 1inch 자체 수수료는 없음
            platform_fee_bps: 0, // 프로토콜 수수료는 별도
            total_fee_bps: 0,
            fee_recipient: None,
        })
    }
}

impl OneInchAdapter {
    /// 메트릭 조회
    pub fn get_metrics(&self) -> &AdapterMetrics {
        &self.metrics
    }
    
    /// 메트릭 리셋
    pub fn reset_metrics(&mut self) {
        self.metrics = AdapterMetrics::default();
    }
    
    /// 1inch API 키 설정
    pub fn set_api_key(&mut self, api_key: String) {
        self.config.api_key = Some(api_key);
    }
    
    /// 지원하는 프로토콜 목록 조회
    pub async fn get_supported_protocols(&self) -> Result<Vec<String>, AdapterError> {
        // 1inch API에서 지원하는 프로토콜 목록 조회
        let url = format!("{}/{}/protocols", self.base_url, self.chain_id);
        
        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        
        if let Some(api_key) = &self.config.api_key {
            request = request
                .header("Authorization", format!("Bearer {}", api_key))
                .header("apikey", api_key);
        }
        
        let response = request
            .timeout(std::time::Duration::from_secs(self.config.timeout_seconds))
            .send()
            .await
            .map_err(|e| AdapterError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(AdapterError::NetworkError("Failed to fetch protocols".to_string()));
        }
        
        let protocols: Vec<String> = response
            .json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(protocols)
    }
    
    /// 토큰 정보 조회
    pub async fn get_token_info(&self, token_address: Address) -> Result<TokenInfo, AdapterError> {
        let url = format!(
            "{}/{}/tokens/{}",
            self.base_url,
            self.chain_id,
            format!("{:x}", token_address)
        );
        
        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        
        if let Some(api_key) = &self.config.api_key {
            request = request
                .header("Authorization", format!("Bearer {}", api_key))
                .header("apikey", api_key);
        }
        
        let response = request
            .timeout(std::time::Duration::from_secs(self.config.timeout_seconds))
            .send()
            .await
            .map_err(|e| AdapterError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(AdapterError::NetworkError("Failed to fetch token info".to_string()));
        }
        
        let token_info: TokenInfo = response
            .json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(token_info)
    }
}