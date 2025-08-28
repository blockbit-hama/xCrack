use super::traits::*;
use super::{UniswapV2Adapter, UniswapV3Adapter, SushiswapAdapter, ZeroExAdapter, OneInchAdapter};
use anyhow::Result;
use alloy::primitives::{Address, U256};
use std::collections::HashMap;
use tracing::{debug, warn, info};

/// DEX 어댑터 팩토리
pub struct DexAdapterFactory {
    adapters: HashMap<String, Box<dyn DexAdapter>>,
    config: AdapterConfig,
    chain_id: u32,
}

impl DexAdapterFactory {
    pub fn new(config: AdapterConfig, chain_id: u32) -> Self {
        Self {
            adapters: HashMap::new(),
            config,
            chain_id,
        }
    }
    
    /// 모든 어댑터 초기화
    pub fn initialize_all_adapters(&mut self) -> Result<()> {
        info!("Initializing DEX adapters for chain {}", self.chain_id);
        
        // 네이티브 라우터 어댑터들
        self.add_adapter("uniswap_v2", Box::new(UniswapV2Adapter::new(self.config.clone())))?;
        self.add_adapter("uniswap_v3", Box::new(UniswapV3Adapter::new(self.config.clone())))?;
        self.add_adapter("sushiswap", Box::new(SushiswapAdapter::new(self.config.clone())))?;
        
        // 집계기 어댑터들
        self.add_adapter("0x", Box::new(ZeroExAdapter::new(self.config.clone())))?;
        self.add_adapter("1inch", Box::new(OneInchAdapter::new(self.config.clone(), self.chain_id)))?;
        
        info!("Initialized {} DEX adapters", self.adapters.len());
        Ok(())
    }
    
    /// 특정 어댑터 추가
    pub fn add_adapter(&mut self, name: &str, adapter: Box<dyn DexAdapter>) -> Result<()> {
        self.adapters.insert(name.to_string(), adapter);
        debug!("Added adapter: {}", name);
        Ok(())
    }
    
    /// 어댑터 조회
    pub fn get_adapter(&self, name: &str) -> Option<&Box<dyn DexAdapter>> {
        self.adapters.get(name)
    }
    
    /// 모든 어댑터 목록
    pub fn get_all_adapters(&self) -> &HashMap<String, Box<dyn DexAdapter>> {
        &self.adapters
    }
    
