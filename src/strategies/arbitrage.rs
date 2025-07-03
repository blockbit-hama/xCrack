use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug, error, warn};
use ethers::prelude::*;
use async_trait::async_trait;

use crate::config::Config;
use crate::types::*;
use super::Strategy;
use ethers::providers::Middleware;

#[derive(Clone)]
pub struct MempoolArbitrageStrategy {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    is_enabled: bool,
}

impl MempoolArbitrageStrategy {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        Ok(Self {
            config,
            provider,
            is_enabled: true,
        })
    }

    /// 트랜잭션에서 차익거래 기회를 찾습니다
    async fn find_arbitrage_opportunities(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        let mut opportunities = Vec::new();
        
        // 트랜잭션 가치가 최소 임계값을 넘는지 확인
        let min_value = self.config.strategies.arbitrage.min_profit_threshold.parse::<f64>().unwrap_or(0.01);
        let min_value_wei = (min_value * 1_000_000_000_000_000_000.0) as u128;
        
        if transaction.value.as_u128() < min_value_wei {
            return Ok(opportunities);
        }

        // DEX 스왑 트랜잭션인지 확인
        if !self.is_dex_swap_transaction(transaction) {
            return Ok(opportunities);
        }

        // 토큰 페어 추출
        if let Some((token_in, token_out)) = self.extract_token_pair(transaction).await? {
            // 여러 DEX에서 가격 조회
            let dex_prices = self.get_dex_prices(token_in, token_out).await?;
            
            if dex_prices.len() >= 2 {
                // 차익거래 기회 계산
                let arbitrage_opportunities = self.calculate_arbitrage_opportunities(
                    &dex_prices, 
                    token_in, 
                    token_out, 
                    transaction
                ).await?;
                
                opportunities.extend(arbitrage_opportunities);
            }
        }

        Ok(opportunities)
    }

    /// 트랜잭션이 DEX 스왑인지 확인합니다
    fn is_dex_swap_transaction(&self, transaction: &Transaction) -> bool {
        use crate::mempool::filters;
        
        // Ethers 트랜잭션으로 변환
        let ethers_tx = ethers::types::Transaction {
            hash: transaction.hash,
            from: transaction.from,
            to: transaction.to,
            value: transaction.value,
            gas_price: transaction.gas_price,
            gas_limit: transaction.gas_limit,
            data: transaction.data.clone().into(),
            nonce: transaction.nonce.into(),
            block_number: transaction.block_number.map(|bn| bn.into()),
            ..Default::default()
        };
        
        filters::is_dex_swap(&ethers_tx)
    }

    /// 트랜잭션에서 토큰 페어를 추출합니다
    async fn extract_token_pair(&self, transaction: &Transaction) -> Result<Option<(H160, H160)>> {
        // Uniswap V2 스왑 데이터 파싱
        if transaction.data.len() >= 4 {
            let selector = &transaction.data[0..4];
            
            match selector {
                // swapExactETHForTokens
                [0x7f, 0xf3, 0x6a, 0xb5] => {
                    if transaction.data.len() >= 168 { // 최소 데이터 길이
                        // WETH 주소 (Uniswap V2)
                        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<H160>()?;
                        
                        // 토큰 주소는 데이터에서 추출 (offset 36-56)
                        let token_out_bytes = &transaction.data[36..56];
                        let token_out = H160::from_slice(token_out_bytes);
                        
                        return Ok(Some((weth, token_out)));
                    }
                },
                // swapExactTokensForTokens
                [0x38, 0xed, 0x17, 0x39] => {
                    if transaction.data.len() >= 168 {
                        // 토큰 주소들은 데이터에서 추출
                        let token_in_bytes = &transaction.data[36..56];
                        let token_out_bytes = &transaction.data[68..88];
                        
                        let token_in = H160::from_slice(token_in_bytes);
                        let token_out = H160::from_slice(token_out_bytes);
                        
                        return Ok(Some((token_in, token_out)));
                    }
                },
                _ => {}
            }
        }
        
        Ok(None)
    }

    /// 여러 DEX에서 토큰 가격을 조회합니다
    async fn get_dex_prices(&self, token_in: H160, token_out: H160) -> Result<Vec<DexPrice>> {
        let mut prices = Vec::new();
        let amount_in = ethers::utils::parse_ether("1").unwrap(); // 1 토큰 기준

        // Uniswap V2 가격 조회
        if let Ok(Some(price)) = self.get_uniswap_v2_price(token_in, token_out, amount_in).await {
            prices.push(price);
        }

        // SushiSwap 가격 조회
        if let Ok(Some(price)) = self.get_sushiswap_price(token_in, token_out, amount_in).await {
            prices.push(price);
        }

        debug!("Found {} DEX prices for {}/{}", prices.len(), token_in, token_out);
        Ok(prices)
    }

    /// Uniswap V2 가격 조회
    async fn get_uniswap_v2_price(&self, token_in: H160, token_out: H160, amount_in: U256) -> Result<Option<DexPrice>> {
        // Uniswap V2 Factory 주소
        let factory_address: H160 = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse()?;
        
        // Factory ABI (간단한 버전)
        let factory_abi = ethers::abi::parse_abi(&[
            "function getPair(address tokenA, address tokenB) external view returns (address pair)"
        ])?;
        
        let factory = Contract::new(factory_address, factory_abi, Arc::clone(&self.provider));
        
        // Pair 주소 가져오기
        let pair_address: Address = factory
            .method::<_, Address>("getPair", (token_in, token_out))?
            .call()
            .await?;

        if pair_address == Address::zero() {
            return Ok(None);
        }

        // Pair ABI
        let pair_abi = ethers::abi::parse_abi(&[
            "function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)",
            "function token0() external view returns (address)"
        ])?;

        let pair = Contract::new(pair_address, pair_abi, Arc::clone(&self.provider));
        
        // Reserves 가져오기
        let (reserve0, reserve1, _): (u128, u128, u32) = pair
            .method("getReserves", ())?
            .call()
            .await?;
        
        let token0: Address = pair.method("token0", ())?.call().await?;

        let (reserve_in, reserve_out) = if token0 == token_in {
            (U256::from(reserve0), U256::from(reserve1))
        } else {
            (U256::from(reserve1), U256::from(reserve0))
        };

        if reserve_in.is_zero() || reserve_out.is_zero() {
            return Ok(None);
        }

        // AMM 공식으로 출력량 계산
        let amount_out = self.calculate_amm_output(amount_in, reserve_in, reserve_out, 300); // 0.3% fee

        Ok(Some(DexPrice {
            dex: "uniswap_v2".to_string(),
            token_in,
            token_out,
            amount_in,
            amount_out,
            price: amount_out.as_u128() as f64 / amount_in.as_u128() as f64,
            liquidity: reserve_in + reserve_out,
            reserve_in,
            reserve_out,
            gas_estimate: 150_000,
        }))
    }

    /// SushiSwap 가격 조회
    async fn get_sushiswap_price(&self, token_in: H160, token_out: H160, amount_in: U256) -> Result<Option<DexPrice>> {
        // SushiSwap Factory 주소
        let factory_address: H160 = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse()?;
        
        // Factory ABI
        let factory_abi = ethers::abi::parse_abi(&[
            "function getPair(address tokenA, address tokenB) external view returns (address pair)"
        ])?;
        
        let factory = Contract::new(factory_address, factory_abi, Arc::clone(&self.provider));
        
        // Pair 주소 가져오기
        let pair_address: Address = factory
            .method::<_, Address>("getPair", (token_in, token_out))?
            .call()
            .await?;

        if pair_address == Address::zero() {
            return Ok(None);
        }

        // Pair ABI
        let pair_abi = ethers::abi::parse_abi(&[
            "function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)",
            "function token0() external view returns (address)"
        ])?;

        let pair = Contract::new(pair_address, pair_abi, Arc::clone(&self.provider));
        
        // Reserves 가져오기
        let (reserve0, reserve1, _): (u128, u128, u32) = pair
            .method("getReserves", ())?
            .call()
            .await?;
        
        let token0: Address = pair.method("token0", ())?.call().await?;

        let (reserve_in, reserve_out) = if token0 == token_in {
            (U256::from(reserve0), U256::from(reserve1))
        } else {
            (U256::from(reserve1), U256::from(reserve0))
        };

        if reserve_in.is_zero() || reserve_out.is_zero() {
            return Ok(None);
        }

        // AMM 공식으로 출력량 계산
        let amount_out = self.calculate_amm_output(amount_in, reserve_in, reserve_out, 300); // 0.3% fee

        Ok(Some(DexPrice {
            dex: "sushiswap".to_string(),
            token_in,
            token_out,
            amount_in,
            amount_out,
            price: amount_out.as_u128() as f64 / amount_in.as_u128() as f64,
            liquidity: reserve_in + reserve_out,
            reserve_in,
            reserve_out,
            gas_estimate: 150_000,
        }))
    }

    /// AMM 출력량 계산 (Uniswap V2 공식)
    fn calculate_amm_output(&self, amount_in: U256, reserve_in: U256, reserve_out: U256, fee: u32) -> U256 {
        let amount_in_with_fee = amount_in * U256::from(10000 - fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(10000) + amount_in_with_fee;
        
        if denominator.is_zero() {
            U256::zero()
        } else {
            numerator / denominator
        }
    }

    /// 차익거래 기회를 계산합니다
    async fn calculate_arbitrage_opportunities(
        &self,
        dex_prices: &[DexPrice],
        token_in: H160,
        token_out: H160,
        original_tx: &Transaction,
    ) -> Result<Vec<Opportunity>> {
        let mut opportunities = Vec::new();

        // 모든 DEX 조합에 대해 차익거래 기회 찾기
        for i in 0..dex_prices.len() {
            for j in i + 1..dex_prices.len() {
                let price1 = &dex_prices[i];
                let price2 = &dex_prices[j];

                // 가격 차이 계산
                let price_diff = (price1.price - price2.price).abs() / price1.price.min(price2.price);
                
                // 최소 수익성 임계값 (0.5%)
                if price_diff < 0.005 {
                    continue;
                }

                // 구매/판매 DEX 결정
                let (buy_dex, sell_dex) = if price1.price < price2.price {
                    (price1, price2)
                } else {
                    (price2, price1)
                };

                // 최적 거래 크기 계산
                let trade_size = self.calculate_optimal_trade_size(buy_dex, sell_dex).await?;
                
                if trade_size.is_zero() {
                    continue;
                }

                // 실제 수익 계산
                let buy_amount_out = self.calculate_amm_output(
                    trade_size,
                    buy_dex.reserve_in,
                    buy_dex.reserve_out,
                    300,
                );

                let sell_amount_out = self.calculate_amm_output(
                    buy_amount_out,
                    sell_dex.reserve_in,
                    sell_dex.reserve_out,
                    300,
                );

                if sell_amount_out <= trade_size {
                    continue; // 수익 없음
                }

                let gross_profit = sell_amount_out - trade_size;
                let gas_cost = U256::from(buy_dex.gas_estimate + sell_dex.gas_estimate) * 
                               U256::from(20_000_000_000u64); // 20 gwei

                if gross_profit <= gas_cost {
                    continue; // 가스비를 고려하면 수익 없음
                }

                let net_profit = gross_profit - gas_cost;

                // Opportunity 생성
                let opportunity = Opportunity::new(
                    OpportunityType::Arbitrage,
                    StrategyType::Arbitrage,
                    net_profit,
                    0.8, // 80% 신뢰도
                    buy_dex.gas_estimate + sell_dex.gas_estimate,
                    0, // 만료 블록은 나중에 설정
                    OpportunityDetails::Arbitrage(ArbitrageDetails {
                        token_in,
                        token_out,
                        amount_in: trade_size,
                        amount_out: buy_amount_out,
                        dex_path: vec![buy_dex.dex.clone(), sell_dex.dex.clone()],
                        price_impact: self.calculate_price_impact(trade_size, buy_dex.reserve_in, buy_dex.reserve_out),
                    }),
                );

                opportunities.push(opportunity);
            }
        }

        Ok(opportunities)
    }

    /// 최적 거래 크기 계산
    async fn calculate_optimal_trade_size(&self, buy_dex: &DexPrice, sell_dex: &DexPrice) -> Result<U256> {
        let min_trade_size = ethers::utils::parse_ether("0.1").unwrap(); // 0.1 ETH
        let max_trade_size = ethers::utils::parse_ether("10").unwrap(); // 10 ETH
        
        // 유동성의 1% 제한
        let max_trade_by_liquidity = buy_dex.reserve_in / 100;
        let max_trade_by_sell_liquidity = sell_dex.reserve_out / 100;
        
        let liquidity_constraint = max_trade_by_liquidity.min(max_trade_by_sell_liquidity);
        
        // 설정된 최대 거래 크기와 비교
        let config_max_size = ethers::utils::parse_ether(&self.config.strategies.arbitrage.max_trade_size)?;
        let max_trade_size = max_trade_size.min(config_max_size).min(liquidity_constraint);
        
        if max_trade_size < min_trade_size {
            return Ok(U256::zero());
        }
        
        Ok(max_trade_size)
    }

    /// 가격 영향 계산
    fn calculate_price_impact(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> f64 {
        let amount_in_f64 = amount_in.as_u128() as f64;
        let reserve_in_f64 = reserve_in.as_u128() as f64;
        
        if reserve_in_f64 == 0.0 {
            return 0.0;
        }
        
        amount_in_f64 / (reserve_in_f64 + amount_in_f64)
    }
}

/// DEX 가격 정보
#[derive(Debug, Clone)]
struct DexPrice {
    dex: String,
    token_in: H160,
    token_out: H160,
    amount_in: U256,
    amount_out: U256,
    price: f64,
    liquidity: U256,
    reserve_in: U256,
    reserve_out: U256,
    gas_estimate: u64,
}

#[async_trait]
impl Strategy for MempoolArbitrageStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Arbitrage
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled && self.config.strategies.arbitrage.enabled
    }

    async fn start(&mut self) -> Result<()> {
        info!("🔄 멤풀 기반 차익거래 전략 시작");
        self.is_enabled = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("⏹️ 멤풀 기반 차익거래 전략 중지");
        self.is_enabled = false;
        Ok(())
    }

    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        if !self.is_enabled() {
            return Ok(Vec::new());
        }

        debug!("🔍 트랜잭션 분석 중: {}", transaction.hash);
        self.find_arbitrage_opportunities(transaction).await
    }

    async fn validate_opportunity(&self, opportunity: &Opportunity) -> Result<bool> {
        // 최소 수익 임계값 확인
        let min_profit = ethers::utils::parse_ether(&self.config.strategies.arbitrage.min_profit_threshold)?;
        if opportunity.expected_profit < min_profit {
            return Ok(false);
        }

        // 최대 거래 크기 확인
        if let OpportunityDetails::Arbitrage(details) = &opportunity.details {
            let max_trade_size = ethers::utils::parse_ether(&self.config.strategies.arbitrage.max_trade_size)?;
            let amount_in = details.amount_in.parse::<U256>()?;
            if amount_in > max_trade_size {
                return Ok(false);
            }

            // 가격 영향 확인
            if details.price_impact > self.config.strategies.arbitrage.max_price_impact {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle> {
        let current_block = self.provider.get_block_number().await?.as_u64();
        
        // 차익거래 번들 생성 (실제 구현에서는 플래시론 사용)
        let mut transactions = Vec::new();
        
        if let OpportunityDetails::Arbitrage(details) = &opportunity.details {
            // 1. 첫 번째 DEX에서 구매
            let buy_tx = self.create_buy_transaction(details).await?;
            transactions.push(buy_tx);
            
            // 2. 두 번째 DEX에서 판매
            let sell_tx = self.create_sell_transaction(details).await?;
            transactions.push(sell_tx);
        }
        
        let bundle = Bundle::new(
            transactions,
            current_block + 1,
            opportunity.expected_profit,
            opportunity.gas_estimate,
            StrategyType::Arbitrage,
        );

        Ok(bundle)
    }
}

impl MempoolArbitrageStrategy {
    /// 구매 트랜잭션 생성
    async fn create_buy_transaction(&self, details: &ArbitrageDetails) -> Result<Transaction> {
        // 실제 구현에서는 DEX 라우터를 통한 스왑 트랜잭션 생성
        Ok(Transaction {
            hash: H256::zero(), // 서명 시 설정됨
            from: "0x742d35Cc6570000000000000000000000000004".parse().unwrap(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()), // Uniswap V2 Router
            value: details.amount_in.parse::<U256>()?,
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            gas_limit: U256::from(200_000u64),
            data: vec![0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            nonce: 0, // 지갑에서 설정됨
            timestamp: chrono::Utc::now(),
            block_number: None,
        })
    }

    /// 판매 트랜잭션 생성
    async fn create_sell_transaction(&self, details: &ArbitrageDetails) -> Result<Transaction> {
        Ok(Transaction {
            hash: H256::zero(),
            from: "0x742d35Cc6570000000000000000000000000004".parse().unwrap(),
            to: Some("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap()), // SushiSwap Router
            value: U256::zero(), // 토큰 스왑이므로 ETH 값 없음
            gas_price: U256::from(20_000_000_000u64),
            gas_limit: U256::from(200_000u64),
            data: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::{H160, H256, U256};

    fn create_test_config() -> Arc<Config> {
        let mut config = Config::default();
        config.strategies.arbitrage.enabled = true;
        config.strategies.arbitrage.min_profit_threshold = "0.01".to_string();
        config.strategies.arbitrage.max_trade_size = "10.0".to_string();
        config.strategies.arbitrage.max_price_impact = 0.05;
        Arc::new(config)
    }

    fn create_test_provider() -> Arc<Provider<Ws>> {
        // 테스트용 더미 프로바이더 (실제 연결 없음)
        Arc::new(Provider::new(Ws::connect("wss://dummy").await.unwrap()))
    }

    #[test]
    fn test_amm_output_calculation() {
        let config = create_test_config();
        let provider = create_test_provider();
        
        let strategy = tokio::runtime::Runtime::new().unwrap().block_on(async {
            MempoolArbitrageStrategy::new(config, provider).await
        }).unwrap();
        
        let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let reserve_in = U256::from(100_000_000_000_000_000_000u128); // 100 ETH
        let reserve_out = U256::from(200_000_000_000u128); // 200,000 USDC
        
        let output = strategy.calculate_amm_output(amount_in, reserve_in, reserve_out, 300);
        
        // 출력이 0보다 크고 입력보다 작아야 함
        assert!(output > U256::zero());
        assert!(output < amount_in);
    }

    #[test]
    fn test_price_impact_calculation() {
        let config = create_test_config();
        let provider = create_test_provider();
        
        let strategy = tokio::runtime::Runtime::new().unwrap().block_on(async {
            MempoolArbitrageStrategy::new(config, provider).await
        }).unwrap();
        
        let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let reserve_in = U256::from(100_000_000_000_000_000_000u128); // 100 ETH
        
        let impact = strategy.calculate_price_impact(amount_in, reserve_in, U256::zero());
        
        // 가격 영향이 0과 1 사이여야 함
        assert!(impact >= 0.0);
        assert!(impact <= 1.0);
    }

    #[test]
    fn test_dex_swap_detection() {
        let config = create_test_config();
        let provider = create_test_provider();
        
        let strategy = tokio::runtime::Runtime::new().unwrap().block_on(async {
            MempoolArbitrageStrategy::new(config, provider).await
        }).unwrap();
        
        // Uniswap V2 스왑 트랜잭션
        let swap_tx = Transaction {
            hash: H256::zero(),
            from: "0x742d35Cc6570000000000000000000000000001".parse().unwrap(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()),
            value: U256::from(1_000_000_000_000_000_000u128),
            gas_price: U256::from(20_000_000_000u64),
            gas_limit: U256::from(200_000u64),
            data: vec![0x7f, 0xf3, 0x6a, 0xb5, 0x00, 0x00, 0x00, 0x00],
            nonce: 1,
            timestamp: chrono::Utc::now(),
            block_number: Some(1000),
        };
        
        assert!(strategy.is_dex_swap_transaction(&swap_tx));
        
        // 일반 ETH 전송
        let eth_tx = Transaction {
            hash: H256::zero(),
            from: "0x742d35Cc6570000000000000000000000000001".parse().unwrap(),
            to: Some("0x742d35Cc6570000000000000000000000000002".parse().unwrap()),
            value: U256::from(1_000_000_000_000_000_000u128),
            gas_price: U256::from(20_000_000_000u64),
            gas_limit: U256::from(21_000u64),
            data: vec![],
            nonce: 2,
            timestamp: chrono::Utc::now(),
            block_number: Some(1001),
        };
        
        assert!(!strategy.is_dex_swap_transaction(&eth_tx));
    }
}
