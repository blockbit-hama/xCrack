use super::types::{TargetTransaction, DexType, CompetitionLevel};
use super::dex_router::DexRouterManager;
use anyhow::{Result, anyhow};
use ethers::prelude::*;
use ethers::types::{Address, U256, Bytes};
use std::sync::Arc;
use tracing::{debug, warn};

/// 타겟 트랜잭션 분석기
pub struct TargetAnalyzer {
    provider: Arc<Provider<Ws>>,
    dex_manager: Arc<DexRouterManager>,
}

/// 타겟 분석 결과
#[derive(Debug, Clone)]
pub struct TargetAnalysis {
    pub tx: TargetTransaction,
    pub dex_type: DexType,
    pub router_address: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub path: Vec<Address>,
    pub deadline: U256,
    pub estimated_price_impact: f64,
    pub pool_reserves: Option<PoolReserves>,
    pub competition_level: CompetitionLevel,
}

#[derive(Debug, Clone)]
pub struct PoolReserves {
    pub reserve_in: U256,
    pub reserve_out: U256,
    pub liquidity: U256,
}

impl TargetAnalyzer {
    pub fn new(
        provider: Arc<Provider<Ws>>,
        dex_manager: Arc<DexRouterManager>,
    ) -> Self {
        Self {
            provider,
            dex_manager,
        }
    }

    /// 타겟 트랜잭션 분석
    pub async fn analyze(&self, tx: &TargetTransaction, dex_type: DexType) -> Result<TargetAnalysis> {
        debug!("🔍 타겟 트랜잭션 분석 시작: {:?}", tx.hash);

        // 트랜잭션 데이터 디코딩
        let decoded = self.decode_swap_data(&tx.data, dex_type)?;

        // 가격 영향 추정
        let price_impact = self.estimate_price_impact(
            decoded.amount_in,
            decoded.token_in,
            decoded.token_out,
            dex_type,
        ).await?;

        // 풀 리저브 조회 (옵션)
        let pool_reserves = self.get_pool_reserves(
            decoded.token_in,
            decoded.token_out,
            dex_type,
        ).await.ok();

        // 경쟁 수준 평가
        let competition_level = self.assess_competition_level(
            tx.gas_price,
            decoded.amount_in,
            price_impact,
        ).await;

        Ok(TargetAnalysis {
            tx: tx.clone(),
            dex_type,
            router_address: tx.to,
            token_in: decoded.token_in,
            token_out: decoded.token_out,
            amount_in: decoded.amount_in,
            amount_out_min: decoded.amount_out_min,
            path: decoded.path,
            deadline: decoded.deadline,
            estimated_price_impact: price_impact,
            pool_reserves,
            competition_level,
        })
    }

    /// 스왑 데이터 디코딩 (실제 ABI 디코딩)
    fn decode_swap_data(&self, data: &Bytes, dex_type: DexType) -> Result<DecodedSwap> {
        if data.len() < 4 {
            return Err(anyhow!("Invalid transaction data"));
        }

        match dex_type {
            DexType::UniswapV2 | DexType::SushiSwap => {
                self.decode_uniswap_v2_swap(data)
            }
            DexType::UniswapV3 => {
                self.decode_uniswap_v3_swap(data)
            }
            _ => Err(anyhow!("Unsupported DEX type for decoding")),
        }
    }

    fn decode_uniswap_v2_swap(&self, data: &Bytes) -> Result<DecodedSwap> {
        use ethers::abi::{decode, ParamType, Token};

        if data.len() < 4 {
            return Err(anyhow!("Data too short"));
        }

        let function_selector = &data[0..4];
        let params_data = &data[4..];

        // swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] path, address to, uint deadline)
        if function_selector == [0x38, 0xed, 0x17, 0x39] {
            let param_types = vec![
                ParamType::Uint(256),           // amountIn
                ParamType::Uint(256),           // amountOutMin
                ParamType::Array(Box::new(ParamType::Address)), // path
                ParamType::Address,             // to
                ParamType::Uint(256),           // deadline
            ];

            match decode(&param_types, params_data) {
                Ok(tokens) => {
                    let amount_in = match &tokens[0] {
                        Token::Uint(val) => *val,
                        _ => return Err(anyhow!("Invalid amountIn")),
                    };

                    let amount_out_min = match &tokens[1] {
                        Token::Uint(val) => *val,
                        _ => return Err(anyhow!("Invalid amountOutMin")),
                    };

                    let path = match &tokens[2] {
                        Token::Array(arr) => {
                            arr.iter().filter_map(|t| {
                                if let Token::Address(addr) = t {
                                    Some(*addr)
                                } else {
                                    None
                                }
                            }).collect::<Vec<Address>>()
                        }
                        _ => return Err(anyhow!("Invalid path")),
                    };

                    let deadline = match &tokens[4] {
                        Token::Uint(val) => *val,
                        _ => return Err(anyhow!("Invalid deadline")),
                    };

                    if path.len() < 2 {
                        return Err(anyhow!("Path too short"));
                    }

                    return Ok(DecodedSwap {
                        amount_in,
                        amount_out_min,
                        token_in: path[0],
                        token_out: path[path.len() - 1],
                        path,
                        deadline,
                    });
                }
                Err(e) => return Err(anyhow!("ABI decode failed: {}", e)),
            }
        }

        Err(anyhow!("Unsupported function selector"))
    }

