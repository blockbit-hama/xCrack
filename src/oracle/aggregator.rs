use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use ethers::types::Address;
use rust_decimal::Decimal;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

use super::price_oracle::{PriceOracle, PriceSource, PriceData, PriceFeed, PriceValidator};

/// 가격 집계 전략
#[derive(Debug, Clone)]
pub enum AggregationStrategy {
    /// 중간값 사용
    Median,
    /// 평균값 사용
    Mean,
    /// 가중 평균 사용
    WeightedMean,
    /// 최빈값 사용
    Mode,
    /// 가장 신뢰할 수 있는 소스 사용
    MostReliable,
    /// 가장 최근 가격 사용
    MostRecent,
}

/// 다중 오라클 가격 집계기
pub struct PriceAggregator {
    /// 가격 피드 목록
    price_feeds: Vec<PriceFeed>,
    /// 집계 전략
    strategy: AggregationStrategy,
    /// 최대 가격 편차 (%)
    max_deviation_pct: f64,
    /// 최소 필요 소스 수
    min_sources: usize,
    /// 가격 캐시
    price_cache: Arc<RwLock<HashMap<Address, PriceData>>>,
    /// 캐시 유효 시간 (초)
    cache_ttl: u64,
}

impl PriceAggregator {
    /// 새로운 가격 집계기 생성
    pub fn new(strategy: AggregationStrategy) -> Self {
        Self {
            price_feeds: Vec::new(),
            strategy,
            max_deviation_pct: 5.0,  // 기본 5% 편차 허용
            min_sources: 2,  // 최소 2개 소스 필요
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: 60,  // 1분 캐시
        }
    }
    
    /// 가격 피드 추가
    pub fn add_feed(&mut self, oracle: Arc<dyn PriceOracle>, priority: u8, weight: f64) {
        self.price_feeds.push(PriceFeed {
            oracle,
            priority,
            weight,
        });
        
        // 우선순위로 정렬
        self.price_feeds.sort_by_key(|f| f.priority);
    }
    
    /// 여러 소스에서 가격 수집
    async fn collect_prices(&self, token: Address) -> Result<Vec<PriceData>> {
        let mut prices = Vec::new();
        
        for feed in &self.price_feeds {
            match feed.oracle.get_price_usd(token).await {
                Ok(price) => {
                    // 가격 유효성 검증
                    if let Err(e) = PriceValidator::validate_price(&price) {
                        warn!("Invalid price from {:?}: {}", price.source, e);
                        continue;
                    }
                    prices.push(price);
                }
                Err(e) => {
                    debug!("Failed to get price from feed: {}", e);
                }
            }
        }
        
        if prices.len() < self.min_sources {
            return Err(anyhow::anyhow!(
                "Not enough price sources: {} < {}",
                prices.len(),
                self.min_sources
            ));
        }
        
        Ok(prices)
    }
    
    /// 가격 집계
    fn aggregate_prices(&self, prices: Vec<PriceData>) -> Result<PriceData> {
        if prices.is_empty() {
            return Err(anyhow::anyhow!("No prices to aggregate"));
        }
        
        let aggregated_price = match self.strategy {
            AggregationStrategy::Median => self.calculate_median(&prices),
            AggregationStrategy::Mean => self.calculate_mean(&prices),
            AggregationStrategy::WeightedMean => self.calculate_weighted_mean(&prices),
            AggregationStrategy::Mode => self.calculate_mode(&prices),
            AggregationStrategy::MostReliable => self.get_most_reliable(&prices),
            AggregationStrategy::MostRecent => self.get_most_recent(&prices),
        };
        
        // 편차 확인
        self.check_deviation(&prices, &aggregated_price)?;
        
        Ok(aggregated_price)
    }
    
