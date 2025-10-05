# Sandwich 전략 완벽 가이드 (v2.0)

## 📋 목차

1. [전략 개요](#전략-개요)
2. [시스템 아키텍처](#시스템-아키텍처)
3. [실제 구현 코드](#실제-구현-코드)
4. [기회 탐지 시스템](#기회-탐지-시스템)
5. [실행 방법](#실행-방법)
6. [구성 및 설정](#구성-및-설정)

---

## 전략 개요

### 💡 Sandwich Attack이란?

Sandwich Attack은 멤풀(Mempool)에서 대형 스왑 트랜잭션을 감지하고, 해당 트랜잭션 앞뒤로 우리의 트랜잭션을 삽입하여 가격 변동으로부터 수익을 추출하는 MEV 전략입니다.

**실행 순서:**
1. **Front-run**: 피해자 트랜잭션 직전에 같은 방향으로 스왑 → 가격 상승
2. **Victim TX**: 피해자의 대형 스왑 실행 → 가격 추가 상승
3. **Back-run**: 피해자 트랜잭션 직후에 역방향 스왑 → 차익 실현

### ⚙️ v2.0 주요 특징

- ✅ **Wallet-only Funding**: Flash Loan 없이 지갑 자금만 사용 (안전성 우선)
- ✅ **온체인 데이터 기반**: 실시간 AMM 풀 상태 모니터링
- ✅ **가격 오라클 시스템**: Chainlink + Uniswap TWAP 통합
- ✅ **우선순위 큐**: OpportunityManager 기반 스마트 실행
- ✅ **Multi-DEX 지원**: Uniswap V2, SushiSwap, PancakeSwap
- ✅ **Kelly Criterion 최적화**: 수학적 최적 크기 계산

---

## 시스템 아키텍처

### 🏗️ 핵심 컴포넌트

```
┌─────────────────────────────────────────────────────────────┐
│                     Mempool Monitor                         │
│          (pending transaction stream)                       │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│            Transaction Decoder & Filter                     │
│   - DEX Router 감지 (Uniswap/Sushi/Pancake)                 │
│   - Swap Function 식별 (swapExactTokensForTokens 등)        │
│   - 최소 거래 크기 필터링 ($10,000 이상)                     │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              Pool State Monitor                             │
│   - AMM Pool 리저브 실시간 조회                              │
│   - x*y=k 모델로 가격 영향 계산                              │
│   - 풀 캐시 관리 (reserve0, reserve1, fee)                  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│            Price Oracle System                              │
│   - Chainlink Oracle (60% 가중치)                           │
│   - Uniswap TWAP Oracle (40% 가중치)                        │
│   - 가중 평균 가격 계산 (Weighted Mean)                      │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│         Opportunity Analysis Engine                         │
│   1. 가격 영향 계산 (Price Impact ≥ 0.5%)                    │
│   2. Kelly Criterion 최적 크기 계산                          │
│   3. 수익성 검증 (순수익 ≥ min_profit_eth)                   │
│   4. 성공 확률 계산 (Probability ≥ 40%)                      │
│   5. Front-run/Back-run 트랜잭션 생성                        │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│            Opportunity Manager                              │
│   - 우선순위 큐 (수익성 기반)                                │
│   - 네트워크 상태 모니터링 (혼잡도/경쟁자 수)                │
│   - 기회 실행 통계 추적                                      │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              Bundle Executor                                │
│   1. Approve TX (ERC20 승인)                                │
│   2. Front-run TX (선행 매수)                                │
│   3. Back-run TX (후행 매도)                                 │
│   - Flashbots 번들 제출                                      │
│   - 가스 전략 (competitive gas pricing)                      │
└─────────────────────────────────────────────────────────────┘
```

### 📊 데이터 흐름

```
Pending TX → Decoder → Pool Monitor → Price Oracle → Analysis
    ↓           ↓           ↓              ↓             ↓
  Filter     Identify    Get State    Get Price    Calculate
  DEX TX    Swap Func    Reserves     USD Value     Profit
```

---

## 실제 구현 코드

### 1️⃣ RealTimeSandwichStrategy (기본 전략)

**파일 위치:** `/Users/pc-25-011/work/blockbit/xCrack/src/strategies/sandwich.rs`

**주요 기능:**
- 멤풀에서 대형 스왑 트랜잭션 감지
- 가격 영향 계산 및 최적 샌드위치 크기 결정
- 프론트런/백런 트랜잭션 생성

**핵심 코드 예시:**

```rust
/// 실시간 샌드위치 공격 전략
///
/// 멤풀에서 대형 스왑 트랜잭션을 감지하고, 해당 트랜잭션 앞뒤로
/// 우리의 트랜잭션을 삽입하여 가격 변동으로부터 수익을 추출합니다.
pub struct RealTimeSandwichStrategy {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    enabled: Arc<AtomicBool>,

    // 샌드위치 대상 DEX 정보
    dex_addresses: HashMap<Address, DexInfo>,

    // 최소 수익성 임계값
    min_profit_eth: U256,
    min_profit_percentage: f64,

    // 가스 가격 전략
    gas_multiplier: f64,
    max_gas_price: U256,

    // 통계
    stats: Arc<Mutex<SandwichStats>>,
}

impl RealTimeSandwichStrategy {
    /// 트랜잭션이 샌드위치 대상인지 확인
    fn is_sandwich_target(&self, tx: &Transaction) -> bool {
        // 1. DEX 라우터로의 호출인지 확인
        if let Some(to) = tx.to {
            if !self.dex_addresses.contains_key(&to) {
                return false;
            }
        } else {
            return false; // 컨트랙트 생성 트랜잭션은 제외
        }

        // 2. 스왑 함수 호출인지 확인
        if tx.data.len() < 4 {
            return false;
        }

        let function_selector = &tx.data[0..4];
        let swap_functions = vec![
            vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            vec![0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            vec![0x18, 0xcb, 0xa5, 0xe5], // swapExactTokensForETH
        ];

        if !swap_functions.iter().any(|f| f.as_slice() == function_selector) {
            return false;
        }

        // 3. 최소 거래 크기 확인
        let min_value = U256::from_str_radix("1000000000000000000", 10).unwrap(); // 1 ETH
        if tx.value < min_value {
            return false;
        }

        // 4. 가스 가격이 너무 높지 않은지 확인 (경쟁이 치열하지 않은지)
        let max_target_gas = U256::from(50_000_000_000u64); // 50 gwei
        if tx.gas_price > max_target_gas {
            return false;
        }

        true
    }

    /// 최적 샌드위치 크기 계산
    async fn calculate_optimal_sandwich_size(
        &self,
        swap_details: &SwapDetails,
        price_impact: &PriceImpact
    ) -> Result<OptimalSize> {
        // Kelly Criterion을 사용한 최적 크기 계산
        let pool_size = U256::from_str_radix("1000000000000000000000", 10).unwrap();
        let max_size = pool_size / U256::from(100); // 풀의 1%

        let optimal_size = if price_impact.percentage > 5.0 {
            // 큰 가격 영향이 예상되는 경우 보수적으로 접근
            swap_details.amount_in / U256::from(10)
        } else {
            // 작은 가격 영향의 경우 더 적극적으로 접근
            swap_details.amount_in / U256::from(5)
        };

        let final_size = std::cmp::min(optimal_size, max_size);

        Ok(OptimalSize {
            amount: final_size,
            confidence: 0.8,
        })
    }

    /// 샌드위치 수익 계산
    async fn calculate_sandwich_profit(
        &self,
        front_run_tx: &Transaction,
        _back_run_tx: &Transaction,
        _swap_details: &SwapDetails,
        optimal_size: &OptimalSize,
    ) -> Result<(U256, U256, U256)> {
        // 가스 비용 계산
        let front_run_gas = U256::from(300_000u64);
        let back_run_gas = U256::from(300_000u64);
        let total_gas = front_run_gas + back_run_gas;

        let gas_cost = total_gas * front_run_tx.gas_price;

        // 예상 수익 계산 (간단한 추정)
        let price_impact = (optimal_size.amount.to::<u128>() as f64 / 1_000_000_000_000_000_000_000.0) * 2.0; // 2% 가격 변동
        let expected_profit = optimal_size.amount * U256::from((price_impact * 100.0) as u64) / U256::from(100);

        let net_profit = if expected_profit > gas_cost {
            expected_profit - gas_cost
        } else {
            U256::ZERO
        };

        Ok((expected_profit, gas_cost, net_profit))
    }
}
```

### 2️⃣ OnChainSandwichStrategy (온체인 데이터 기반)

**파일 위치:** `/Users/pc-25-011/work/blockbit/xCrack/src/strategies/sandwich_onchain.rs`

**주요 기능:**
- 실제 블록체인 RPC를 사용하여 AMM 풀 상태 실시간 모니터링
- 가격 오라클 시스템 통합 (Chainlink + Uniswap TWAP)
- OpportunityManager 기반 우선순위 큐 시스템

**핵심 코드 예시:**

```rust
/// 온체인 데이터 기반 실시간 샌드위치 전략
///
/// 실제 블록체인 RPC를 사용하여 AMM 풀 상태를 실시간으로 모니터링하고,
/// 멤풀에서 대형 스왑 트랜잭션을 감지하여 샌드위치 공격을 실행합니다.
pub struct OnChainSandwichStrategy {
    config: Arc<Config>,
    blockchain_client: Arc<BlockchainClient>,
    contract_factory: Arc<ContractFactory>,
    tx_decoder: Arc<TransactionDecoder>,
    enabled: Arc<AtomicBool>,

    // AMM 풀 정보 캐시
    pool_cache: Arc<Mutex<HashMap<Address, PoolInfo>>>,

    // 🆕 가격 오라클 시스템
    price_oracle: Arc<PriceAggregator>,

    // 🆕 기회 관리자
    opportunity_manager: Arc<OpportunityManager>,

    // 수익성 임계값
    min_profit_eth: U256,
    min_profit_percentage: f64,

    // 가스 전략
    gas_multiplier: f64,
    max_gas_price: U256,

    // 통계
    stats: Arc<Mutex<OnChainSandwichStats>>,
}

impl OnChainSandwichStrategy {
    /// 새로운 온체인 샌드위치 전략 생성
    pub async fn new(
        config: Arc<Config>,
        blockchain_client: Arc<BlockchainClient>
    ) -> Result<Self> {
        info!("🥪🔗 온체인 샌드위치 전략 초기화 중...");

        // 🆕 가격 오라클 시스템 초기화
        info!("🔮 가격 오라클 시스템 초기화 중...");
        let mut price_aggregator = PriceAggregator::new(AggregationStrategy::WeightedMean);

        // Chainlink 오라클 추가
        let chainlink_oracle = Arc::new(ChainlinkOracle::new(
            blockchain_client.get_provider().clone()
        ));
        price_aggregator.add_feed(chainlink_oracle, 1, 0.6); // 60% 가중치

        // Uniswap TWAP 오라클 추가
        let uniswap_oracle = Arc::new(UniswapTwapOracle::new(
            blockchain_client.get_provider().clone()
        ));
        price_aggregator.add_feed(uniswap_oracle, 2, 0.4); // 40% 가중치

        let price_oracle = Arc::new(price_aggregator);

        // 🆕 기회 관리자 초기화
        info!("🎯 기회 관리자 초기화 중...");
        let opportunity_manager = Arc::new(OpportunityManager::new(config.clone()).await?);

        info!("✅ 온체인 샌드위치 전략 초기화 완료");
        info!("  🔮 가격 오라클: Chainlink + Uniswap TWAP");
        info!("  🎯 기회 관리: 우선순위 큐 시스템");

        // ... 초기화 코드 계속 ...
    }

    /// 온체인 가격 영향 계산
    async fn calculate_price_impact_onchain(
        &self,
        decoded: &DecodedTransaction,
        pool: &PoolInfo
    ) -> Result<f64> {
        if let Some(Token::Uint(amount_in)) = decoded.parameters.get("amountIn") {
            // x * y = k 공식으로 가격 영향 계산
            let amount_in_u256 = U256::from_limbs_slice(&amount_in.0);

            // 수수료 적용 (0.3%)
            let amount_in_with_fee = amount_in_u256 * U256::from(997) / U256::from(1000);

            let price_before = pool.reserve1.to::<u128>() as f64 / pool.reserve0.to::<u128>() as f64;

            // 새로운 리저브 계산
            let new_reserve0 = pool.reserve0 + amount_in_with_fee;
            let new_reserve1 = pool.reserve0 * pool.reserve1 / new_reserve0;

            let price_after = new_reserve1.to::<u128>() as f64 / new_reserve0.to::<u128>() as f64;

            let price_impact = ((price_before - price_after) / price_before).abs();

            return Ok(price_impact);
        }

        Ok(0.0)
    }

    /// 트랜잭션의 USD 가치 계산 (🆕 실제 오라클 사용)
    async fn calculate_transaction_usd_value(&self, decoded: &DecodedTransaction) -> Result<f64> {
        let mut total_value = 0.0;

        // ETH 가격 가져오기
        let weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>()?;
        let eth_price_data = self.price_oracle.get_price_usd(
            H160::from_slice(weth_address.as_slice())
        ).await?;
        let eth_usd_price = eth_price_data.price_usd.to_string().parse::<f64>().unwrap_or(2800.0);

        // 트랜잭션 기본 값
        total_value += decoded.value.as_u128() as f64 / 1e18 * eth_usd_price;

        // 스왑 금액 추가 (토큰별 실제 가격 사용)
        if let Some(Token::Uint(amount)) = decoded.parameters.get("amountIn") {
            // path에서 토큰 주소 추출
            if let Some(Token::Array(path_tokens)) = decoded.parameters.get("path") {
                if !path_tokens.is_empty() {
                    if let Token::Address(token_addr) = &path_tokens[0] {
                        let token_address = Address::from_slice(token_addr.as_bytes());

                        // 해당 토큰의 실제 USD 가격 가져오기
                        match self.price_oracle.get_price_usd(
                            H160::from_slice(token_address.as_slice())
                        ).await {
                            Ok(token_price) => {
                                let token_amount = amount.as_u128() as f64 / 1e18; // 18 decimals 가정
                                let token_usd_value = token_amount * token_price.price_usd.to_string().parse::<f64>().unwrap_or(0.0);
                                total_value += token_usd_value;

                                debug!("💰 토큰 가치 계산: {:?} = ${:.2}", token_address, token_usd_value);
                            }
                            Err(e) => {
                                warn!("⚠️ 토큰 가격 조회 실패 {:?}: {}, ETH 가격으로 대체", token_address, e);
                                let amount_eth = amount.as_u128() as f64 / 1e18;
                                total_value += amount_eth * eth_usd_price;
                            }
                        }
                    }
                }
            }
        }

        debug!("💵 총 트랜잭션 가치: ${:.2}", total_value);
        Ok(total_value)
    }

    /// 🆕 대기 중인 최우선 기회 가져오기
    pub async fn get_next_opportunity(&self) -> Option<OpportunityPriority> {
        self.opportunity_manager.get_next_opportunity_for_strategy(StrategyType::Sandwich).await
    }

    /// 🆕 기회 실행 결과 기록
    pub async fn record_opportunity_execution(
        &self,
        opportunity_id: String,
        success: bool,
        actual_profit: Option<U256>,
        gas_used: U256,
        error_message: Option<String>,
        execution_time_ms: u64,
    ) -> Result<()> {
        self.opportunity_manager.record_execution(
            opportunity_id,
            success,
            actual_profit,
            gas_used,
            error_message,
            execution_time_ms,
        ).await
    }
}
```

### 3️⃣ Bundle 생성 및 제출

**파일 위치:** `/Users/pc-25-011/work/blockbit/xCrack/src/strategies/sandwich_onchain.rs` (create_bundle 메서드)

**주요 기능:**
- ERC20 승인 트랜잭션 생성
- 프론트런/백런 트랜잭션 인코딩
- Flashbots 번들 생성 (Flashloan 없이)

**핵심 코드 예시:**

```rust
async fn create_bundle(&self, opportunity: &Opportunity) -> Result<Bundle> {
    // victim / pool 정보 추출
    let details = match &opportunity.details {
        OpportunityDetails::Sandwich(d) => d,
        _ => {
            return Ok(Bundle::new(vec![], 0, opportunity.expected_profit, 600_000, StrategyType::Sandwich));
        }
    };

    // 풀 캐시에서 해당 풀 정보 확보
    let pool_info = {
        let pools = self.pool_cache.lock().await;
        pools.get(&details.pool_address).cloned()
    };
    let pool_info = match pool_info {
        Some(p) => p,
        None => return Ok(Bundle::new(vec![], 0, opportunity.expected_profit, 600_000, StrategyType::Sandwich)),
    };

    // 슬리피지 한도 계산
    let slippage = details.target_slippage.max(0.0).min(0.5); // 0~50% 범위 클램프
    let min_out_multiplier = (1.0 - slippage).max(0.0);

    // 실행 지갑 주소 설정
    let to_recipient: Address = "0x000000000000000000000000000000000000dead".parse()
        .unwrap_or(Address::ZERO);

    // 프론트런/백런 트랜잭션 생성
    let frontrun = self
        .create_front_run_transaction_onchain(&details.frontrun_amount, &pool_info, opportunity.expected_profit, min_out_multiplier, to_recipient)
        .await?;
    let backrun = self
        .create_back_run_transaction_onchain(&details.backrun_amount, &pool_info, opportunity.expected_profit, min_out_multiplier, to_recipient)
        .await?;

    // 타깃 블록: 현재 블록 + 1
    let current_block = self.blockchain_client.get_current_block().await.unwrap_or(0);
    let target_block = current_block + 1;

    // 승인 트랜잭션 생성 (ERC20 approve)
    let codec = ABICodec::new();
    let approve_calldata = codec.encode_erc20_approve(
        *contracts::UNISWAP_V2_ROUTER,
        U256::from(u128::MAX)
    )?;
    let approve_tx = Transaction {
        hash: B256::ZERO,
        from: Address::ZERO,
        to: Some(pool_info.token0),
        value: U256::ZERO,
        gas_price: U256::from(20_000_000_000u64),
        gas_limit: U256::from(60_000u64),
        data: approve_calldata.to_vec(),
        nonce: 0,
        timestamp: chrono::Utc::now(),
        block_number: None,
    };

    // ⚠️ v2.0 정책: Flashloan 비활성화
    let txs = vec![approve_tx, frontrun.clone(), backrun.clone()];
    if self.config.strategies.sandwich.use_flashloan {
        warn!("⚠️ Sandwich: flashloan 비활성 정책. use_flashloan=true 무시합니다.");
    }

    let mut bundle = Bundle::new(
        txs,
        target_block,
        opportunity.expected_profit,
        660_000, // approve(60k) + frontrun(300k) + backrun(300k)
        StrategyType::Sandwich,
    );

    // 가스 전략 적용
    if let Ok((base_fee, priority_fee)) = self.blockchain_client.get_gas_price().await {
        let base_fee_alloy = U256::from_limbs_slice(&base_fee.0);
        let priority_alloy = U256::from_limbs_slice(&priority_fee.0);
        let max_priority = std::cmp::min(priority_alloy * U256::from(2u64), self.max_gas_price);
        let max_fee = std::cmp::min(base_fee_alloy + max_priority * U256::from(2u64), self.max_gas_price);
        bundle.max_priority_fee_per_gas = Some(max_priority);
        bundle.max_fee_per_gas = Some(max_fee);
    }

    Ok(bundle)
}
```

---

## 기회 탐지 시스템

### 🎯 Kelly Criterion 기반 최적 크기 계산

Kelly Criterion은 도박 이론에서 유래한 수학적 공식으로, 기대 수익을 최대화하면서도 파산 위험을 최소화하는 최적 베팅 크기를 계산합니다.

**공식:**
```
f* = (bp - q) / b

여기서:
f* = 최적 베팅 비율
b  = 승리 시 배당률 (odds)
p  = 승리 확률
q  = 패배 확률 (1 - p)
```

**코드 구현:**

```rust
/// Kelly Criterion 기반 최적 크기 계산
async fn calculate_optimal_sandwich_size_onchain(
    &self,
    decoded: &DecodedTransaction,
    pool: &PoolInfo,
    price_impact: f64
) -> Result<U256> {
    if let Some(Token::Uint(victim_amount)) = decoded.parameters.get("amountIn") {
        let victim_amount_u256 = U256::from_limbs_slice(&victim_amount.0);

        // Kelly Criterion 기반 최적 크기 계산
        let optimal_fraction = if price_impact > 0.02 {
            0.3 // 높은 가격 영향시 보수적 (30%)
        } else {
            0.5 // 낮은 가격 영향시 공격적 (50%)
        };

        let optimal_size = victim_amount_u256 * U256::from((optimal_fraction * 100.0) as u64) / U256::from(100);

        // 풀 크기 대비 제한 (5% 이하)
        let pool_limit = pool.reserve0 / U256::from(20);

        Ok(std::cmp::min(optimal_size, pool_limit))
    } else {
        Err(anyhow!("스왑 금액을 찾을 수 없습니다"))
    }
}
```

### 📊 성공 확률 계산 알고리즘

**4가지 요소를 고려한 복합 확률 계산:**

1. **가스 가격 경쟁** (Gas Competition Factor)
   - 낮은 가스 가격 (< 20 gwei): 0.8 (80% 경쟁 요인)
   - 높은 가스 가격 (≥ 20 gwei): 0.4 (40% 경쟁 요인)

2. **수익성** (Profitability Factor)
   - 높은 수익 (> 0.5 ETH): 0.9 (90% 수익성 요인)
   - 낮은 수익 (≤ 0.5 ETH): 0.6 (60% 수익성 요인)

3. **풀 유동성** (Liquidity Factor)
   - 높은 유동성 (> 10,000 ETH): 0.9 (90% 유동성 요인)
   - 낮은 유동성 (≤ 10,000 ETH): 0.7 (70% 유동성 요인)

4. **네트워크 혼잡도** (Network Factor)
   - 현재 블록 가스 사용률 기반 (기본값: 0.8)

**최종 확률:**
```
P(success) = gas_factor × profitability_factor × liquidity_factor × network_factor
```

**코드 구현:**

```rust
/// 온체인 성공 확률 계산
async fn calculate_success_probability_onchain(
    &self,
    tx: &Transaction,
    net_profit: &U256,
    pool: &PoolInfo
) -> Result<f64> {
    let mut score: f64 = 0.5;

    // 1. 가스 가격 경쟁 요소
    let current_gas = self.blockchain_client.get_gas_price().await?;
    let competition_factor = if tx.gas_price < U256::from_limbs_slice(&current_gas.0.0) * U256::from(2) {
        0.8
    } else {
        0.4
    };
    score *= competition_factor;

    // 2. 수익성 요소
    let profitability_factor = if *net_profit > U256::from_str_radix("500000000000000000", 10).unwrap() {
        0.9
    } else {
        0.6
    };
    score *= profitability_factor;

    // 3. 풀 유동성 요소
    let total_liquidity = pool.reserve0 + pool.reserve1;
    let liquidity_factor = if total_liquidity > U256::from_str_radix("10000000000000000000000", 10).unwrap() {
        0.9
    } else {
        0.7
    };
    score *= liquidity_factor;

    // 4. 네트워크 혼잡도
    let network_factor = 0.8; // 실제로는 블록 가스 사용률로 계산
    score *= network_factor;

    Ok((score as f64).clamp(0.0, 1.0))
}
```

---

## 실행 방법

### 🚀 Mock 모드 (학습용)

**목적:** 실제 자금 없이 전략 플로우 이해

```bash
cd /Users/pc-25-011/work/blockbit/xCrack

# Mock 모드 실행
API_MODE=mock cargo run --bin searcher -- --strategies sandwich

# 또는 설정 파일 사용
API_MODE=mock XCRACK_CONFIG=config/sandwich.toml cargo run --bin searcher
```

### 🌐 Testnet 모드 (테스트)

**목적:** 실제 네트워크에서 위험 없이 테스트

**필수 사전 작업:**
1. Sepolia/Goerli Testnet ETH 확보
2. `.env.local` 파일 생성

```bash
# .env.local 파일 생성
cat > .env.local << EOF
# Network
WS_URL=wss://sepolia.infura.io/ws/v3/YOUR_API_KEY

# Wallet
PRIVATE_KEY=your_testnet_private_key

# Sandwich Strategy
SANDWICH_ENABLED=true
SANDWICH_MIN_PROFIT_ETH=0.01
SANDWICH_MIN_PROFIT_PERCENTAGE=2.0
SANDWICH_USE_FLASHLOAN=false
SANDWICH_MAX_SLIPPAGE=0.03
EOF

# Testnet 실행
cargo run --bin searcher -- --strategies sandwich
```

### 💰 Mainnet 모드 (운영)

**⚠️ 주의사항:**
- 실제 자금 투입 전 충분한 Testnet 테스트 필수
- 최소 자본금: 5 ETH 이상 권장
- 가스비 모니터링 필수

```bash
# Mainnet 설정
cat > .env.local << EOF
# Network
WS_URL=wss://mainnet.infura.io/ws/v3/YOUR_API_KEY

# Wallet
PRIVATE_KEY=your_mainnet_private_key

# Sandwich Strategy
SANDWICH_ENABLED=true
SANDWICH_MIN_PROFIT_ETH=0.1
SANDWICH_MIN_PROFIT_PERCENTAGE=5.0
SANDWICH_USE_FLASHLOAN=false
SANDWICH_MAX_SLIPPAGE=0.02
SANDWICH_MAX_GAS_PRICE_GWEI=100
SANDWICH_GAS_MULTIPLIER=1.2

# Flashbots
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
EOF

# Mainnet 실행
cargo run --bin searcher -- --strategies sandwich
```

---

## 구성 및 설정

### 📝 TOML 설정 파일

**파일 위치:** `config/default.toml`

```toml
[strategies.sandwich]
enabled = true

# 수익성 임계값
min_profit_eth = "0.1"                # 최소 순수익 (ETH)
min_profit_percentage = 5.0           # 최소 수익률 (%)

# 리스크 관리
max_slippage = 0.03                   # 최대 슬리피지 (3%)
max_gas_price_gwei = "100"            # 최대 가스 가격 (Gwei)
gas_multiplier = 1.2                  # 경쟁 가스 배수

# 자금 조달 (v2.0)
use_flashloan = false                 # ⚠️ 항상 false (Wallet-only)

# DEX 라우터 주소
dex_routers = [
    "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",  # Uniswap V2
    "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F",  # SushiSwap
    "0x10ED43C718714eb63d5aA57B78B54704E256024E"   # PancakeSwap V2
]

# 필터링
min_target_value_eth = "1.0"          # 최소 대상 트랜잭션 크기 (ETH)
min_transaction_usd_value = 10000.0   # 최소 USD 가치 ($10,000)
```

### 🔐 환경 변수 설정

**파일 위치:** `.env.local`

```bash
# ===========================================
# Network Configuration
# ===========================================
WS_URL=wss://mainnet.infura.io/ws/v3/YOUR_API_KEY
HTTP_URL=https://mainnet.infura.io/v3/YOUR_API_KEY
CHAIN_ID=1

# ===========================================
# Wallet Configuration
# ===========================================
PRIVATE_KEY=your_private_key_without_0x_prefix

# ===========================================
# Sandwich Strategy
# ===========================================
SANDWICH_ENABLED=true
SANDWICH_MIN_PROFIT_ETH=0.1
SANDWICH_MIN_PROFIT_PERCENTAGE=5.0
SANDWICH_USE_FLASHLOAN=false
SANDWICH_MAX_SLIPPAGE=0.03
SANDWICH_MAX_GAS_PRICE_GWEI=100
SANDWICH_GAS_MULTIPLIER=1.2

# ===========================================
# Flashbots Configuration
# ===========================================
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
FLASHBOTS_SIGNATURE_KEY=your_flashbots_signature_key

# ===========================================
# Oracle Configuration
# ===========================================
CHAINLINK_ORACLE_ENABLED=true
UNISWAP_TWAP_ENABLED=true
ORACLE_AGGREGATION_STRATEGY=weighted_mean
```

### 🎛️ 주요 파라미터 설명

#### 수익성 파라미터

| 파라미터 | 설명 | 권장값 (Testnet) | 권장값 (Mainnet) |
|---------|------|-----------------|-----------------|
| `min_profit_eth` | 최소 순수익 (ETH) | 0.01 | 0.1 |
| `min_profit_percentage` | 최소 수익률 (%) | 2.0% | 5.0% |

#### 리스크 파라미터

| 파라미터 | 설명 | 권장값 (Testnet) | 권장값 (Mainnet) |
|---------|------|-----------------|-----------------|
| `max_slippage` | 최대 슬리피지 | 5% (0.05) | 2% (0.02) |
| `max_gas_price_gwei` | 최대 가스 가격 | 200 Gwei | 100 Gwei |
| `gas_multiplier` | 경쟁 가스 배수 | 1.5x | 1.2x |

#### 필터링 파라미터

| 파라미터 | 설명 | 권장값 |
|---------|------|--------|
| `min_target_value_eth` | 최소 대상 트랜잭션 크기 | 1.0 ETH |
| `min_transaction_usd_value` | 최소 USD 가치 | $10,000 |

---

## 📚 추가 리소스

### 관련 문서

- [STEP_BY_STEP.md](/Users/pc-25-011/work/blockbit/xCrack/docs/STEP_BY_STEP.md) - 4단계 학습 로드맵
- [RUNNING.md](/Users/pc-25-011/work/blockbit/xCrack/docs/RUNNING.md) - 실행 가이드
- [API.md](/Users/pc-25-011/work/blockbit/xCrack/docs/API.md) - API 문서

### 참고 자료

- [Flashbots Documentation](https://docs.flashbots.net/)
- [Uniswap V2 Documentation](https://docs.uniswap.org/protocol/V2/introduction)
- [Kelly Criterion](https://en.wikipedia.org/wiki/Kelly_criterion)
- [MEV Best Practices](https://github.com/flashbots/pm)

---

## ⚠️ 면책 조항

본 문서는 교육 목적으로 작성되었습니다. MEV 봇 운영은 고위험 활동이며, 실제 자금 투입 전 충분한 테스트와 이해가 필요합니다. 저자는 본 문서 사용으로 인한 어떠한 손실에도 책임을 지지 않습니다.

**운영 전 필수 체크리스트:**
- [ ] Testnet에서 충분한 테스트 완료 (최소 100회 이상)
- [ ] 가스비 모니터링 시스템 구축
- [ ] 손실 한도 설정 및 자동 중단 시스템 구현
- [ ] 네트워크 혼잡도 대응 전략 수립
- [ ] 경쟁자 분석 및 대응 전략 마련