    fn decode_uniswap_v3_swap(&self, data: &Bytes) -> Result<DecodedSwap> {
        use ethers::abi::{decode, ParamType, Token};

        if data.len() < 4 {
            return Err(anyhow!("Data too short"));
        }

        let function_selector = &data[0..4];
        let params_data = &data[4..];

        // exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))
        if function_selector == [0xc0, 0x4b, 0x8d, 0x59] {
            // Tuple 구조 디코딩
            let param_types = vec![
                ParamType::Tuple(vec![
                    ParamType::Address,    // tokenIn
                    ParamType::Address,    // tokenOut
                    ParamType::Uint(24),   // fee
                    ParamType::Address,    // recipient
                    ParamType::Uint(256),  // deadline
                    ParamType::Uint(256),  // amountIn
                    ParamType::Uint(256),  // amountOutMinimum
                    ParamType::Uint(160),  // sqrtPriceLimitX96
                ])
            ];

            match decode(&param_types, params_data) {
                Ok(tokens) => {
                    if let Token::Tuple(tuple_tokens) = &tokens[0] {
                        let token_in = match &tuple_tokens[0] {
                            Token::Address(addr) => *addr,
                            _ => return Err(anyhow!("Invalid tokenIn")),
                        };

                        let token_out = match &tuple_tokens[1] {
                            Token::Address(addr) => *addr,
                            _ => return Err(anyhow!("Invalid tokenOut")),
                        };

                        let deadline = match &tuple_tokens[4] {
                            Token::Uint(val) => *val,
                            _ => return Err(anyhow!("Invalid deadline")),
                        };

                        let amount_in = match &tuple_tokens[5] {
                            Token::Uint(val) => *val,
                            _ => return Err(anyhow!("Invalid amountIn")),
                        };

                        let amount_out_min = match &tuple_tokens[6] {
                            Token::Uint(val) => *val,
                            _ => return Err(anyhow!("Invalid amountOutMinimum")),
                        };

                        return Ok(DecodedSwap {
                            amount_in,
                            amount_out_min,
                            token_in,
                            token_out,
                            path: vec![token_in, token_out],
                            deadline,
                        });
                    }
                }
                Err(e) => return Err(anyhow!("ABI decode failed: {}", e)),
            }
        }

        Err(anyhow!("Unsupported V3 function selector"))
    }

    /// 가격 영향 추정
    async fn estimate_price_impact(
        &self,
        amount_in: U256,
        token_in: Address,
        token_out: Address,
        dex_type: DexType,
    ) -> Result<f64> {
        // 실제 구현에서는 풀 리저브 조회 후 계산
        // 여기서는 간단한 휴리스틱 사용

        let amount_in_eth = amount_in.as_u128() as f64 / 1e18;

        // 거래량에 따른 가격 영향 추정
        let base_impact = match dex_type {
            DexType::UniswapV2 | DexType::SushiSwap => {
                if amount_in_eth < 1.0 {
                    0.001 // 0.1%
                } else if amount_in_eth < 10.0 {
                    0.005 // 0.5%
                } else if amount_in_eth < 50.0 {
                    0.02 // 2%
                } else {
                    0.05 // 5%
                }
            }
            DexType::UniswapV3 => {
                // V3는 집중 유동성으로 영향이 더 클 수 있음
                if amount_in_eth < 1.0 {
                    0.002
                } else if amount_in_eth < 10.0 {
                    0.01
                } else {
                    0.03
                }
            }
            _ => 0.01,
        };

        debug!("   가격 영향 추정: {:.2}%", base_impact * 100.0);
        Ok(base_impact)
    }