    /// 지원하는 DEX 목록
    pub fn get_supported_dexes(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
    
    /// 네이티브 라우터 어댑터들만 조회
    pub fn get_native_adapters(&self) -> HashMap<String, &Box<dyn DexAdapter>> {
        self.adapters
            .iter()
            .filter(|(_, adapter)| adapter.dex_type() == DexType::NativeRouter)
            .map(|(name, adapter)| (name.clone(), adapter))
            .collect()
    }
    
    /// 집계기 어댑터들만 조회
    pub fn get_aggregator_adapters(&self) -> HashMap<String, &Box<dyn DexAdapter>> {
        self.adapters
            .iter()
            .filter(|(_, adapter)| adapter.dex_type() == DexType::Aggregator)
            .map(|(name, adapter)| (name.clone(), adapter))
            .collect()
    }
    
    /// 최적 어댑터 선택 (견적 비교)
    pub async fn select_best_adapter(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
        preferred_type: Option<DexType>,
    ) -> Result<Option<(String, Quote)>, AdapterError> {
        let mut best_quote: Option<(String, Quote)> = None;
        let mut tasks = Vec::new();
        
        // 선호 타입이 있으면 해당 타입만 필터링
        let adapters_to_use = if let Some(pref_type) = preferred_type {
            self.adapters
                .iter()
                .filter(|(_, adapter)| adapter.dex_type() == pref_type)
                .collect::<Vec<_>>()
        } else {
            self.adapters.iter().collect::<Vec<_>>()
        };
        
        // 모든 어댑터에서 병렬로 견적 요청
        for (name, adapter) in adapters_to_use {
            let name = name.clone();
            let adapter = adapter.as_ref();
            let task = async move {
                match adapter.quote(token_in, token_out, amount_in, slippage_bps).await {
                    Ok(quote) => Some((name, quote)),
                    Err(e) => {
                        warn!("Quote failed for {}: {}", name, e);
                        None
                    }
                }
            };
            tasks.push(task);
        }
        
        // 모든 견적 수집
        let results = futures::future::join_all(tasks).await;
        for result in results {
            if let Some((name, quote)) = result {
                if let Some((_, best)) = &best_quote {
                    if quote.amount_out > best.amount_out {
                        best_quote = Some((name, quote));
                    }
                } else {
                    best_quote = Some((name, quote));
                }
            }
        }
        
        Ok(best_quote)
    }
    
    /// 하이브리드 전략: 네이티브 우선, 실패 시 집계기
    pub async fn get_hybrid_quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<(String, Quote), AdapterError> {
        // 1단계: 네이티브 라우터에서 최적 견적 찾기
        if let Some((name, quote)) = self.select_best_adapter(
            token_in,
            token_out,
            amount_in,
            slippage_bps,
            Some(DexType::NativeRouter),
        ).await? {
            debug!("Using native router: {} with output: {}", name, quote.amount_out);
            return Ok((name, quote));
        }
        
        // 2단계: 네이티브 실패 시 집계기 사용
        if let Some((name, quote)) = self.select_best_adapter(
            token_in,
            token_out,
            amount_in,
            slippage_bps,
            Some(DexType::Aggregator),
        ).await? {
            debug!("Falling back to aggregator: {} with output: {}", name, quote.amount_out);
            return Ok((name, quote));
        }
        
        Err(AdapterError::QuoteFailed {
            message: "All adapters failed to provide quotes".to_string(),
        })
    }
    
    /// 견적 비교 (모든 어댑터)
    pub async fn compare_all_quotes(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<QuoteComparison, AdapterError> {
        let mut quotes = HashMap::new();
        let mut tasks = Vec::new();
        
        // 모든 어댑터에서 병렬로 견적 요청
        for (name, adapter) in &self.adapters {
            let name = name.clone();
            let adapter = adapter.as_ref();
            let task = async move {
                match adapter.quote(token_in, token_out, amount_in, slippage_bps).await {
                    Ok(quote) => Some((name, quote)),
                    Err(e) => {
                        warn!("Quote failed for {}: {}", name, e);
                        None
                    }
                }
            };
            tasks.push(task);
        }
        
        // 모든 견적 수집
        let results = futures::future::join_all(tasks).await;
        for result in results {
            if let Some((name, quote)) = result {
                quotes.insert(name, quote);
            }
        }
        
        if quotes.is_empty() {
            return Err(AdapterError::QuoteFailed {
                message: "All adapters failed to provide quotes".to_string(),
            });
        }
        
        // 최고 견적 찾기
        let (best_adapter, best_quote) = quotes
            .iter()
            .max_by_key(|(_, quote)| quote.amount_out)
            .unwrap();
        
        // 견적 차이 계산
        let mut differences = HashMap::new();
        for (name, quote) in &quotes {
            if name != best_adapter {
                let diff = (best_quote.amount_out.to::<u128>() as f64 - quote.amount_out.to::<u128>() as f64)
                    / quote.amount_out.to::<u128>() as f64 * 100.0;
                differences.insert(name.clone(), diff);
            }
        }
        
        Ok(QuoteComparison {
            quotes,
            best_adapter: best_adapter.clone(),
            best_amount_out: best_quote.amount_out,
            differences,
        })
    }
    
    /// 어댑터 상태 확인
    pub async fn check_adapter_health(&self) -> HashMap<String, bool> {
        let mut health_status = HashMap::new();
        
        for (name, adapter) in &self.adapters {
            // 간단한 토큰 페어로 테스트
            let test_token_in = Address::from_slice(&[0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e, 0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2]); // WETH
            let test_token_out = Address::from_slice(&[0xa0, 0xb8, 0x69, 0x91, 0xc6, 0x21, 0x8b, 0x36, 0xc1, 0xd1, 0x9d, 0x4a, 0x2e, 0x9e, 0xb0, 0xce, 0x36, 0x06, 0xeb, 0x48]); // USDC
            let test_amount = U256::from(1000000000000000000u64); // 1 ETH
            
            let is_healthy = adapter.quote(test_token_in, test_token_out, test_amount, 50).await.is_ok();
            health_status.insert(name.clone(), is_healthy);
        }
        
        health_status
    }
    
    /// 어댑터 메트릭 조회
    pub fn get_all_metrics(&self) -> HashMap<String, AdapterMetrics> {
        let mut metrics = HashMap::new();
        
        for (name, adapter) in &self.adapters {
            // 각 어댑터의 메트릭을 조회 (실제 구현에서는 어댑터별 메트릭 접근 필요)
            metrics.insert(name.clone(), AdapterMetrics::default());
        }
        
        metrics
    }
    
    /// 설정 업데이트
    pub fn update_config(&mut self, config: AdapterConfig) {
        self.config = config;
        // 필요시 어댑터들 재초기화
    }
    
    /// API 키 설정 (집계기용)
    pub fn set_api_keys(&mut self, api_keys: HashMap<String, String>) {
        for (adapter_name, api_key) in api_keys {
            if let Some(adapter) = self.adapters.get_mut(&adapter_name) {
                match adapter_name.as_str() {
                    "0x" => {
                        if let Some(zeroex_adapter) = adapter.as_any().downcast_mut::<ZeroExAdapter>() {
                            zeroex_adapter.set_api_key(api_key);
                        }
                    }
                    "1inch" => {
                        if let Some(oneinch_adapter) = adapter.as_any().downcast_mut::<OneInchAdapter>() {
                            oneinch_adapter.set_api_key(api_key);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Default for DexAdapterFactory {
    fn default() -> Self {
        Self::new(AdapterConfig::default(), 1) // Ethereum mainnet
    }
}

/// 어댑터 선택 전략
#[derive(Debug, Clone)]
pub enum AdapterSelectionStrategy {
    /// 최고 견적 선택
    BestQuote,
    /// 네이티브 라우터 우선
    NativeFirst,
    /// 집계기 우선
    AggregatorFirst,
    /// 하이브리드 (네이티브 실패 시 집계기)
    Hybrid,
    /// 특정 어댑터 고정
    Fixed(String),
}

/// 어댑터 선택기
pub struct AdapterSelector {
    factory: DexAdapterFactory,
    strategy: AdapterSelectionStrategy,
}

impl AdapterSelector {
    pub fn new(factory: DexAdapterFactory, strategy: AdapterSelectionStrategy) -> Self {
        Self { factory, strategy }
    }
    
    /// 전략에 따른 어댑터 선택
    pub async fn select_adapter(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<(String, Quote), AdapterError> {
        match &self.strategy {
            AdapterSelectionStrategy::BestQuote => {
                self.factory.select_best_adapter(token_in, token_out, amount_in, slippage_bps, None)
                    .await?
                    .ok_or_else(|| AdapterError::QuoteFailed { message: "No quotes available".to_string() })
            }
            AdapterSelectionStrategy::NativeFirst => {
                self.factory.select_best_adapter(token_in, token_out, amount_in, slippage_bps, Some(DexType::NativeRouter))
                    .await?
                    .ok_or_else(|| AdapterError::QuoteFailed { message: "No native router quotes available".to_string() })
            }
            AdapterSelectionStrategy::AggregatorFirst => {
                self.factory.select_best_adapter(token_in, token_out, amount_in, slippage_bps, Some(DexType::Aggregator))
                    .await?
                    .ok_or_else(|| AdapterError::QuoteFailed { message: "No aggregator quotes available".to_string() })
            }
            AdapterSelectionStrategy::Hybrid => {
                self.factory.get_hybrid_quote(token_in, token_out, amount_in, slippage_bps).await
            }
            AdapterSelectionStrategy::Fixed(adapter_name) => {
                let adapter = self.factory.get_adapter(adapter_name)
                    .ok_or_else(|| AdapterError::QuoteFailed { message: format!("Adapter {} not found", adapter_name) })?;
                
                let quote = adapter.quote(token_in, token_out, amount_in, slippage_bps).await?;
                Ok((adapter_name.clone(), quote))
            }
        }
    }
    
    /// 전략 변경
    pub fn set_strategy(&mut self, strategy: AdapterSelectionStrategy) {
        self.strategy = strategy;
    }
    
    /// 팩토리 접근
    pub fn factory(&self) -> &DexAdapterFactory {
        &self.factory
    }
    
    /// 팩토리 가변 접근
    pub fn factory_mut(&mut self) -> &mut DexAdapterFactory {
        &mut self.factory
    }
}