# 🥪 Sandwich Attack Flow 시퀀스 다이어그램

> **샌드위치 공격 전략의 모든 시나리오별 상세 시퀀스 다이어그램**
>
> 각 컴포넌트와 외부 서비스 간의 상호작용을 단계별로 시각화

---

## 📋 목차

1. [전체 샌드위치 프로세스](#-전체-샌드위치-프로세스)
2. [Mempool 모니터링 및 타겟 탐지 플로우](#-mempool-모니터링-및-타겟-탐지-플로우)
3. [Uniswap V2 타겟 분석 플로우](#-uniswap-v2-타겟-분석-플로우)
4. [Uniswap V3 타겟 분석 플로우](#-uniswap-v3-타겟-분석-플로우)
5. [Kelly Criterion 포지션 계산 플로우](#-kelly-criterion-포지션-계산-플로우)
6. [MEV 번들 생성 및 제출 플로우](#-mev-번들-생성-및-제출-플로우)
7. [경쟁 수준 평가 및 가스 최적화 플로우](#-경쟁-수준-평가-및-가스-최적화-플로우)
8. [Pool Reserves 조회 플로우](#-pool-reserves-조회-플로우)
9. [Flashbots 실행 및 확인 플로우](#-flashbots-실행-및-확인-플로우)
10. [에러 처리 및 복구 플로우](#-에러-처리-및-복구-플로우)

---

## 🔄 전체 샌드위치 프로세스

### 1️⃣ 통합 샌드위치 관리자 실행 플로우

```mermaid
sequenceDiagram
    participant User as 사용자/봇
    participant ISM as IntegratedSandwichManager
    participant MM as MempoolMonitor
    participant DRM as DexRouterManager
    participant TA as TargetAnalyzer
    participant PA as ProfitabilityAnalyzer
    participant SSM as SandwichStrategyManager
    participant BB as BundleBuilder
    participant EX as SandwichExecutor
    participant FB as Flashbots Relay
    participant BC as Blockchain

    User->>ISM: start()
    activate ISM

    ISM->>MM: new(provider, dex_manager, 0.1 ETH, 200 Gwei)
    activate MM
    MM-->>ISM: (mempool_monitor, mempool_rx)
    deactivate MM

    ISM->>MM: start()
    activate MM
    MM->>BC: subscribe_pending_txs()
    activate BC
    BC-->>MM: pending_tx_stream
    deactivate BC
    MM-->>ISM: OK
    deactivate MM

    ISM->>SSM: new(provider, 0.01 ETH, 2%, 5%, 0.5)
    activate SSM
    SSM-->>ISM: (strategy_manager, opportunity_rx)
    deactivate SSM

    ISM->>SSM: start(mempool_rx)
    activate SSM
    SSM-->>ISM: OK
    deactivate SSM

    ISM->>EX: new(provider, wallet, contract, relay_url)
    activate EX
    EX-->>ISM: executor
    deactivate EX

    ISM->>ISM: start_execution_loop(opportunity_rx)
    ISM->>ISM: start_stats_loop()

    loop 실행 루프 (실시간)
        MM->>BC: pending_tx_stream.next()
        activate BC
        BC-->>MM: tx_hash
        deactivate BC

        MM->>BC: get_transaction(tx_hash)
        activate BC
        BC-->>MM: transaction
        deactivate BC

        MM->>DRM: identify_dex_swap(tx)
        activate DRM
        DRM-->>MM: Some(dex_type) | None
        deactivate DRM

        alt DEX 스왑 트랜잭션
            MM->>MM: filter by value (>= 0.1 ETH)
            MM->>MM: filter by gas_price (<= 200 Gwei)

            alt 필터 통과
                MM->>SSM: send (target_tx, dex_type)

                SSM->>TA: analyze(target_tx, dex_type)
                activate TA
                TA->>TA: decode_swap_data(tx.data, dex_type)
                TA->>TA: estimate_price_impact(amount_in, tokens)
                TA->>BC: get_pool_reserves(token_in, token_out)
                activate BC
                BC-->>TA: pool_reserves
                deactivate BC
                TA->>TA: assess_competition_level(gas_price, amount)
                TA-->>SSM: target_analysis
                deactivate TA

                SSM->>BC: get_gas_price()
                activate BC
                BC-->>SSM: current_gas_price
                deactivate BC

                SSM->>PA: analyze_opportunity(target_analysis, gas_price)
                activate PA
                PA->>PA: filter by price_impact (<= 5%)
                PA->>PA: calculate_kelly_criterion(params)
                PA->>PA: estimate_profit(front_run_amount, target)
                PA->>PA: estimate_gas_cost(competition)
                PA->>PA: check min_profit (>= 0.01 ETH, >= 2%)
                PA-->>SSM: Some(opportunity) | None
                deactivate PA

                alt 수익성 있음
                    SSM->>ISM: send opportunity

                    ISM->>BC: get_block_number()
                    activate BC
                    BC-->>ISM: block_number
                    deactivate BC

                    ISM->>BB: build_bundle(opportunity, block_number)
                    activate BB
                    BB->>BB: encode front_run swap
                    BB->>BB: encode back_run swap
                    BB->>BB: calculate gas_prices (competition)
                    BB->>BB: compute bundle_hash
                    BB-->>ISM: sandwich_bundle
                    deactivate BB

                    ISM->>EX: execute_bundle(bundle)
                    activate EX
                    EX->>EX: build_and_sign_transaction(front_run)
                    EX->>EX: build_and_sign_transaction(back_run)
                    EX->>FB: submit_flashbots_bundle(bundle)
                    activate FB
                    FB->>BC: eth_sendBundle
                    activate BC
                    BC-->>FB: bundle_hash
                    deactivate BC
                    FB-->>EX: (front_run_hash, back_run_hash)
                    deactivate FB

                    loop 번들 포함 대기 (최대 3블록)
                        EX->>BC: get_transaction_receipt(front_run_hash)
                        activate BC
                        BC-->>EX: receipt | None
                        deactivate BC

                        alt 번들 포함됨
                            EX->>EX: record_successful_sandwich()
                            EX-->>ISM: ExecutionResult{success=true, profit}
                        else 타임아웃
                            EX->>EX: record_failed_sandwich()
                            EX-->>ISM: ExecutionResult{success=false}
                        end
                    end
                    deactivate EX
                end
            end
        end
    end

    loop 통계 출력 루프 (5분마다)
        ISM->>ISM: print_stats()
    end

    User->>ISM: stop()
    ISM->>MM: stop()
    ISM->>SSM: stop()
    ISM->>ISM: print_final_stats()
    ISM-->>User: final_statistics
    deactivate ISM
```

---

## 🔍 Mempool 모니터링 및 타겟 탐지 플로우

### 2️⃣ 실시간 Pending 트랜잭션 감시

```mermaid
sequenceDiagram
    participant MM as MempoolMonitor
    participant WS as WebSocket Provider
    participant BC as Blockchain Node
    participant DRM as DexRouterManager
    participant Channel as mpsc::channel

    MM->>WS: subscribe_pending_txs()
    activate WS
    WS->>BC: subscribe("newPendingTransactions")
    activate BC
    BC-->>WS: subscription_id
    deactivate BC
    WS-->>MM: pending_tx_stream
    deactivate WS

    MM->>MM: start()
    activate MM

    loop 실시간 스트림 처리
        WS->>MM: stream.next() → tx_hash
        MM->>BC: get_transaction(tx_hash)
        activate BC
        BC-->>MM: Option<Transaction>
        deactivate BC

        alt 트랜잭션 존재
            MM->>MM: extract (to, value, gas_price, data)

            alt to 주소 존재
                MM->>DRM: is_dex_router(to)
                activate DRM
                DRM->>DRM: check known_routers hashmap
                DRM-->>MM: bool
                deactivate DRM

                alt DEX 라우터인 경우
                    MM->>DRM: identify_swap_function(data[0..4])
                    activate DRM
                    DRM->>DRM: match function_selector
                    Note over DRM: 0x38ed1739 = swapExactTokensForTokens<br/>0xc04b8d59 = exactInputSingle<br/>etc.
                    DRM-->>MM: Some(dex_type)
                    deactivate DRM

                    alt 스왑 함수 감지
                        MM->>MM: apply filters

                        alt value >= min_value_filter (0.1 ETH)
                            alt gas_price <= max_gas_price (200 Gwei)
                                MM->>MM: create TargetTransaction struct

                                MM->>Channel: send((target_tx, dex_type))
                                activate Channel
                                Channel-->>MM: OK
                                deactivate Channel

                                Note over MM: ✅ 타겟 트랜잭션 발견!
                            else 가스 가격 초과
                                Note over MM: ⚠️ 가스 가격 필터링
                            end
                        else 금액 부족
                            Note over MM: ⚠️ 최소 금액 필터링
                        end
                    else 스왑 함수 아님
                        Note over MM: 무시 (non-swap function)
                    end
                else DEX 라우터 아님
                    Note over MM: 무시 (non-DEX transaction)
                end
            else to 주소 없음 (contract creation)
                Note over MM: 무시
            end
        else 트랜잭션 없음
            Note over MM: pending TX가 이미 mined됨
        end
    end

    MM->>WS: unsubscribe()
    deactivate MM
```

**핵심 포인트**:
- WebSocket으로 **실시간 pending TX** 수신
- 조기 필터링으로 불필요한 처리 제거
- mpsc 채널로 다음 단계에 전달

---

## 📊 Uniswap V2 타겟 분석 플로우

### 3️⃣ Uniswap V2 트랜잭션 디코딩 및 분석

```mermaid
sequenceDiagram
    participant SSM as SandwichStrategyManager
    participant TA as TargetAnalyzer
    participant ABI as ethers::abi
    participant BC as Blockchain
    participant Factory as Uniswap V2 Factory
    participant Pair as Uniswap V2 Pair

    SSM->>TA: analyze(target_tx, DexType::UniswapV2)
    activate TA

    TA->>TA: decode_swap_data(tx.data, UniswapV2)
    activate TA

    TA->>TA: extract function_selector = data[0..4]
    Note over TA: Expected: [0x38, 0xed, 0x17, 0x39]<br/>swapExactTokensForTokens

    TA->>ABI: decode(param_types, data[4..])
    activate ABI
    Note over ABI: ParamTypes:<br/>Uint(256) amountIn<br/>Uint(256) amountOutMin<br/>Array<Address> path<br/>Address to<br/>Uint(256) deadline

    ABI-->>TA: Vec<Token>
    deactivate ABI

    TA->>TA: extract tokens[0] → amountIn
    TA->>TA: extract tokens[1] → amountOutMin
    TA->>TA: extract tokens[2] → path (Vec<Address>)
    TA->>TA: extract tokens[4] → deadline

    TA->>TA: token_in = path[0]
    TA->>TA: token_out = path[path.len() - 1]

    TA-->>TA: DecodedSwap {amountIn, amountOutMin, token_in, token_out, path, deadline}
    deactivate TA

    TA->>TA: estimate_price_impact(amountIn, token_in, token_out)
    activate TA
    Note over TA: 휴리스틱 모델:<br/>&lt;1 ETH → 0.1%<br/>1-10 ETH → 0.5%<br/>10-50 ETH → 2%<br/>&gt;50 ETH → 5%
    TA-->>TA: price_impact (f64)
    deactivate TA

    TA->>TA: get_pool_reserves(token_in, token_out, UniswapV2)
    activate TA

    TA->>Factory: call getPair(token_in, token_out)
    activate Factory
    Note over Factory: Address: 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f<br/>Selector: 0xe6a43905
    Factory-->>TA: pair_address
    deactivate Factory

    alt pair_address != 0x0
        TA->>Pair: call getReserves()
        activate Pair
        Note over Pair: Selector: 0x0902f1ac
        Pair-->>TA: (reserve0, reserve1, blockTimestampLast)
        deactivate Pair

        TA->>TA: order reserves by token addresses
        Note over TA: if token_in < token_out:<br/>  reserve_in = reserve0<br/>else:<br/>  reserve_in = reserve1

        TA->>TA: liquidity = reserve_in + reserve_out
        TA-->>TA: PoolReserves {reserve_in, reserve_out, liquidity}
    else pair not found
        TA-->>TA: Err("Pair does not exist")
    end
    deactivate TA

    TA->>TA: assess_competition_level(gas_price, amountIn, price_impact)
    activate TA
    Note over TA: Logic:<br/>gas > 200 Gwei OR (amount > 100 ETH AND impact > 3%) → Critical<br/>gas > 100 Gwei OR (amount > 50 ETH AND impact > 2%) → High<br/>gas > 50 Gwei OR amount > 10 ETH → Medium<br/>else → Low
    TA-->>TA: competition_level
    deactivate TA

    TA-->>SSM: TargetAnalysis {tx, dex_type, router, tokens, amounts, path, deadline, price_impact, pool_reserves, competition}
    deactivate TA
```

**핵심 디코딩**:
```rust
// Uniswap V2: swapExactTokensForTokens
function_selector: [0x38, 0xed, 0x17, 0x39]

ParamTypes:
- Uint(256)           // amountIn
- Uint(256)           // amountOutMin
- Array<Address>      // path
- Address             // to
- Uint(256)           // deadline
```

---

## 🔄 Uniswap V3 타겟 분석 플로우

### 4️⃣ Uniswap V3 트랜잭션 디코딩 (Tuple 구조)

```mermaid
sequenceDiagram
    participant SSM as SandwichStrategyManager
    participant TA as TargetAnalyzer
    participant ABI as ethers::abi
    participant BC as Blockchain

    SSM->>TA: analyze(target_tx, DexType::UniswapV3)
    activate TA

    TA->>TA: decode_swap_data(tx.data, UniswapV3)
    activate TA

    TA->>TA: extract function_selector = data[0..4]
    Note over TA: Expected: [0xc0, 0x4b, 0x8d, 0x59]<br/>exactInputSingle

    TA->>ABI: decode(param_types, data[4..])
    activate ABI
    Note over ABI: ParamTypes:<br/>Tuple(<br/>  Address tokenIn,<br/>  Address tokenOut,<br/>  Uint(24) fee,<br/>  Address recipient,<br/>  Uint(256) deadline,<br/>  Uint(256) amountIn,<br/>  Uint(256) amountOutMinimum,<br/>  Uint(160) sqrtPriceLimitX96<br/>)

    ABI-->>TA: Vec<Token> (1개 Tuple)
    deactivate ABI

    TA->>TA: extract tuple_tokens = tokens[0]
    TA->>TA: token_in = tuple_tokens[0]
    TA->>TA: token_out = tuple_tokens[1]
    TA->>TA: fee = tuple_tokens[2] (e.g., 3000 = 0.3%)
    TA->>TA: deadline = tuple_tokens[4]
    TA->>TA: amountIn = tuple_tokens[5]
    TA->>TA: amountOutMin = tuple_tokens[6]

    TA-->>TA: DecodedSwap {amountIn, amountOutMin, token_in, token_out, path: [token_in, token_out], deadline}
    deactivate TA

    TA->>TA: estimate_price_impact(amountIn, token_in, token_out)
    activate TA
    Note over TA: V3는 집중 유동성으로<br/>가격 영향이 더 클 수 있음:<br/>&lt;1 ETH → 0.2%<br/>1-10 ETH → 1%<br/>&gt;10 ETH → 3%
    TA-->>TA: price_impact (f64)
    deactivate TA

    TA->>TA: assess_competition_level(gas_price, amountIn, price_impact)
    TA-->>SSM: TargetAnalysis {tx, dex_type, router, tokens, amounts, path, deadline, price_impact, pool_reserves: None, competition}
    deactivate TA
```

**핵심 차이점**:
- Uniswap V2: 평면 파라미터 구조
- Uniswap V3: **Tuple 파라미터** (8개 필드)
- V3는 `fee` 필드로 pool 구분 (500/3000/10000 bps)

---

## 🧮 Kelly Criterion 포지션 계산 플로우

### 5️⃣ 수학적 최적 포지션 크기 결정

```mermaid
sequenceDiagram
    participant SSM as SandwichStrategyManager
    participant PA as ProfitabilityAnalyzer
    participant BC as Blockchain
    participant Kelly as Kelly Calculator

    SSM->>PA: analyze_opportunity(target_analysis, current_gas_price)
    activate PA

    PA->>PA: check price_impact <= max_price_impact (5%)
    alt price_impact > 5%
        PA-->>SSM: None (필터링)
    end

    PA->>Kelly: calculate_kelly_criterion(params)
    activate Kelly
    Note over Kelly: Input:<br/>success_probability = competition.success_probability()<br/>  Low: 85%, Medium: 70%, High: 50%, Critical: 30%<br/>price_impact_bps = price_impact * 10000<br/>available_capital = target.amount_in * 2<br/>risk_factor = 0.5 (Half Kelly)

    Kelly->>Kelly: p = success_probability
    Kelly->>Kelly: q = 1.0 - p
    Kelly->>Kelly: b = price_impact_bps / 10000.0

    Kelly->>Kelly: kelly_fraction = (p * b - q) / b
    Note over Kelly: Kelly Formula:<br/>f* = (p * b - q) / b

    alt p * b <= q (기대값 음수)
        Kelly-->>PA: Err("Expected value negative")
        PA-->>SSM: None
    end

    Kelly->>Kelly: adjusted_kelly = kelly_fraction * risk_factor
    Note over Kelly: Half Kelly (0.5x)<br/>변동성 75% 감소

    Kelly->>Kelly: clamped_kelly = max(0.01, min(0.25, adjusted_kelly))
    Note over Kelly: 포지션 제한:<br/>최소 1%, 최대 25%

    Kelly->>Kelly: optimal_size = available_capital * clamped_kelly
    Kelly->>Kelly: expected_value = p * b - q * b
    Kelly->>Kelly: risk_of_ruin = (q / p)^(optimal_size / available_capital)

    Kelly-->>PA: KellyCriterionResult {optimal_size, kelly_percentage, expected_value, risk_of_ruin}
    deactivate Kelly

    PA->>PA: front_run_amount = kelly_result.optimal_size
    alt front_run_amount == 0
        PA-->>SSM: None
    end

    PA->>PA: estimate_profit(front_run_amount, target.amount_in, price_impact, dex_type)
    activate PA
    Note over PA: Profit Model:<br/>profit_from_target = front_run_eth * price_impact<br/>dex_fees = front_run_eth * 0.003 * 2 (0.3% * 2번 스왑)<br/>net_profit_eth = profit_from_target - dex_fees
    PA-->>PA: estimated_profit (U256)
    deactivate PA

    PA->>PA: estimate_gas_cost(current_gas_price, competition)
    activate PA
    Note over PA: Gas Model:<br/>gas_per_tx = 200,000<br/>total_gas = gas_per_tx * 2<br/>gas_multiplier = competition.gas_multiplier()<br/>priority_fee = competition-based (1-10 Gwei)<br/>total_cost = (base_fee + priority_fee) * total_gas
    PA-->>PA: gas_cost (U256)
    deactivate PA

    PA->>PA: net_profit = estimated_profit - gas_cost
    alt net_profit <= 0
        PA-->>SSM: None
    end

    PA->>PA: check net_profit >= min_profit_wei (0.01 ETH)
    alt net_profit < min_profit
        PA-->>SSM: None
    end

    PA->>PA: profit_percentage = net_profit / front_run_amount
    PA->>PA: check profit_percentage >= min_profit_percentage (2%)
    alt profit_percentage < 2%
        PA-->>SSM: None
    end

    PA->>PA: create SandwichOpportunity
    PA-->>SSM: Some(opportunity)
    deactivate PA
```

**Kelly Criterion 예시**:
```
입력:
- p (성공 확률) = 0.7 (70%)
- b (가격 영향) = 0.025 (2.5%)
- available_capital = 10 ETH
- risk_factor = 0.5

계산:
- q = 1 - 0.7 = 0.3
- kelly_fraction = (0.7 * 0.025 - 0.3) / 0.025
                 = (0.0175 - 0.3) / 0.025
                 = -11.3 (음수!)

→ p * b < q이므로 기대값이 음수
→ 투자하지 않음 (None 반환)

올바른 예시 (b = 수익률로 해석):
- b = 0.30 (30% 수익률)
- kelly_fraction = (0.7 * 0.30 - 0.3) / 0.30
                 = (0.21 - 0.3) / 0.30
                 = -0.3 (여전히 음수...)

실제 수익성 예시:
- p = 0.85 (85% 성공 확률, Low competition)
- b = 0.05 (5% 수익률)
- kelly_fraction = (0.85 * 0.05 - 0.15) / 0.05
                 = (0.0425 - 0.15) / 0.05
                 = -2.15 (음수)

→ 샌드위치 공격은 수익률이 매우 낮아서
   Kelly Criterion으로는 대부분 음수가 나옴!
→ 실전에서는 price_impact를 다르게 해석하거나
   고정 비율(예: 타겟의 10-20%) 사용
```

---

## 📦 MEV 번들 생성 및 제출 플로우

### 6️⃣ Front-run/Back-run 트랜잭션 구성

```mermaid
sequenceDiagram
    participant ISM as IntegratedSandwichManager
    participant BB as BundleBuilder
    participant ABI as ABI Encoder
    participant EX as SandwichExecutor
    participant Wallet as LocalWallet
    participant FB as Flashbots Relay
    participant BC as Blockchain

    ISM->>BB: build_bundle(opportunity, block_number)
    activate BB

    BB->>ABI: encode_swap(token_in, token_out, front_run_amount, 0, path)
    activate ABI
    Note over ABI: Function: swapExactTokensForTokens<br/>Params: (amountIn, amountOutMin=0, path, to, deadline)
    ABI-->>BB: front_run_calldata (Bytes)
    deactivate ABI

    BB->>ABI: encode_swap(token_out, token_in, back_run_amount, expected_out, reverse_path)
    activate ABI
    Note over ABI: 반대 방향 스왑<br/>amountOutMin = expected_amount_out (수익 보장)
    ABI-->>BB: back_run_calldata (Bytes)
    deactivate ABI

    BB->>BC: get_gas_price()
    activate BC
    BC-->>BB: base_gas_price
    deactivate BC

    BB->>BB: calculate gas prices
    Note over BB: competition = opportunity.competition_level<br/>front_run_gas_price = base * competition.gas_multiplier()<br/>  Low: 1.1x, Medium: 1.3x, High: 1.6x, Critical: 2.0x<br/>back_run_gas_price = base * 1.1

    BB->>BB: calculate bundle_hash
    Note over BB: keccak256(front_run || target_tx_hash || back_run)

    BB-->>ISM: SandwichBundle {front_run_tx, back_run_tx, gas_prices, target_block, bundle_hash, estimated_profit, gas_cost, net_profit}
    deactivate BB

    ISM->>EX: execute_bundle(bundle)
    activate EX

    EX->>EX: build_and_sign_transaction(front_run_calldata, target_block, is_front_run=true)
    activate EX

    EX->>BC: get_transaction_count(wallet.address(), Pending)
    activate BC
    BC-->>EX: nonce
    deactivate BC

    EX->>BC: get_gas_price()
    activate BC
    BC-->>EX: base_fee
    deactivate BC

    EX->>EX: priority_fee = 5 Gwei (front-run용 높은 우선순위)
    EX->>EX: create EIP-1559 transaction
    Note over EX: Eip1559TransactionRequest:<br/>  to: contract_address<br/>  data: calldata<br/>  nonce: nonce<br/>  gas: 200,000<br/>  max_fee_per_gas: base_fee + priority_fee<br/>  max_priority_fee_per_gas: priority_fee<br/>  chain_id: wallet.chain_id()

    EX->>Wallet: sign_transaction(typed_tx)
    activate Wallet
    Wallet-->>EX: signature
    deactivate Wallet

    EX->>EX: rlp_signed(&signature)
    EX-->>EX: signed_front_run_tx
    deactivate EX

    EX->>EX: build_and_sign_transaction(back_run_calldata, target_block, is_front_run=false)
    activate EX
    Note over EX: nonce = nonce + 1<br/>priority_fee = 2 Gwei (낮은 우선순위)
    EX-->>EX: signed_back_run_tx
    deactivate EX

    EX->>FB: submit_flashbots_bundle(signed_txs, target_block)
    activate FB

    FB->>FB: create JSON-RPC request
    Note over FB: {\n  "jsonrpc": "2.0",\n  "method": "eth_sendBundle",\n  "params": [{\n    "txs": [\n      "0x..." (front_run RLP),\n      "0x..." (victim tx hash),\n      "0x..." (back_run RLP)\n    ],\n    "blockNumber": "0x..." (target_block),\n    "minTimestamp": 0,\n    "maxTimestamp": 0\n  }],\n  "id": 1\n}

    FB->>BC: HTTP POST to https://relay.flashbots.net
    activate BC
    BC-->>FB: {"result": {"bundleHash": "0x..."}}
    deactivate BC

    FB-->>EX: (front_run_hash, back_run_hash)
    deactivate FB

    EX-->>ISM: (front_run_hash, back_run_hash)
    deactivate EX
```

**번들 구조**:
```json
{
  "txs": [
    "0xf86c80...",  // Front-run TX (RLP encoded)
    "0xabc123...",  // Victim TX hash (original)
    "0xf86d01..."   // Back-run TX (RLP encoded)
  ],
  "blockNumber": "0x1122334",
  "minTimestamp": 0,
  "maxTimestamp": 0
}
```

---

## ⚖️ 경쟁 수준 평가 및 가스 최적화 플로우

### 7️⃣ 경쟁 분석 및 동적 가스 조정

```mermaid
sequenceDiagram
    participant TA as TargetAnalyzer
    participant BC as Blockchain
    participant Comp as Competition Analyzer

    TA->>Comp: assess_competition_level(gas_price, amount_in, price_impact)
    activate Comp

    Comp->>Comp: gas_gwei = gas_price / 1_000_000_000
    Comp->>Comp: amount_eth = amount_in / 1e18

    Comp->>Comp: evaluate conditions
    Note over Comp: Critical:<br/>  gas > 200 Gwei OR<br/>  (amount > 100 ETH AND impact > 3%)<br/><br/>High:<br/>  gas > 100 Gwei OR<br/>  (amount > 50 ETH AND impact > 2%)<br/><br/>Medium:<br/>  gas > 50 Gwei OR<br/>  amount > 10 ETH<br/><br/>Low:<br/>  else

    alt gas_gwei > 200 OR (amount_eth > 100 AND price_impact > 0.03)
        Comp-->>TA: CompetitionLevel::Critical
        Note over TA: Success Probability: 30%<br/>Gas Multiplier: 2.0x<br/>Priority Fee: 10 Gwei
    else gas_gwei > 100 OR (amount_eth > 50 AND price_impact > 0.02)
        Comp-->>TA: CompetitionLevel::High
        Note over TA: Success Probability: 50%<br/>Gas Multiplier: 1.6x<br/>Priority Fee: 5 Gwei
    else gas_gwei > 50 OR amount_eth > 10
        Comp-->>TA: CompetitionLevel::Medium
        Note over TA: Success Probability: 70%<br/>Gas Multiplier: 1.3x<br/>Priority Fee: 2 Gwei
    else
        Comp-->>TA: CompetitionLevel::Low
        Note over TA: Success Probability: 85%<br/>Gas Multiplier: 1.1x<br/>Priority Fee: 1 Gwei
    end
    deactivate Comp

    TA->>TA: store competition_level in TargetAnalysis
    TA-->>TA: analysis with competition assessment
```

**경쟁 수준 매트릭스**:

| 경쟁 | 조건 | 성공률 | Gas 배수 | Priority Fee | 최소 수익 |
|-----|------|--------|----------|--------------|----------|
| **Low** | gas ≤ 50 Gwei<br/>amount ≤ 10 ETH | 85% | 1.1x | 1 Gwei | 0.01 ETH |
| **Medium** | gas > 50 Gwei<br/>OR amount > 10 ETH | 70% | 1.3x | 2 Gwei | 0.02 ETH |
| **High** | gas > 100 Gwei<br/>OR (amount > 50 ETH AND impact > 2%) | 50% | 1.6x | 5 Gwei | 0.05 ETH |
| **Critical** | gas > 200 Gwei<br/>OR (amount > 100 ETH AND impact > 3%) | 30% | 2.0x | 10 Gwei | 0.1 ETH |

---

## 🏊 Pool Reserves 조회 플로우

### 8️⃣ Uniswap V2 Factory → Pair → Reserves

```mermaid
sequenceDiagram
    participant TA as TargetAnalyzer
    participant Factory as Uniswap V2 Factory
    participant Pair as Uniswap V2 Pair
    participant ABI as ABI Decoder
    participant BC as Blockchain Provider

    TA->>TA: get_pool_reserves(token_in, token_out, DexType::UniswapV2)
    activate TA

    TA->>TA: factory_address = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
    Note over TA: Uniswap V2 Factory

    TA->>ABI: encode getPair(token_in, token_out)
    activate ABI
    Note over ABI: Selector: 0xe6a43905<br/>keccak256("getPair(address,address)")[:4]
    ABI-->>TA: get_pair_calldata
    deactivate ABI

    TA->>BC: call(factory_address, get_pair_calldata)
    activate BC
    BC->>Factory: eth_call getPair
    activate Factory
    Factory-->>BC: pair_address (32 bytes)
    deactivate Factory
    BC-->>TA: result (Bytes)
    deactivate BC

    TA->>TA: extract pair_address = Address::from_slice(&result[12..32])
    alt pair_address == 0x0
        TA-->>TA: Err("Pair does not exist")
    end

    TA->>ABI: encode getReserves()
    activate ABI
    Note over ABI: Selector: 0x0902f1ac<br/>keccak256("getReserves()")[:4]
    ABI-->>TA: get_reserves_calldata
    deactivate ABI

    TA->>BC: call(pair_address, get_reserves_calldata)
    activate BC
    BC->>Pair: eth_call getReserves
    activate Pair
    Note over Pair: Returns:<br/>(uint112 reserve0,<br/> uint112 reserve1,<br/> uint32 blockTimestampLast)
    Pair-->>BC: (reserve0, reserve1, timestamp)
    deactivate Pair
    BC-->>TA: result (Bytes)
    deactivate BC

    TA->>ABI: decode([Uint(112), Uint(112), Uint(32)], result)
    activate ABI
    ABI-->>TA: [Token::Uint(reserve0), Token::Uint(reserve1), Token::Uint(timestamp)]
    deactivate ABI

    TA->>TA: order reserves by token addresses
    Note over TA: Uniswap V2 sorts tokens:<br/>if token_in < token_out:<br/>  reserve_in = reserve0<br/>  reserve_out = reserve1<br/>else:<br/>  reserve_in = reserve1<br/>  reserve_out = reserve0

    TA->>TA: liquidity = reserve_in + reserve_out

    TA-->>TA: PoolReserves {reserve_in, reserve_out, liquidity}
    deactivate TA
```

**Function Selectors**:
```solidity
// Uniswap V2 Factory
function getPair(address tokenA, address tokenB)
    external view returns (address pair);
// Selector: 0xe6a43905

// Uniswap V2 Pair
function getReserves()
    external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
// Selector: 0x0902f1ac
```

---

## ⚡ Flashbots 실행 및 확인 플로우

### 9️⃣ 번들 제출 및 포함 확인

```mermaid
sequenceDiagram
    participant EX as SandwichExecutor
    participant FB as Flashbots Relay
    participant BC as Blockchain
    participant Stats as StatsManager

    EX->>EX: execute_bundle(bundle)
    activate EX

    EX->>Stats: record_bundle_submitted()
    activate Stats
    Stats-->>EX: OK
    deactivate Stats

    EX->>BC: get_block_number()
    activate BC
    BC-->>EX: current_block
    deactivate BC
    EX->>EX: target_block = current_block + 1

    EX->>EX: build and sign transactions
    EX->>FB: submit_flashbots_bundle(signed_txs, target_block)
    activate FB

    FB->>FB: create eth_sendBundle request
    FB->>BC: HTTP POST to relay.flashbots.net
    activate BC
    BC-->>FB: {"result": {"bundleHash": "0x..."}}
    deactivate BC

    FB-->>EX: (front_run_hash, back_run_hash)
    deactivate FB

    EX->>EX: wait_for_bundle_inclusion(front_run_hash, target_block)
    activate EX

    EX->>BC: get_block_number()
    activate BC
    BC-->>EX: current_block
    deactivate BC

    loop max_wait_blocks = 3 (최대 3블록 대기)
        EX->>BC: get_transaction_receipt(front_run_hash)
        activate BC
        BC-->>EX: Option<TransactionReceipt>
        deactivate BC

        alt receipt exists
            alt receipt.status == 1
                EX->>EX: block_number = receipt.block_number
                Note over EX: ✅ 트랜잭션 포함 확인!

                EX->>EX: calculate actual_profit, actual_gas_cost
                EX->>Stats: record_successful_sandwich(profit, gas)
                activate Stats
                Stats-->>EX: OK
                deactivate Stats

                EX->>Stats: record_bundle_included()
                activate Stats
                Stats-->>EX: OK
                deactivate Stats

                EX-->>EX: SandwichExecutionResult {success=true, profit, gas_cost, net_profit, block_number}
            else receipt.status == 0
                Note over EX: ❌ 트랜잭션 실패

                EX->>Stats: record_failed_sandwich()
                activate Stats
                Stats-->>EX: OK
                deactivate Stats

                EX-->>EX: SandwichExecutionResult {success=false, error="Transaction failed"}
            end
        else receipt not found
            EX->>EX: wait 3 seconds
            EX->>BC: get_block_number()
            activate BC
            BC-->>EX: current_block
            deactivate BC

            alt current_block > target_block + max_wait_blocks
                Note over EX: ⏱️ 타임아웃 (번들 포함 안됨)

                EX->>Stats: record_failed_sandwich()
                activate Stats
                Stats-->>EX: OK
                deactivate Stats

                EX-->>EX: SandwichExecutionResult {success=false, error="Bundle not included"}
            end
        end
    end
    deactivate EX

    EX-->>EX: execution_result
    deactivate EX
```

**확인 로직**:
```rust
// 최대 3블록 동안 대기
let max_wait_blocks = 3;
let mut current_block = provider.get_block_number().await?;

while current_block <= target_block + max_wait_blocks {
    // 트랜잭션 영수증 확인
    if let Some(receipt) = provider.get_transaction_receipt(tx_hash).await? {
        if receipt.status == Some(1.into()) {
            return Ok(true);  // 성공
        } else {
            return Ok(false); // 실패
        }
    }

    tokio::time::sleep(Duration::from_secs(3)).await;
    current_block = provider.get_block_number().await?;
}

Ok(false) // 타임아웃
```

---

## 🚨 에러 처리 및 복구 플로우

### 🔟 다양한 실패 시나리오 처리

```mermaid
sequenceDiagram
    participant ISM as IntegratedSandwichManager
    participant MM as MempoolMonitor
    participant TA as TargetAnalyzer
    participant PA as ProfitabilityAnalyzer
    participant EX as SandwichExecutor
    participant BC as Blockchain
    participant Stats as StatsManager

    rect rgb(255, 230, 230)
        Note over ISM,Stats: 시나리오 1: WebSocket 연결 끊김
        MM->>BC: subscribe_pending_txs()
        BC-->>MM: Error(Connection lost)
        MM->>MM: retry connection (exponential backoff)
        alt retry success
            BC-->>MM: new stream
            MM->>MM: resume monitoring
        else retry failed after 5 attempts
            MM->>ISM: Err("WebSocket connection failed")
            ISM->>ISM: log error, wait 60s, restart MempoolMonitor
        end
    end

    rect rgb(255, 240, 200)
        Note over ISM,Stats: 시나리오 2: ABI 디코딩 실패
        TA->>TA: decode_swap_data(tx.data, dex_type)
        TA-->>TA: Err("Invalid amountIn")
        Note over TA: ❌ 알 수 없는 함수 selector 또는<br/>파라미터 타입 불일치
        TA->>TA: log warning with tx_hash
        TA-->>ISM: Err("ABI decode failed")
        ISM->>ISM: skip this opportunity, continue
    end

    rect rgb(230, 240, 255)
        Note over ISM,Stats: 시나리오 3: Pool Reserves 조회 실패
        TA->>BC: call Factory.getPair(token_in, token_out)
        BC-->>TA: pair_address = 0x0
        Note over TA: ❌ Pair가 존재하지 않음
        TA-->>ISM: Err("Pair does not exist")
        ISM->>ISM: skip opportunity, log warning
    end

    rect rgb(240, 255, 230)
        Note over ISM,Stats: 시나리오 4: Kelly Criterion 음수 (기대값 음수)
        PA->>PA: calculate_kelly_criterion(params)
        PA->>PA: kelly_fraction = (p * b - q) / b
        alt p * b <= q
            Note over PA: 기대값 음수: 투자하지 않음
            PA-->>ISM: None
            ISM->>ISM: skip opportunity, log "negative expected value"
        end
    end

    rect rgb(255, 230, 255)
        Note over ISM,Stats: 시나리오 5: 최소 수익 미달
        PA->>PA: net_profit = estimated_profit - gas_cost
        alt net_profit < min_profit_wei (0.01 ETH)
            Note over PA: ❌ 최소 수익 미달
            PA-->>ISM: None
            ISM->>ISM: skip opportunity, log "profit too low"
        end
    end

    rect rgb(230, 255, 255)
        Note over ISM,Stats: 시나리오 6: Flashbots 제출 실패
        EX->>BC: HTTP POST to relay.flashbots.net
        BC-->>EX: Error 500 (Internal Server Error)
        EX->>EX: log error, mark as failed
        EX->>Stats: record_failed_sandwich()
        EX-->>ISM: ExecutionResult {success=false, error="Flashbots submission failed"}
        ISM->>ISM: log failure, continue
    end

    rect rgb(255, 245, 230)
        Note over ISM,Stats: 시나리오 7: 번들이 포함되지 않음 (경쟁 패배)
        EX->>BC: get_transaction_receipt(front_run_hash)
        loop wait 3 blocks
            BC-->>EX: None
        end
        Note over EX: ⏱️ 타임아웃: 번들이 포함되지 않음
        EX->>Stats: record_failed_sandwich()
        EX-->>ISM: ExecutionResult {success=false, error="Bundle not included"}
        ISM->>ISM: log competition loss, continue
    end

    rect rgb(255, 230, 240)
        Note over ISM,Stats: 시나리오 8: 트랜잭션 revert (슬리피지 초과)
        EX->>BC: get_transaction_receipt(front_run_hash)
        BC-->>EX: receipt {status=0}
        Note over EX: ❌ 트랜잭션 실패 (revert)
        EX->>Stats: record_failed_sandwich()
        EX-->>ISM: ExecutionResult {success=false, error="Transaction reverted"}
        ISM->>ISM: log revert reason, adjust strategy
    end

    rect rgb(240, 240, 240)
        Note over ISM,Stats: 시나리오 9: 전체 시스템 재시작
        ISM->>ISM: detect critical error (e.g., repeated failures)
        ISM->>MM: stop()
        ISM->>ISM: cleanup resources
        ISM->>ISM: wait 120 seconds
        ISM->>ISM: restart all components
        ISM->>MM: start()
        ISM->>ISM: resume operation
    end
```

**에러 복구 전략**:

| 에러 유형 | 복구 방법 | 재시도 | 영향 |
|----------|----------|--------|------|
| WebSocket 끊김 | Exponential backoff 재연결 | 최대 5회 | 일시적 모니터링 중단 |
| ABI 디코딩 실패 | 기회 스킵, 로그 기록 | 없음 | 개별 기회 손실 |
| Pool 없음 | 기회 스킵, 경고 로그 | 없음 | 개별 기회 손실 |
| Kelly 음수 | 기회 스킵, 기대값 로그 | 없음 | 정상 필터링 |
| 최소 수익 미달 | 기회 스킵 | 없음 | 정상 필터링 |
| Flashbots 실패 | 실패 기록, 다음 기회 진행 | 없음 | 개별 실행 손실 |
| 번들 미포함 | 실패 기록, 경쟁 분석 | 없음 | 경쟁 패배 |
| TX revert | 실패 기록, 전략 조정 | 없음 | 슬리피지 또는 경쟁 |
| 반복 실패 | 전체 시스템 재시작 | 1회 | 일시적 중단 |

---

## 📊 통계 및 모니터링

### 실시간 통계 출력 (5분마다)

```
════════════════════════════════════════════════════
📊 샌드위치 전략 통계
════════════════════════════════════════════════════

🎯 기회 분석:
   총 감지: 1,234
   수익성 있음: 56 (4.5%)

📦 번들 제출:
   총 제출: 56
   포함됨: 42 (75%)
   실패: 14 (25%)

✅ 성공한 샌드위치:
   총 성공: 42
   성공률: 75%

💰 수익 통계:
   총 수익: 1.245 ETH
   총 가스 비용: 0.234 ETH
   순이익: 1.011 ETH
   평균 수익/샌드위치: 0.0241 ETH
   평균 가스/샌드위치: 0.0056 ETH
   평균 순이익/샌드위치: 0.0185 ETH

📈 ROI:
   75%

⚡ 성능:
   평균 실행 시간: 1,234 ms

════════════════════════════════════════════════════
```

---

**마지막 업데이트**: 2025-01-XX
**버전**: 1.0.0
**작성자**: xCrack Development Team