    /// 중간값 계산
    fn calculate_median(&self, prices: &[PriceData]) -> PriceData {
        let mut usd_prices: Vec<Decimal> = prices.iter().map(|p| p.price_usd).collect();
        usd_prices.sort();
        
        let median_usd = if usd_prices.len() % 2 == 0 {
            let mid = usd_prices.len() / 2;
            (usd_prices[mid - 1] + usd_prices[mid]) / Decimal::from(2)
        } else {
            usd_prices[usd_prices.len() / 2]
        };
        
        let mut eth_prices: Vec<Decimal> = prices.iter().map(|p| p.price_eth).collect();
        eth_prices.sort();
        
        let median_eth = if eth_prices.len() % 2 == 0 {
            let mid = eth_prices.len() / 2;
            (eth_prices[mid - 1] + eth_prices[mid]) / Decimal::from(2)
        } else {
            eth_prices[eth_prices.len() / 2]
        };
        
        let mut result = PriceData::new(
            prices[0].token,
            median_usd,
            median_eth,
            PriceSource::Aggregated,
        );
        
        // 평균 신뢰도 계산
        let avg_confidence: f64 = prices.iter().map(|p| p.confidence).sum::<f64>() / prices.len() as f64;
        result.confidence = avg_confidence;
        
        result
    }
    
    /// 평균값 계산
    fn calculate_mean(&self, prices: &[PriceData]) -> PriceData {
        let sum_usd: Decimal = prices.iter().map(|p| p.price_usd).sum();
        let sum_eth: Decimal = prices.iter().map(|p| p.price_eth).sum();
        let count = Decimal::from(prices.len() as u64);
        
        let mut result = PriceData::new(
            prices[0].token,
            sum_usd / count,
            sum_eth / count,
            PriceSource::Aggregated,
        );
        
        let avg_confidence: f64 = prices.iter().map(|p| p.confidence).sum::<f64>() / prices.len() as f64;
        result.confidence = avg_confidence;
        
        result
    }
    
    /// 가중 평균 계산
    fn calculate_weighted_mean(&self, prices: &[PriceData]) -> PriceData {
        let mut weighted_sum_usd = Decimal::ZERO;
        let mut weighted_sum_eth = Decimal::ZERO;
        let mut total_weight = 0.0;
        
        for (i, price) in prices.iter().enumerate() {
            // 해당 소스의 가중치 찾기
            let weight = self.price_feeds.iter()
                .find(|f| f.oracle.source_type() == price.source)
                .map(|f| f.weight)
                .unwrap_or(1.0);
            
            let weight_decimal = Decimal::try_from(weight).unwrap_or(Decimal::ONE);
            weighted_sum_usd += price.price_usd * weight_decimal;
            weighted_sum_eth += price.price_eth * weight_decimal;
            total_weight += weight;
        }
        
        let total_weight_decimal = Decimal::try_from(total_weight).unwrap_or(Decimal::ONE);
        
        let mut result = PriceData::new(
            prices[0].token,
            weighted_sum_usd / total_weight_decimal,
            weighted_sum_eth / total_weight_decimal,
            PriceSource::Aggregated,
        );
        
        let avg_confidence: f64 = prices.iter().map(|p| p.confidence).sum::<f64>() / prices.len() as f64;
        result.confidence = avg_confidence;
        
        result
    }
    
    /// 최빈값 계산 (가장 많이 나타나는 가격대)
    fn calculate_mode(&self, prices: &[PriceData]) -> PriceData {
        // 간단하게 중간값 반환 (실제로는 가격을 버킷으로 나누어 계산해야 함)
        self.calculate_median(prices)
    }
    
    /// 가장 신뢰할 수 있는 소스 선택
    fn get_most_reliable(&self, prices: &[PriceData]) -> PriceData {
        prices.iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .cloned()
            .unwrap()
    }
    
    /// 가장 최근 가격 선택
    fn get_most_recent(&self, prices: &[PriceData]) -> PriceData {
        prices.iter()
            .max_by_key(|p| p.timestamp)
            .cloned()
            .unwrap()
    }
    
