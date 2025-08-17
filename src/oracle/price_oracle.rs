use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, U256};
use std::time::{SystemTime, UNIX_EPOCH};
use rust_decimal::Decimal;

/// 가격 데이터 구조체
#[derive(Debug, Clone)]
pub struct PriceData {
    /// 토큰 주소
    pub token: Address,
    /// USD 가격
    pub price_usd: Decimal,
    /// ETH 가격
    pub price_eth: Decimal,
    /// 타임스탬프
    pub timestamp: u64,
    /// 가격 소스
    pub source: PriceSource,
    /// 신뢰도 점수 (0.0 ~ 1.0)
    pub confidence: f64,
    /// 24시간 변동률
    pub change_24h: Option<f64>,
    /// 거래량 (USD)
    pub volume_24h: Option<Decimal>,
}

/// 가격 소스 타입
#[derive(Debug, Clone, PartialEq)]
pub enum PriceSource {
    Chainlink,
    UniswapV2,
    UniswapV3,
    SushiSwap,
    Aggregated,
    Manual,
    CoinGecko,
    Binance,
}

/// 가격 오라클 트레이트
#[async_trait]
pub trait PriceOracle: Send + Sync {
    /// 토큰의 USD 가격을 가져옴
    async fn get_price_usd(&self, token: Address) -> Result<PriceData>;
    
    /// 토큰의 ETH 가격을 가져옴
    async fn get_price_eth(&self, token: Address) -> Result<PriceData>;
    
    /// 두 토큰 간의 가격 비율을 가져옴
    async fn get_price_ratio(&self, token_a: Address, token_b: Address) -> Result<Decimal>;
    
    /// 여러 토큰의 가격을 한 번에 가져옴
    async fn get_prices_batch(&self, tokens: &[Address]) -> Result<Vec<PriceData>>;
    
    /// TWAP (Time Weighted Average Price) 가져옴
    async fn get_twap(&self, token: Address, period_seconds: u64) -> Result<PriceData>;
    
    /// 가격 소스 타입 반환
    fn source_type(&self) -> PriceSource;
    
    /// 오라클 신뢰도 점수 반환
    fn reliability_score(&self) -> f64;
    
    /// 가격 업데이트 주기 (초)
    fn update_frequency(&self) -> u64;
}

/// 가격 피드 구조체
#[derive(Clone)]
pub struct PriceFeed {
    pub oracle: Arc<dyn PriceOracle>,
    pub priority: u8,  // 우선순위 (낮을수록 높은 우선순위)
    pub weight: f64,   // 가중치 (aggregation 시 사용)
}

impl PriceData {
    /// 새로운 가격 데이터 생성
    pub fn new(
        token: Address,
        price_usd: Decimal,
        price_eth: Decimal,
        source: PriceSource,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            token,
            price_usd,
            price_eth,
            timestamp,
            source,
            confidence: 1.0,
            change_24h: None,
            volume_24h: None,
        }
    }
    
    /// 가격이 만료되었는지 확인 (기본 5분)
    pub fn is_stale(&self, max_age_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.timestamp > max_age_seconds
    }
    
    /// 슬리피지 적용된 가격 계산
    pub fn with_slippage(&self, slippage_bps: u64, is_buy: bool) -> Decimal {
        let slippage = Decimal::from(slippage_bps) / Decimal::from(10000);
        
        if is_buy {
            self.price_usd * (Decimal::from(1) + slippage)
        } else {
            self.price_usd * (Decimal::from(1) - slippage)
        }
    }
    
    /// 가격 영향 계산
    pub fn calculate_price_impact(&self, amount: U256, liquidity: U256) -> f64 {
        if liquidity == U256::zero() {
            return 0.0;
        }
        
        let amount_f = amount.as_u128() as f64;
        let liquidity_f = liquidity.as_u128() as f64;
        
        // 간단한 x*y=k 공식 기반 계산
        let ratio = amount_f / liquidity_f;
        ratio * 100.0  // 퍼센트로 반환
    }
}

/// 가격 검증 유틸리티
pub struct PriceValidator;

impl PriceValidator {
    /// 가격 편차 확인
    pub fn check_deviation(price_a: &PriceData, price_b: &PriceData, max_deviation_pct: f64) -> bool {
        let deviation = ((price_a.price_usd - price_b.price_usd) / price_a.price_usd).abs();
        let deviation_pct = deviation.to_string().parse::<f64>().unwrap_or(0.0) * 100.0;
        
        deviation_pct <= max_deviation_pct
    }
    
    /// 가격 유효성 검증
    pub fn validate_price(price: &PriceData) -> Result<()> {
        // 가격이 0 이하인지 확인
        if price.price_usd <= Decimal::ZERO {
            return Err(anyhow::anyhow!("Invalid price: price_usd is zero or negative"));
        }
        
        // 타임스탬프가 미래 시간인지 확인
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if price.timestamp > now + 60 {  // 1분 이상 미래
            return Err(anyhow::anyhow!("Invalid timestamp: price is from the future"));
        }
        
        // 신뢰도 점수 확인
        if price.confidence < 0.0 || price.confidence > 1.0 {
            return Err(anyhow::anyhow!("Invalid confidence score: must be between 0 and 1"));
        }
        
        Ok(())
    }
}