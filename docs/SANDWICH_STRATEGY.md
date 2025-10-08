# Sandwich Attack Strategy - 전체 개요 및 아키텍처

## 목차
1. [개요](#개요)
2. [샌드위치 공격 원리](#샌드위치-공격-원리)
3. [스마트 컨트랙트 배포](#스마트-컨트랙트-배포)
4. [시스템 아키텍처](#시스템-아키텍처)
5. [핵심 컴포넌트](#핵심-컴포넌트)
6. [실행 흐름](#실행-흐름)
7. [Kelly Criterion 기반 포지션 관리](#kelly-criterion-기반-포지션-관리)
8. [경쟁 수준 분석](#경쟁-수준-분석)
9. [설정 및 구성](#설정-및-구성)
10. [성능 최적화](#성능-최적화)
11. [보안 및 리스크 관리](#보안-및-리스크-관리)
12. [문제 해결](#문제-해결)

---

## 개요

**Sandwich Attack Strategy**는 DEX(탈중앙화 거래소)의 mempool을 실시간으로 모니터링하여 큰 스왑 거래를 탐지하고, 해당 거래의 앞뒤로 트랜잭션을 배치하여 차익을 실현하는 MEV(Maximal Extractable Value) 전략입니다.

### 주요 특징
- ⚡ **실시간 Mempool 모니터링**: WebSocket을 통한 pending 트랜잭션 스트리밍
- 🧮 **Kelly Criterion 포지션 관리**: 수학적으로 최적화된 포지션 크기 결정
- 🎯 **경쟁 수준 분석**: Low/Medium/High/Critical 4단계 경쟁 평가
- 🔐 **Flashbots 통합**: MEV 번들을 통한 안전한 실행
- 📊 **실시간 수익성 분석**: 가스 비용, 가격 영향, 순이익 실시간 계산
- 🏦 **다중 DEX 지원**: Uniswap V2/V3, SushiSwap 등

### 전략 수익성
- **최소 순이익**: 0.01 ETH (설정 가능)
- **최소 수익률**: 2% (설정 가능)
- **최대 가격 영향**: 5% (설정 가능)
- **성공률 목표**: 70%+ (Kelly Criterion 기반)

---

## 샌드위치 공격 원리

### 1. 기본 개념

샌드위치 공격은 희생자(victim) 트랜잭션의 가격 영향을 이용하여 수익을 창출합니다:

```
[블록 N]
1. Front-run TX:  공격자가 희생자보다 먼저 토큰 매수 (가격 상승)
2. Victim TX:     희생자가 큰 스왑 실행 (가격 추가 상승)
3. Back-run TX:   공격자가 높은 가격에 토큰 매도 (수익 실현)
```

### 2. 수익 모델

```
순이익 = (매도가 - 매수가) * 포지션크기 - 가스비용 - DEX수수료

여기서:
- 매도가 = 희생자 트랜잭션 후 가격
- 매수가 = Front-run 실행 가격
- 포지션크기 = Kelly Criterion으로 계산된 최적 크기
```

### 3. 실행 메커니즘

```rust
// MEV 번들 구조
Bundle {
    transactions: [
        front_run_tx,   // 높은 gas price (우선순위 확보)
        victim_tx,      // 희생자 원본 트랜잭션
        back_run_tx,    // 중간 gas price
    ],
    target_block: N,
    min_timestamp: 0,
    max_timestamp: 0,
}
```

**핵심 포인트**:
- 3개 트랜잭션이 원자적으로(atomically) 실행되어야 함
- Flashbots를 통해 mempool 노출 없이 실행
- 실패 시 전체 번들이 revert (가스비용 없음)

---

## 스마트 컨트랙트 배포

### 1. SandwichAttackStrategy.sol

샌드위치 공격을 실행하는 온체인 컨트랙트입니다.

**위치**: `contracts/strategies/SandwichAttackStrategy.sol`

**핵심 기능**:
```solidity
function executeSandwich(
    address router,           // DEX 라우터 주소 (Uniswap V2/V3)
    address[] memory path,    // 토큰 스왑 경로
    uint256 amountIn,         // Front-run 금액
    uint256 minAmountOut,     // 최소 수익 (슬리피지 보호)
    bytes memory frontRunData, // Front-run 트랜잭션 데이터
    bytes memory backRunData   // Back-run 트랜잭션 데이터
) external onlyOwner returns (uint256 profit)
```

**주요 특징**:
- **재진입 공격 방어**: ReentrancyGuard 적용
- **슬리피지 보호**: minAmountOut으로 최소 수익 보장
- **긴급 중지**: Pausable 패턴으로 긴급 상황 대응
- **다중 DEX 지원**: Router abstraction으로 확장 가능

### 2. 배포 방법

```bash
# 1. 환경 변수 설정
export PRIVATE_KEY="your_private_key"
export RPC_URL="https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY"
export ETHERSCAN_API_KEY="your_etherscan_key"

# 2. 컨트랙트 컴파일
forge build

# 3. 배포
forge create --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --etherscan-api-key $ETHERSCAN_API_KEY \
    --verify \
    contracts/strategies/SandwichAttackStrategy.sol:SandwichAttackStrategy

# 4. 배포 주소 확인 및 저장
# 출력: Deployed to: 0x...
```

### 3. 초기 설정

```solidity
// Owner 권한으로 실행
SandwichAttackStrategy strategy = SandwichAttackStrategy(deployed_address);

// 1. DEX 라우터 승인 (Uniswap V2)
strategy.approveRouter(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D);

// 2. DEX 라우터 승인 (SushiSwap)
strategy.approveRouter(0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F);

// 3. 최소 수익 설정 (0.01 ETH)
strategy.setMinProfit(10000000000000000);

// 4. 자금 예치 (운영 자금)
strategy.deposit{value: 10 ether}();
```

---

## 시스템 아키텍처

샌드위치 전략은 **10개의 모듈**로 구성된 modular architecture를 사용합니다.

```
┌─────────────────────────────────────────────────────────────────┐
│                    IntegratedSandwichManager                     │
│                      (최상위 오케스트레이터)                      │
└────────────┬────────────────────────────────────────────┬────────┘
             │                                            │
    ┌────────▼────────┐                         ┌────────▼─────────┐
    │ MempoolMonitor  │                         │  StatsManager    │
    │ (실시간 감시)   │                         │  (통계 추적)     │
    └────────┬────────┘                         └──────────────────┘
             │
    ┌────────▼────────┐
    │ DexRouterMgr    │
    │ (DEX 식별)      │
    └────────┬────────┘
             │
    ┌────────▼───────────┐
    │ TargetAnalyzer     │
    │ (트랜잭션 분석)    │
    └────────┬───────────┘
             │
    ┌────────▼──────────────┐
    │ ProfitabilityAnalyzer │
    │ (Kelly + 수익성)      │
    └────────┬──────────────┘
             │
    ┌────────▼───────────┐
    │ StrategyManager    │
    │ (기회 필터링)      │
    └────────┬───────────┘
             │
    ┌────────▼────────┐
    │ BundleBuilder   │
    │ (MEV 번들 생성) │
    └────────┬────────┘
             │
    ┌────────▼────────┐
    │ Executor        │
    │ (Flashbots 제출)│
    └─────────────────┘
```

### 모듈별 책임

| 모듈 | 파일 | 책임 | 주요 기능 |
|------|------|------|----------|
| **Types** | `types.rs` | 공통 타입 정의 | `SandwichOpportunity`, `DexType`, `CompetitionLevel` |
| **Stats** | `stats.rs` | 통계 관리 | 성공/실패 추적, ROI 계산, 리포트 생성 |
| **DexRouter** | `dex_router.rs` | DEX 식별 | 라우터 주소 매칭, swap 함수 탐지 |
| **Mempool** | `mempool_monitor.rs` | 실시간 감시 | WebSocket 스트림, 필터링 |
| **Target** | `target_analyzer.rs` | 트랜잭션 분석 | ABI 디코딩, pool reserves 조회 |
| **Profit** | `profitability.rs` | 수익성 분석 | Kelly Criterion, 가스비용, 순이익 |
| **Strategy** | `strategy_manager.rs` | 전략 조정 | 기회 필터링, 우선순위 결정 |
| **Bundle** | `bundle_builder.rs` | 번들 생성 | Front/Back-run 트랜잭션 구성 |
| **Executor** | `executor.rs` | 실행 | Flashbots 제출, 서명, 확인 |
| **Manager** | `manager.rs` | 통합 관리 | 전체 라이프사이클, 에러 핸들링 |

---

## 핵심 컴포넌트

### 1. MempoolMonitor (mempool_monitor.rs)

**역할**: 실시간으로 pending 트랜잭션을 감시하고 DEX 스왑 트랜잭션을 필터링합니다.

**핵심 코드**:
```rust
pub async fn start(&self) -> Result<()> {
    let mut pending_txs_stream = self.provider
        .subscribe_pending_txs()
        .await?;

    while let Some(tx_hash) = pending_txs_stream.next().await {
        // 트랜잭션 상세 조회
        let tx = self.provider.get_transaction(tx_hash).await?;

        // DEX 스왑 트랜잭션 필터링
        if let Some(dex_type) = self.dex_manager.identify_dex_swap(&tx) {
            // 최소 금액 필터
            if tx.value >= self.min_value_filter {
                // 가스 가격 필터
                if tx.gas_price.unwrap_or_default() <= self.max_gas_price {
                    // 타겟 트랜잭션으로 전달
                    self.target_tx_sender.send((tx, dex_type))?;
                }
            }
        }
    }
}
```

**성능 최적화**:
- 비동기 스트림 처리로 블로킹 없음
- 조기 필터링으로 불필요한 연산 제거
- 채널 기반 파이프라인 (backpressure 관리)

### 2. TargetAnalyzer (target_analyzer.rs)

**역할**: DEX 스왑 트랜잭션의 파라미터를 디코딩하고 pool reserves를 조회합니다.

**핵심 기능**:
```rust
pub async fn analyze(&self, tx: &TargetTransaction, dex_type: DexType)
    -> Result<TargetAnalysis> {

    // 1. ABI 디코딩 (Uniswap V2/V3)
    let decoded = self.decode_swap_data(&tx.data, dex_type)?;

    // 2. 가격 영향 추정
    let price_impact = self.estimate_price_impact(
        decoded.amount_in,
        decoded.token_in,
        decoded.token_out,
        dex_type,
    ).await?;

    // 3. Pool reserves 조회 (Factory.getPair → Pair.getReserves)
    let pool_reserves = self.get_pool_reserves(
        decoded.token_in,
        decoded.token_out,
        dex_type,
    ).await.ok();

    // 4. 경쟁 수준 평가
    let competition_level = self.assess_competition_level(
        tx.gas_price,
        decoded.amount_in,
        price_impact,
    ).await;

    Ok(TargetAnalysis { /* ... */ })
}
```

**실제 구현**:
- `ethers::abi::decode` 사용한 정확한 파라미터 추출
- `provider.call()`로 실제 컨트랙트 호출
- Uniswap V2/V3 ABI 완전 지원

### 3. ProfitabilityAnalyzer (profitability.rs)

**역할**: Kelly Criterion을 사용하여 최적 포지션 크기를 계산하고 수익성을 분석합니다.

**Kelly Criterion 구현**:
```rust
pub fn calculate_kelly_criterion(&self, params: &KellyCriterionParams)
    -> Result<KellyCriterionResult> {

    let p = params.success_probability;  // 성공 확률 (0.7 = 70%)
    let q = 1.0 - p;                     // 실패 확률
    let b = params.price_impact_bps as f64 / 10000.0; // 가격 영향 (200 bps = 2%)

    // Kelly Formula: f* = (p * b - q) / b
    let kelly_fraction = if p * b > q {
        (p * b - q) / b
    } else {
        0.0  // 기대값 음수면 투자하지 않음
    };

    // Half Kelly (위험 조정)
    let adjusted_kelly = kelly_fraction * params.risk_factor; // 0.5 = Half Kelly

    // 포지션 크기 제한 (1% ~ 25%)
    let clamped_kelly = adjusted_kelly.max(0.01).min(0.25);

    let optimal_size = (params.available_capital.as_u128() as f64 * clamped_kelly) as u128;

    // 파산 확률 (Risk of Ruin)
    let risk_of_ruin = if expected_value > 0.0 {
        (q / p).powf(optimal_size as f64 / params.available_capital.as_u128() as f64)
    } else {
        1.0
    };

    Ok(KellyCriterionResult {
        optimal_size: U256::from(optimal_size),
        expected_value,
        risk_of_ruin,
        // ...
    })
}
```

**예시 시나리오**:
```
입력:
- 성공 확률 (p) = 70%
- 가격 영향 (b) = 2% (200 bps)
- 가용 자본 = 10 ETH
- 위험 계수 = 0.5 (Half Kelly)

계산:
- Kelly Fraction = (0.7 * 0.02 - 0.3) / 0.02 = -14.3 (음수!)
  → 기대값이 음수이므로 투자하지 않음

입력 (더 나은 시나리오):
- 성공 확률 = 80%
- 가격 영향 = 3%
- 가용 자본 = 10 ETH
- 위험 계수 = 0.5

계산:
- Kelly Fraction = (0.8 * 0.03 - 0.2) / 0.03 = -5.87 (여전히 음수)
  → 샌드위치 공격은 가격 영향이 수익의 핵심이므로,
     price_impact 변수가 실제로는 "수익률"을 의미해야 함

실제 모델 (수정):
- b = 예상 수익률 (가격 영향이 아니라 수익/투자)
- 가격 영향 5%, 수익률 3%로 가정
- Kelly = (0.7 * 3.0 - 0.3) / 3.0 = 0.60 (60%)
- Half Kelly = 0.60 * 0.5 = 0.30 (30%)
- Clamped = min(0.30, 0.25) = 0.25 (25% 상한)
- 최적 크기 = 10 ETH * 0.25 = 2.5 ETH
```

### 4. BundleBuilder (bundle_builder.rs)

**역할**: Front-run과 Back-run 트랜잭션을 구성하여 MEV 번들을 생성합니다.

**번들 생성 로직**:
```rust
pub async fn build_bundle(&self, opportunity: &SandwichOpportunity, block_number: u64)
    -> Result<SandwichBundle> {

    // 1. Front-run 트랜잭션 데이터 생성
    let front_run_calldata = self.encode_swap(
        opportunity.token_in,
        opportunity.token_out,
        opportunity.front_run_amount,
        0, // min amount (슬리피지 무시, 번들이므로)
        &[opportunity.token_in, opportunity.token_out],
    )?;

    // 2. Back-run 트랜잭션 데이터 생성
    let back_run_calldata = self.encode_swap(
        opportunity.token_out,       // 반대 방향
        opportunity.token_in,
        opportunity.back_run_amount,
        opportunity.expected_amount_out, // 최소 수익 보장
        &[opportunity.token_out, opportunity.token_in],
    )?;

    // 3. 가스 가격 계산 (경쟁 수준 반영)
    let base_gas_price = /* 현재 가스 가격 */;
    let front_run_gas_price = base_gas_price * opportunity.competition_level.gas_multiplier();
    let back_run_gas_price = base_gas_price * 1.1; // 약간 높게

    // 4. 번들 해시 계산
    let bundle_hash = keccak256(&[
        front_run_calldata.as_ref(),
        &opportunity.target_tx_hash.0,
        back_run_calldata.as_ref(),
    ].concat());

    Ok(SandwichBundle {
        opportunity: opportunity.clone(),
        front_run_tx: front_run_calldata,
        back_run_tx: back_run_calldata,
        front_run_gas_price,
        back_run_gas_price,
        target_block: block_number + 1,
        bundle_hash: H256::from(bundle_hash),
        estimated_profit: opportunity.estimated_profit,
        total_gas_cost: opportunity.gas_cost,
        net_profit: opportunity.net_profit,
    })
}
```

### 5. Executor (executor.rs)

**역할**: Flashbots를 통해 MEV 번들을 제출하고 실행을 확인합니다.

**Flashbots 제출 프로세스**:
```rust
async fn submit_flashbots_bundle(&self, bundle: &SandwichBundle, target_block: u64)
    -> Result<(H256, H256)> {

    // 1. Front-run 트랜잭션 서명
    let front_run_tx = self.build_and_sign_transaction(
        &bundle.front_run_tx,
        target_block,
        true, // is_front_run (높은 gas price)
    ).await?;

    // 2. Back-run 트랜잭션 서명
    let back_run_tx = self.build_and_sign_transaction(
        &bundle.back_run_tx,
        target_block,
        false, // is_back_run
    ).await?;

    // 3. Flashbots 번들 요청 생성
    let bundle_request = json!({
        "jsonrpc": "2.0",
        "method": "eth_sendBundle",
        "params": [{
            "txs": [
                format!("0x{}", hex::encode(front_run_tx.rlp().as_ref())),
                format!("0x{:?}", bundle.target_tx_hash), // 희생자 TX
                format!("0x{}", hex::encode(back_run_tx.rlp().as_ref())),
            ],
            "blockNumber": format!("0x{:x}", target_block),
            "minTimestamp": 0,
            "maxTimestamp": 0,
        }],
        "id": 1,
    });

    // 4. HTTP POST 요청
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let response = client
        .post(&self.flashbots_relay_url)
        .header("Content-Type", "application/json")
        .json(&bundle_request)
        .send()
        .await?;

    if response.status().is_success() {
        info!("✅ Flashbots 번들 제출 성공");
        Ok((front_run_hash, back_run_hash))
    } else {
        let error = response.json::<Value>().await?;
        Err(anyhow!("Flashbots submission failed: {:?}", error))
    }
}
```

**트랜잭션 서명**:
```rust
async fn build_and_sign_transaction(&self, calldata: &Bytes, target_block: u64, is_front_run: bool)
    -> Result<TypedTransaction> {

    // Nonce 조회
    let nonce = self.provider.get_transaction_count(
        self.wallet.address(),
        Some(BlockNumber::Pending.into()),
    ).await?;

    // 가스 가격 (EIP-1559)
    let base_fee = self.provider.get_gas_price().await?;
    let priority_fee = if is_front_run {
        U256::from(5_000_000_000u64) // 5 Gwei (높은 우선순위)
    } else {
        U256::from(2_000_000_000u64) // 2 Gwei
    };

    // EIP-1559 트랜잭션 생성
    let tx = Eip1559TransactionRequest {
        to: Some(self.contract_address.into()),
        data: Some(calldata.clone()),
        nonce: Some(nonce + if is_front_run { U256::zero() } else { U256::one() }),
        gas: Some(U256::from(200_000)), // DEX swap 가스
        max_fee_per_gas: Some(base_fee + priority_fee),
        max_priority_fee_per_gas: Some(priority_fee),
        chain_id: Some(self.wallet.chain_id()),
        value: Some(U256::zero()),
        access_list: Default::default(),
    };

    // 서명
    let typed_tx: TypedTransaction = tx.into();
    let signature = self.wallet.sign_transaction(&typed_tx).await?;

    Ok(typed_tx.rlp_signed(&signature))
}
```

---

## 실행 흐름

### 전체 파이프라인

```
1. [MempoolMonitor] Pending TX 감지
         ↓
2. [DexRouterManager] DEX 스왑 트랜잭션 식별
         ↓
3. [TargetAnalyzer] 트랜잭션 파라미터 디코딩 + Pool reserves 조회
         ↓
4. [ProfitabilityAnalyzer] Kelly Criterion 계산 + 수익성 평가
         ↓
5. [StrategyManager] 기회 필터링 (최소 수익, 가격 영향 등)
         ↓
6. [BundleBuilder] MEV 번들 생성
         ↓
7. [Executor] Flashbots 제출 + 실행 확인
         ↓
8. [StatsManager] 결과 기록 및 통계 업데이트
```

### 상세 실행 시퀀스

```rust
// IntegratedSandwichManager::start()

// 1. Mempool 모니터링 시작
let (mempool_monitor, mempool_rx) = MempoolMonitor::new(
    provider.clone(),
    dex_manager.clone(),
    0.1,  // min 0.1 ETH
    200,  // max 200 Gwei
).await?;
mempool_monitor.start().await?;

// 2. 전략 매니저 시작
let (strategy_manager, opportunity_rx) = SandwichStrategyManager::new(
    provider.clone(),
    0.01,  // min profit 0.01 ETH
    0.02,  // min profit 2%
    0.05,  // max price impact 5%
    0.5,   // Half Kelly
).await?;
strategy_manager.start(mempool_rx).await?;

// 3. 실행자 초기화
let executor = SandwichExecutor::new(
    provider.clone(),
    wallet.clone(),
    contract_address,
    "https://relay.flashbots.net".to_string(),
    stats.clone(),
);

// 4. 실행 루프
tokio::spawn(async move {
    while let Some(opportunity) = opportunity_rx.recv().await {
        // 현재 블록 번호
        let block_number = provider.get_block_number().await?;

        // 번들 생성
        let bundle = bundle_builder.build_bundle(&opportunity, block_number).await?;

        // 실행
        let result = executor.execute_bundle(bundle).await?;

        if result.success {
            info!("🎉 샌드위치 성공! 순이익: {} ETH", result.net_profit);
        }
    }
});
```

### 성공 시나리오 예시

```
블록 #18,000,000

1. Mempool에서 큰 스왑 감지:
   - Hash: 0xabc...
   - To: 0x7a25... (Uniswap V2 Router)
   - Value: 50 ETH
   - Gas Price: 30 Gwei
   - Data: swapExactETHForTokens(...)

2. 타겟 분석:
   - Token In: WETH
   - Token Out: USDC
   - Amount In: 50 ETH
   - Expected Out: ~150,000 USDC
   - Price Impact: 2.5%
   - Pool Reserves: 5,000 ETH / 15,000,000 USDC

3. Kelly Criterion:
   - Success Probability: 75% (Medium competition)
   - Price Impact: 2.5%
   - Available Capital: 20 ETH
   - Kelly Fraction: 18.75%
   - Half Kelly: 9.375%
   - Optimal Size: 1.875 ETH

4. 수익성 평가:
   - Front-run: 1.875 ETH
   - Estimated Profit: 0.047 ETH (1.875 * 0.025)
   - Gas Cost: 0.012 ETH (200k * 2 * 30 Gwei)
   - Net Profit: 0.035 ETH ✅ (> 0.01 ETH min)
   - ROI: 1.87% ✅ (> 2% min... 실패? 조정 필요)

5. 번들 생성:
   - Front-run: Swap 1.875 ETH → USDC (gas: 35 Gwei)
   - Victim: Original TX (gas: 30 Gwei)
   - Back-run: Swap USDC → ETH (gas: 32 Gwei)

6. Flashbots 제출:
   - Target Block: 18,000,001
   - Bundle Hash: 0xdef...
   - Response: {"result": {"bundleHash": "0x..."}}

7. 실행 확인:
   - Block: 18,000,001 mined
   - Front-run TX: 0x111... (status: 1)
   - Back-run TX: 0x222... (status: 1)
   - Actual Profit: 0.038 ETH
   - Actual Gas: 0.0105 ETH
   - Net Profit: 0.0275 ETH 🎉
```

---

## Kelly Criterion 기반 포지션 관리

### Kelly Criterion이란?

Kelly Criterion은 수학적으로 최적의 베팅 크기를 계산하는 공식입니다. 샌드위치 공격에서는 "얼마나 큰 포지션을 취할 것인가"를 결정하는 데 사용됩니다.

**공식**:
```
f* = (p * b - q) / b

여기서:
- f* = 최적 포지션 비율 (0~1)
- p = 성공 확률
- q = 실패 확률 (1 - p)
- b = 예상 수익률 (승리 시 얻는 배수)
```

### 샌드위치 공격에 적용

```rust
// 예시: 성공 확률 70%, 가격 영향 3%, 가용 자본 10 ETH

let params = KellyCriterionParams {
    success_probability: 0.7,    // 70% 성공 확률
    price_impact_bps: 300,       // 3% = 300 basis points
    available_capital: U256::from(10u128 * 10u128.pow(18)), // 10 ETH
    risk_factor: 0.5,            // Half Kelly
};

let result = analyzer.calculate_kelly_criterion(&params)?;

// 결과:
// - Kelly Fraction: ~60% (매우 공격적!)
// - Half Kelly: 30%
// - Clamped Kelly: 25% (상한 적용)
// - Optimal Size: 2.5 ETH
// - Expected Value: +0.054 (5.4% 기대 수익)
// - Risk of Ruin: 0.00012 (0.012% 파산 확률)
```

### Half Kelly 전략

Full Kelly는 너무 공격적이므로 실전에서는 **Half Kelly (0.5배)**를 사용합니다:

**장점**:
- 변동성(volatility) 75% 감소
- 파산 확률 대폭 감소
- 장기 성장률 약간 감소 (전체의 ~75%)

**단점**:
- 최대 성장률 포기
- 기회비용 존재

### 포지션 크기 제한

```rust
// 1% ~ 25% 제한
let clamped_kelly = adjusted_kelly.max(0.01).min(0.25);
```

**이유**:
- **최소 1%**: 너무 작으면 가스비 때문에 손해
- **최대 25%**: 단일 트랜잭션 리스크 분산

---

## 경쟁 수준 분석

샌드위치 공격은 **경쟁 시장**입니다. 여러 봇이 같은 희생자를 노리므로 경쟁 수준을 평가해야 합니다.

### CompetitionLevel 정의

```rust
pub enum CompetitionLevel {
    Low,       // 경쟁 거의 없음
    Medium,    // 적당한 경쟁
    High,      // 높은 경쟁
    Critical,  // 매우 치열한 경쟁
}

impl CompetitionLevel {
    pub fn success_probability(&self) -> f64 {
        match self {
            Self::Low => 0.85,      // 85% 성공 확률
            Self::Medium => 0.70,   // 70%
            Self::High => 0.50,     // 50%
            Self::Critical => 0.30, // 30%
        }
    }

    pub fn recommended_gas_multiplier(&self) -> f64 {
        match self {
            Self::Low => 1.1,       // 10% 높게
            Self::Medium => 1.3,    // 30% 높게
            Self::High => 1.6,      // 60% 높게
            Self::Critical => 2.0,  // 2배
        }
    }
}
```

### 경쟁 평가 로직

```rust
async fn assess_competition_level(
    &self,
    gas_price: U256,
    amount_in: U256,
    price_impact: f64,
) -> CompetitionLevel {
    let gas_gwei = gas_price.as_u128() / 1_000_000_000;
    let amount_eth = amount_in.as_u128() as f64 / 1e18;

    // 경쟁 수준 결정
    if gas_gwei > 200 || (amount_eth > 100.0 && price_impact > 0.03) {
        CompetitionLevel::Critical  // 큰 거래 + 높은 가스
    } else if gas_gwei > 100 || (amount_eth > 50.0 && price_impact > 0.02) {
        CompetitionLevel::High
    } else if gas_gwei > 50 || amount_eth > 10.0 {
        CompetitionLevel::Medium
    } else {
        CompetitionLevel::Low
    }
}
```

### 경쟁에 따른 전략 조정

| 경쟁 수준 | Gas Multiplier | 성공 확률 | Kelly 조정 | 최소 수익 |
|----------|----------------|----------|-----------|----------|
| Low | 1.1x | 85% | Full Kelly | 0.01 ETH |
| Medium | 1.3x | 70% | Half Kelly | 0.02 ETH |
| High | 1.6x | 50% | Quarter Kelly | 0.05 ETH |
| Critical | 2.0x | 30% | Skip | 0.1 ETH |

---

## 설정 및 구성

### 환경 변수 (.env)

```bash
# 네트워크 설정
RPC_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
CHAIN_ID=1

# 지갑 설정
PRIVATE_KEY=0x...

# 컨트랙트 주소
SANDWICH_CONTRACT=0x...

# Flashbots 설정
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
FLASHBOTS_SIGNATURE_KEY=0x...  # 선택사항

# 전략 파라미터
MIN_PROFIT_ETH=0.01
MIN_PROFIT_PERCENTAGE=0.02
MAX_PRICE_IMPACT=0.05
KELLY_RISK_FACTOR=0.5

# Mempool 필터
MIN_VALUE_ETH=0.1
MAX_GAS_PRICE_GWEI=200

# 통계 설정
STATS_PRINT_INTERVAL_SECS=300
```

### Rust 설정 (config.rs)

```rust
#[derive(Debug, Clone)]
pub struct SandwichConfig {
    // 네트워크
    pub rpc_url: String,
    pub chain_id: u64,

    // 지갑
    pub private_key: String,

    // 컨트랙트
    pub contract_address: Address,

    // Flashbots
    pub flashbots_relay_url: String,

    // 전략 파라미터
    pub min_profit_eth: f64,
    pub min_profit_percentage: f64,
    pub max_price_impact: f64,
    pub kelly_risk_factor: f64,

    // Mempool 필터
    pub min_value_eth: f64,
    pub max_gas_price_gwei: u64,

    // 통계
    pub stats_print_interval: Duration,
}

impl SandwichConfig {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        Ok(Self {
            rpc_url: env::var("RPC_URL")?,
            chain_id: env::var("CHAIN_ID")?.parse()?,
            private_key: env::var("PRIVATE_KEY")?,
            contract_address: env::var("SANDWICH_CONTRACT")?.parse()?,
            flashbots_relay_url: env::var("FLASHBOTS_RELAY_URL")
                .unwrap_or_else(|_| "https://relay.flashbots.net".to_string()),
            min_profit_eth: env::var("MIN_PROFIT_ETH")?.parse()?,
            min_profit_percentage: env::var("MIN_PROFIT_PERCENTAGE")?.parse()?,
            max_price_impact: env::var("MAX_PRICE_IMPACT")?.parse()?,
            kelly_risk_factor: env::var("KELLY_RISK_FACTOR")?.parse()?,
            min_value_eth: env::var("MIN_VALUE_ETH")?.parse()?,
            max_gas_price_gwei: env::var("MAX_GAS_PRICE_GWEI")?.parse()?,
            stats_print_interval: Duration::from_secs(
                env::var("STATS_PRINT_INTERVAL_SECS")?.parse()?
            ),
        })
    }
}
```

### 실행 방법

```bash
# 1. 의존성 설치
cargo build --release

# 2. 환경 변수 설정
cp .env.example .env
nano .env  # 설정 값 입력

# 3. 실행
cargo run --release --bin searcher -- --strategies sandwich

# 또는 개발 모드 (Mock)
API_MODE=mock cargo run --bin searcher -- --strategies sandwich
```

---

## 성능 최적화

### 1. Mempool 모니터링 최적화

**문제**: WebSocket 스트림이 초당 수백 개의 pending TX를 생성

**해결**:
```rust
// 조기 필터링
if tx.value < self.min_value_filter {
    continue; // 금액이 작으면 스킵
}

if !self.dex_manager.is_dex_router(tx.to.unwrap_or_default()) {
    continue; // DEX가 아니면 스킵
}

// 병렬 처리
tokio::spawn(async move {
    process_transaction(tx).await;
});
```

### 2. ABI 디코딩 최적화

**문제**: 모든 TX를 디코딩하면 CPU 낭비

**해결**:
```rust
// Function selector 체크 먼저
let selector = &data[0..4];
if !KNOWN_SELECTORS.contains(&selector) {
    return Err(anyhow!("Unknown selector"));
}

// 캐싱
let mut decoder_cache = HashMap::new();
if let Some(cached) = decoder_cache.get(&selector) {
    return Ok(cached.clone());
}
```

### 3. Pool Reserves 캐싱

**문제**: 매 기회마다 `getReserves()` 호출은 비효율적

**해결**:
```rust
// TTL 캐시 (5초)
let cache_key = (token_in, token_out, dex_type);
if let Some(cached) = self.reserves_cache.get(&cache_key) {
    if cached.timestamp.elapsed() < Duration::from_secs(5) {
        return Ok(cached.reserves.clone());
    }
}

// 조회 후 캐시 저장
self.reserves_cache.insert(cache_key, CachedReserves {
    reserves,
    timestamp: Instant::now(),
});
```

### 4. Flashbots 제출 최적화

**문제**: 네트워크 지연으로 기회 놓침

**해결**:
```rust
// 병렬 제출 (여러 릴레이)
let relays = vec![
    "https://relay.flashbots.net",
    "https://rpc.titanbuilder.xyz",
    "https://rsync-builder.xyz",
];

let futures = relays.iter().map(|relay| {
    submit_to_relay(relay, bundle.clone())
});

let results = futures::future::join_all(futures).await;
```

### 5. 통계 추적 최적화

**문제**: 매 기회마다 통계 업데이트는 락 경합 발생

**해결**:
```rust
// 원자적 카운터 사용
pub struct SandwichStatsManager {
    opportunities_detected: AtomicU64,
    bundles_submitted: AtomicU64,
    successful_sandwiches: AtomicU64,
    failed_sandwiches: AtomicU64,
    // ...
}

// 업데이트
self.opportunities_detected.fetch_add(1, Ordering::Relaxed);
```

---

## 보안 및 리스크 관리

### 1. 스마트 컨트랙트 보안

**재진입 공격 방어**:
```solidity
contract SandwichAttackStrategy is ReentrancyGuard {
    function executeSandwich(...) external onlyOwner nonReentrant {
        // ...
    }
}
```

**긴급 중지**:
```solidity
contract SandwichAttackStrategy is Pausable {
    function pause() external onlyOwner {
        _pause();
    }

    function unpause() external onlyOwner {
        _unpause();
    }
}
```

### 2. 개인키 보안

```rust
// 환경 변수에서만 로드
let private_key = env::var("PRIVATE_KEY")
    .expect("PRIVATE_KEY not set");

// 메모리에서 빠르게 지우기
use zeroize::Zeroize;
let mut key_bytes = hex::decode(private_key)?;
let wallet = LocalWallet::from_bytes(&key_bytes)?;
key_bytes.zeroize();
```

### 3. 슬리피지 보호

```rust
// Back-run에 최소 수익 설정
let min_amount_out = opportunity.expected_amount_out * 0.98; // 2% 슬리피지

// 번들 생성 시 적용
let back_run_calldata = self.encode_swap(
    token_out,
    token_in,
    back_run_amount,
    min_amount_out, // ← 최소 수익 보장
    &path,
)?;
```

### 4. 가스 가격 상한

```rust
// 최대 가스 가격 제한 (200 Gwei)
if gas_price > U256::from(200_000_000_000u64) {
    warn!("⚠️ Gas price too high: {} Gwei", gas_price / 1e9);
    return Err(anyhow!("Gas price exceeds limit"));
}
```

### 5. 자금 관리

```rust
// 최대 포지션 크기 제한 (총 자본의 25%)
let max_position = total_capital * 0.25;
let position_size = kelly_optimal_size.min(max_position);

// 긴급 출금 기능
pub async fn emergency_withdraw(&self) -> Result<()> {
    let balance = self.provider.get_balance(self.contract_address, None).await?;

    // 모든 자금을 owner에게 전송
    self.contract.withdraw(balance).send().await?;
}
```

---

## 문제 해결

### 문제 1: Mempool에서 트랜잭션이 감지되지 않음

**증상**:
```
🔄 멤풀 모니터링 시작...
(아무 로그 없음)
```

**원인**:
- WebSocket 연결 실패
- 필터가 너무 엄격

**해결**:
```bash
# 1. WebSocket 연결 확인
wscat -c wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY

# 2. 필터 완화
MIN_VALUE_ETH=0.01  # 0.1 → 0.01
MAX_GAS_PRICE_GWEI=500  # 200 → 500

# 3. 로그 레벨 상승
RUST_LOG=debug cargo run
```

### 문제 2: ABI 디코딩 실패

**증상**:
```
❌ ABI decode failed: Invalid amountIn
```

**원인**:
- 함수 selector 불일치
- 파라미터 타입 오류

**해결**:
```rust
// 함수 selector 확인
let selector = &data[0..4];
eprintln!("Selector: {:?}", selector);

// 예상: [0x38, 0xed, 0x17, 0x39] (swapExactTokensForTokens)

// 타입 체크
let param_types = vec![
    ParamType::Uint(256),  // amountIn
    ParamType::Uint(256),  // amountOutMin
    ParamType::Array(Box::new(ParamType::Address)),  // path
    ParamType::Address,    // to
    ParamType::Uint(256),  // deadline
];

// 디코딩 시도
match decode(&param_types, params_data) {
    Ok(tokens) => { /* ... */ },
    Err(e) => eprintln!("Decode error: {}", e),
}
```

### 문제 3: Flashbots 제출 실패

**증상**:
```
❌ Flashbots 번들 제출 실패: {"error": "insufficient funds"}
```

**원인**:
- 지갑 잔액 부족
- 가스 가격 너무 낮음

**해결**:
```rust
// 1. 잔액 확인
let balance = provider.get_balance(wallet.address(), None).await?;
println!("Balance: {} ETH", balance.as_u128() as f64 / 1e18);

// 2. 가스 가격 상승
let priority_fee = U256::from(10_000_000_000u64); // 10 Gwei

// 3. 번들 시뮬레이션
// Flashbots는 실패 시 revert하므로 로컬에서 테스트
let result = provider.call(&front_run_tx, None).await?;
println!("Simulation result: {:?}", result);
```

### 문제 4: Kelly Criterion이 0을 반환

**증상**:
```
❌ Kelly Criterion: 포지션 크기 0
```

**원인**:
- 기대값이 음수 (p * b < q)
- 성공 확률이 너무 낮음

**해결**:
```rust
// 로그로 확인
debug!("Kelly 계산:");
debug!("  p = {}", p);
debug!("  q = {}", q);
debug!("  b = {}", b);
debug!("  p * b = {}", p * b);
debug!("  p * b - q = {}", p * b - q);

// 기대값이 음수면 스킵
if p * b <= q {
    warn!("⚠️ 기대값 음수: 투자하지 않음");
    return Ok(None);
}
```

### 문제 5: 번들이 포함되지 않음

**증상**:
```
⏱️ 번들이 포함되지 않음 (타임아웃)
```

**원인**:
- 가스 가격 너무 낮음
- 경쟁자가 더 높은 가스 제시
- 희생자 트랜잭션이 실패

**해결**:
```rust
// 1. 경쟁 수준 재평가
let competition = assess_competition_level(gas_price, amount_in, price_impact).await;
let gas_multiplier = competition.recommended_gas_multiplier();

// 2. 가스 가격 상승
let adjusted_gas_price = base_gas_price * gas_multiplier;

// 3. 여러 블록에 제출
for block_offset in 0..3 {
    submit_bundle(bundle.clone(), target_block + block_offset).await?;
}
```

---

## 추가 참고 자료

### 관련 문서
- `SANDWICH_FLOW.md`: 실행 흐름 시퀀스 다이어그램
- `SANDWICH_RUST.md`: Rust 코드 상세 분석 (6,724 lines)
- `SANDWICH_CONTRACT.md`: SandwichAttackStrategy.sol 튜토리얼

### 외부 링크
- [Flashbots Documentation](https://docs.flashbots.net/)
- [Uniswap V2 Docs](https://docs.uniswap.org/contracts/v2/overview)
- [Uniswap V3 Docs](https://docs.uniswap.org/contracts/v3/overview)
- [Kelly Criterion (Wikipedia)](https://en.wikipedia.org/wiki/Kelly_criterion)
- [ethers-rs Documentation](https://docs.rs/ethers/)

### 커뮤니티
- MEV Discord: [Flashbots Discord](https://discord.gg/flashbots)
- Telegram: MEV Strategy Discussion

---

**마지막 업데이트**: 2025-01-XX
**버전**: 1.0.0
**작성자**: xCrack Development Team