    /// 가격 편차 확인
    fn check_deviation(&self, prices: &[PriceData], aggregated: &PriceData) -> Result<()> {
        for price in prices {
            if !PriceValidator::check_deviation(price, aggregated, self.max_deviation_pct) {
                warn!(
                    "Price deviation too high: {:?} vs aggregated ${} (>{:.1}%)",
                    price.source,
                    aggregated.price_usd,
                    self.max_deviation_pct
                );
            }
        }
        
        Ok(())
    }
    
    /// 캐시에서 가격 가져오기
    async fn get_from_cache(&self, token: Address) -> Option<PriceData> {
        let cache = self.price_cache.read().await;
        
        if let Some(price) = cache.get(&token) {
            if !price.is_stale(self.cache_ttl) {
                return Some(price.clone());
            }
        }
        
        None
    }
    
    /// 캐시에 가격 저장
    async fn save_to_cache(&self, price: PriceData) {
        let mut cache = self.price_cache.write().await;
        cache.insert(price.token, price);
    }
}

#[async_trait]
impl PriceOracle for PriceAggregator {
    async fn get_price_usd(&self, token: Address) -> Result<PriceData> {
        // 캐시 확인
        if let Some(cached) = self.get_from_cache(token).await {
            debug!("Using cached price for {:?}", token);
            return Ok(cached);
        }
        
        // 여러 소스에서 가격 수집
        let prices = self.collect_prices(token).await?;
        
        info!(
            "Collected {} prices for {:?}: {:?}",
            prices.len(),
            token,
            prices.iter().map(|p| (p.source.clone(), p.price_usd)).collect::<Vec<_>>()
        );
        
        // 가격 집계
        let aggregated = self.aggregate_prices(prices)?;
        
        // 캐시 저장
        self.save_to_cache(aggregated.clone()).await;
        
        info!("Aggregated price for {:?}: ${}", token, aggregated.price_usd);
        
        Ok(aggregated)
    }
    
    async fn get_price_eth(&self, token: Address) -> Result<PriceData> {
        self.get_price_usd(token).await
    }
    
    async fn get_price_ratio(&self, token_a: Address, token_b: Address) -> Result<Decimal> {
        let price_a = self.get_price_usd(token_a).await?;
        let price_b = self.get_price_usd(token_b).await?;
        
        Ok(price_a.price_usd / price_b.price_usd)
    }
    
    async fn get_prices_batch(&self, tokens: &[Address]) -> Result<Vec<PriceData>> {
        let mut prices = Vec::new();
        
        for token in tokens {
            match self.get_price_usd(*token).await {
                Ok(price) => prices.push(price),
                Err(e) => {
                    warn!("Failed to get aggregated price for {:?}: {}", token, e);
                }
            }
        }
        
        Ok(prices)
    }
    
    async fn get_twap(&self, token: Address, period_seconds: u64) -> Result<PriceData> {
        // TWAP를 지원하는 첫 번째 피드 사용
        for feed in &self.price_feeds {
            if let Ok(twap) = feed.oracle.get_twap(token, period_seconds).await {
                return Ok(twap);
            }
        }
        
        // TWAP를 지원하는 피드가 없으면 현재 가격 반환
        self.get_price_usd(token).await
    }
    
    fn source_type(&self) -> PriceSource {
        PriceSource::Aggregated
    }
    
    fn reliability_score(&self) -> f64 {
        // 모든 피드의 평균 신뢰도
        if self.price_feeds.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = self.price_feeds.iter()
            .map(|f| f.oracle.reliability_score() * f.weight)
            .sum();
        
        let total_weight: f64 = self.price_feeds.iter()
            .map(|f| f.weight)
            .sum();
        
        sum / total_weight
    }
    
    fn update_frequency(&self) -> u64 {
        // 가장 빠른 업데이트 주기 사용
        self.price_feeds.iter()
            .map(|f| f.oracle.update_frequency())
            .min()
            .unwrap_or(60)
    }
}