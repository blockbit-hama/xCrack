use anyhow::Result;
use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// DEX 어댑터 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Quote failed: {message}")]
    QuoteFailed { message: String },
    
    #[error("Calldata generation failed: {message}")]
    CalldataGenerationFailed { message: String },
    
    #[error("Unsupported token pair: {token_in} -> {token_out}")]
    UnsupportedTokenPair { token_in: Address, token_out: Address },
    
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,
    
    #[error("Slippage too high: {actual}% > {max}%")]
    SlippageTooHigh { actual: f64, max: f64 },
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// 견적 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    /// 입력 토큰 주소
    pub token_in: Address,
    /// 출력 토큰 주소
    pub token_out: Address,
    /// 입력 수량
    pub amount_in: U256,
    /// 예상 출력 수량
    pub amount_out: U256,
    /// 최소 출력 수량 (슬리피지 고려)
    pub amount_out_min: U256,
    /// 가격 영향 (0.0 ~ 1.0)
    pub price_impact: f64,
    /// 가스 추정량
    pub gas_estimate: u64,
    /// 견적 유효 시간 (초)
    pub valid_for: u64,
    /// 견적 생성 시간
    pub timestamp: u64,
    /// 추가 메타데이터
    pub metadata: HashMap<String, String>,
}

/// 실행용 calldata 번들
#[derive(Debug, Clone)]
pub struct CalldataBundle {
    /// 호출 대상 주소 (DEX 라우터 또는 집계기)
    pub to: Address,
    /// 승인 대상 주소 (집계기 사용 시 allowanceTarget)
    pub spender: Option<Address>,
    /// 실행할 calldata
    pub data: Vec<u8>,
    /// 전송할 ETH 값 (대부분 0)
    pub value: U256,
    /// 가스 추정량
    pub gas_estimate: u64,
    /// 데드라인 (Unix timestamp)
    pub deadline: u64,
}

/// DEX 어댑터 트레이트
#[async_trait]
pub trait DexAdapter: Send + Sync {
    /// Any를 위한 타입 캐스팅 지원
    fn as_any(&mut self) -> &mut dyn std::any::Any;
    
    /// 어댑터 이름
    fn name(&self) -> &str;
    
    /// 지원하는 DEX 타입
    fn dex_type(&self) -> DexType;
    
    /// 견적 조회
    async fn quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        slippage_bps: u64,
    ) -> Result<Quote, AdapterError>;
    
    /// 실행용 calldata 생성
    async fn build_swap_calldata(
        &self,
        quote: &Quote,
        recipient: Address,
        deadline: u64,
    ) -> Result<CalldataBundle, AdapterError>;
    
    /// 견적 유효성 검증
    async fn validate_quote(&self, quote: &Quote) -> Result<bool, AdapterError>;
    
    /// 지원하는 토큰 페어인지 확인
    async fn supports_pair(&self, token_in: Address, token_out: Address) -> Result<bool, AdapterError>;
    
    /// 최소 거래 수량 조회
    async fn get_min_amount(&self, token: Address) -> Result<U256, AdapterError>;
    
    /// 수수료 정보 조회
    async fn get_fee_info(&self, token_in: Address, token_out: Address) -> Result<FeeInfo, AdapterError>;
}

/// DEX 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DexType {
    /// Uniswap V2 (네이티브)
    UniswapV2,
    /// Uniswap V3 (네이티브)
    UniswapV3,
    /// SushiSwap (네이티브)
    SushiSwap,
    /// 0x Protocol (애그리게이터)
    ZeroEx,
    /// 1inch (애그리게이터)
    OneInch,
    /// 네이티브 라우터 (일반)
    NativeRouter,
    /// 애그리게이터 (일반)
    Aggregator,
}

impl DexType {
    /// DEX가 네이티브 프로토콜인지 확인
    pub fn is_native(&self) -> bool {
        matches!(self, DexType::UniswapV2 | DexType::UniswapV3 | DexType::SushiSwap | DexType::NativeRouter)
    }
    
    /// DEX가 애그리게이터인지 확인
    pub fn is_aggregator(&self) -> bool {
        matches!(self, DexType::ZeroEx | DexType::OneInch | DexType::Aggregator)
    }
    