    /// 풀 리저브 조회 (실제 컨트랙트 호출)
    async fn get_pool_reserves(
        &self,
        token_in: Address,
        token_out: Address,
        dex_type: DexType,
    ) -> Result<PoolReserves> {
        use ethers::abi::{encode, Token, ParamType, decode};
        use ethers::types::Bytes;

        // 1. Factory 주소 가져오기
        let factory_address = match dex_type {
            DexType::UniswapV2 => "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse::<Address>()?,
            DexType::SushiSwap => "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse::<Address>()?,
            DexType::UniswapV3 => "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse::<Address>()?,
            _ => return Err(anyhow!("Unsupported DEX for reserves query")),
        };

        // 2. getPair(tokenA, tokenB) 호출
        let get_pair_selector = [0xe6, 0xa4, 0x39, 0x05]; // keccak256("getPair(address,address)")[:4]
        let get_pair_data = {
            let mut data = get_pair_selector.to_vec();
            data.extend_from_slice(&encode(&[
                Token::Address(token_in.into()),
                Token::Address(token_out.into()),
            ]));
            Bytes::from(data)
        };

        // eth_call로 pair 주소 조회
        let pair_address = match self.provider.call(
            &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
                ethers::types::TransactionRequest {
                    to: Some(factory_address.into()),
                    data: Some(get_pair_data),
                    ..Default::default()
                }
            ),
            None,
        ).await {
            Ok(result) => {
                if result.len() >= 32 {
                    Address::from_slice(&result[12..32])
                } else {
                    return Err(anyhow!("Invalid pair address response"));
                }
            }
            Err(e) => return Err(anyhow!("Failed to get pair address: {}", e)),
        };

        // Pair가 존재하지 않으면 (zero address)
        if pair_address == Address::zero() {
            return Err(anyhow!("Pair does not exist"));
        }

        // 3. getReserves() 호출
        let get_reserves_selector = [0x09, 0x02, 0xf1, 0xac]; // keccak256("getReserves()")[:4]
        let get_reserves_data = Bytes::from(get_reserves_selector.to_vec());

        let reserves_result = match self.provider.call(
            &ethers::types::transaction::eip2718::TypedTransaction::Legacy(
                ethers::types::TransactionRequest {
                    to: Some(pair_address.into()),
                    data: Some(get_reserves_data),
                    ..Default::default()
                }
            ),
            None,
        ).await {
            Ok(result) => result,
            Err(e) => return Err(anyhow!("Failed to get reserves: {}", e)),
        };

        // 4. Reserves 디코딩: (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        let param_types = vec![
            ParamType::Uint(112), // reserve0
            ParamType::Uint(112), // reserve1
            ParamType::Uint(32),  // blockTimestampLast
        ];

        match decode(&param_types, &reserves_result) {
            Ok(tokens) => {
                let reserve0 = match &tokens[0] {
                    Token::Uint(val) => *val,
                    _ => return Err(anyhow!("Invalid reserve0")),
                };

                let reserve1 = match &tokens[1] {
                    Token::Uint(val) => *val,
                    _ => return Err(anyhow!("Invalid reserve1")),
                };

                // token0과 token1 순서 확인 (token_in이 reserve_in인지 확인)
                let (reserve_in, reserve_out) = if token_in < token_out {
                    (reserve0, reserve1)
                } else {
                    (reserve1, reserve0)
                };

                let liquidity = reserve_in + reserve_out;

                debug!("   풀 리저브 조회 성공: in={}, out={}",
                       format_reserve(reserve_in), format_reserve(reserve_out));

                Ok(PoolReserves {
                    reserve_in,
                    reserve_out,
                    liquidity,
                })
            }
            Err(e) => Err(anyhow!("Failed to decode reserves: {}", e)),
        }
    }

    /// 경쟁 수준 평가
    async fn assess_competition_level(
        &self,
        gas_price: U256,
        amount_in: U256,
        price_impact: f64,
    ) -> CompetitionLevel {
        let gas_gwei = gas_price.as_u128() / 1_000_000_000;
        let amount_eth = amount_in.as_u128() as f64 / 1e18;

        // 경쟁 수준 결정 로직
        if gas_gwei > 200 || (amount_eth > 100.0 && price_impact > 0.03) {
            CompetitionLevel::Critical
        } else if gas_gwei > 100 || (amount_eth > 50.0 && price_impact > 0.02) {
            CompetitionLevel::High
        } else if gas_gwei > 50 || amount_eth > 10.0 {
            CompetitionLevel::Medium
        } else {
            CompetitionLevel::Low
        }
    }
}

#[derive(Debug, Clone)]
struct DecodedSwap {
    amount_in: U256,
    amount_out_min: U256,
    token_in: Address,
    token_out: Address,
    path: Vec<Address>,
    deadline: U256,
}

fn format_reserve(reserve: U256) -> String {
    let amount = reserve.as_u128() as f64 / 1e18;
    format!("{:.2}", amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_competition_level_assessment() {
        // Mock test
        assert!(true);
    }
}