    /// 가스 비용 가중치 (네이티브가 더 저렴)
    pub fn gas_weight(&self) -> f64 {
        match self {
            DexType::UniswapV2 | DexType::SushiSwap => 1.0,
            DexType::UniswapV3 => 1.1,  // V3는 약간 더 비쌈
            DexType::ZeroEx | DexType::OneInch => 1.3,  // 애그리게이터는 30% 더 비쌈
            DexType::NativeRouter => 1.0,  // 네이티브 라우터
            DexType::Aggregator => 1.3,   // 일반 애그리게이터
        }
    }
    
    /// 신뢰도 점수 (네이티브가 더 신뢰할 수 있음)
    pub fn reliability_score(&self) -> f64 {
        if self.is_native() {
            0.95
        } else {
            0.85  // 애그리게이터는 외부 의존성 때문에 약간 낮음
        }
    }
}

/// 수수료 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeInfo {
    /// 거래 수수료 (basis points, 300 = 0.3%)
    pub trading_fee_bps: u32,
    /// 플랫폼 수수료 (basis points)
    pub platform_fee_bps: u32,
    /// 총 수수료 (basis points)
    pub total_fee_bps: u32,
    /// 수수료 수취자 주소
    pub fee_recipient: Option<Address>,
}

/// 어댑터 설정
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    /// RPC URL
    pub rpc_url: String,
    /// API 키 (집계기용)
    pub api_key: Option<String>,
    /// 타임아웃 (초)
    pub timeout_seconds: u64,
    /// 최대 재시도 횟수
    pub max_retries: u32,
    /// 기본 슬리피지 (basis points)
    pub default_slippage_bps: u64,
    /// 가스 가격 승수
    pub gas_price_multiplier: f64,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://eth-mainnet.g.alchemy.com/v2/demo".to_string(),
            api_key: None,
            timeout_seconds: 30,
            max_retries: 3,
            default_slippage_bps: 50, // 0.5%
            gas_price_multiplier: 1.1,
        }
    }
}

/// 어댑터 성능 메트릭
#[derive(Debug, Clone, Default)]
pub struct AdapterMetrics {
    /// 총 견적 요청 수
    pub total_quotes: u64,
    /// 성공한 견적 수
    pub successful_quotes: u64,
    /// 실패한 견적 수
    pub failed_quotes: u64,
    /// 평균 응답 시간 (ms)
    pub avg_response_time_ms: f64,
    /// 마지막 성공 시간
    pub last_success: Option<u64>,
    /// 마지막 실패 시간
    pub last_failure: Option<u64>,
    /// 연속 실패 횟수
    pub consecutive_failures: u32,
}

impl AdapterMetrics {
    pub fn success_rate(&self) -> f64 {
        if self.total_quotes == 0 {
            return 0.0;
        }
        self.successful_quotes as f64 / self.total_quotes as f64
    }
    
    pub fn is_healthy(&self) -> bool {
        self.consecutive_failures < 5 && self.success_rate() > 0.8
    }
}

/// 견적 비교 결과
#[derive(Debug, Clone)]
pub struct QuoteComparison {
    /// 어댑터별 견적
    pub quotes: HashMap<String, Quote>,
    /// 최고 견적 어댑터
    pub best_adapter: String,
    /// 최고 출력 수량
    pub best_amount_out: U256,
    /// 견적 차이 (최고 대비)
    pub differences: HashMap<String, f64>,
}

/// 견적 비교기
pub struct QuoteComparator {
    adapters: HashMap<String, Box<dyn DexAdapter>>,
}

impl QuoteComparator {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }
    
    pub fn add_adapter(&mut self, name: String, adapter: Box<dyn DexAdapter>) {
        self.adapters.insert(name, adapter);
    }
    
    /// 여러 어댑터에서 견적 비교
    pub async fn compare_quotes(
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
                        tracing::warn!("Quote failed for {}: {}", name, e);
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
            quotes: quotes.clone(),
            best_adapter: best_adapter.clone(),
            best_amount_out: best_quote.amount_out,
            differences,
        })
    }
}

impl Default for QuoteComparator {
    fn default() -> Self {
        Self::new()
    }
}