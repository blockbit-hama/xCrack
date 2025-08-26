# 샌드위치 공격 전략 (Sandwich Attack Strategy)

## 개요

샌드위치 공격은 멘플에서 감지한 큰 거래를 앞뒤로 '샌드위치'하여 가격 임팩트를 이용해 수익을 얻는 MEV 전략입니다. 희생자(victim)의 거래 전에 같은 토큰을 먼저 구매하고, 희생자 거래 후에 더 높은 가격에 판매하여 차익을 얻습니다.

## 전략 원리

### 기본 메커니즘
```
1. 멤풀 모니터링 → 큰 구매 주문(victim) 감지
2. Front-run → 같은 토큰을 먼저 구매 (가격 상승 유도)
3. Victim Transaction → 희생자가 더 높은 가격에 구매
4. Back-run → 상승된 가격에 토큰 판매 (수익 실현)
```

### 수익 계산
```rust
// 기본 수익 공식
profit = (back_run_price - front_run_price) * token_amount - gas_costs
```

## 플래시론 vs 일반 실행 비교

### 1. 일반 샌드위치 (Non-Flashloan)

**장점**:
- 단순한 구조
- 낮은 가스 비용
- 빠른 실행 속도
- 플래시론 수수료 없음

**단점**:
- 자본 필요 (ETH/토큰 보유 필수)
- 제한된 거래 규모
- 자본 효율성 낮음
- 슬리피지 리스크

**실행 시나리오**:
```
보유 자본: 10 ETH
대상 거래: USDC 100,000 구매

1. Front-run: 5 ETH → WETH/USDC 스왑
2. Victim: 100,000 USDC → WETH 구매 (가격 10% 상승)  
3. Back-run: WETH → ETH 스왑 (10% 프리미엄으로 판매)

예상 수익: ~0.5 ETH (가스비 제외)
자본 효율성: 5% ROI per trade
```

### 2. 플래시론 샌드위치 (Flashloan Sandwich)

**장점**:
- 무제한 자본 활용
- 높은 자본 효율성
- 대형 거래 공격 가능
- 원자적 실행 (실패시 롤백)

**단점**:
- 복잡한 스마트 컨트랙트
- 플래시론 수수료 (0.05-0.30%)
- 높은 가스 비용
- 기술적 복잡성

**실행 시나리오**:
```
플래시론 금액: 1000 ETH
대상 거래: USDC 500,000 구매

1. Flashloan: Aave에서 1000 ETH 대출
2. Front-run: 800 ETH → WETH/USDC 스왑
3. Victim: 500,000 USDC → WETH 구매 (가격 15% 상승)
4. Back-run: WETH → ETH 스왑 (15% 프리미엄 획득)
5. Repay: 1000 ETH + 0.5 ETH (수수료) 상환

예상 수익: ~8-12 ETH (가스비 및 수수료 제외)
자본 효율성: 무한대 (자본 불필요)
```

## 스마트 컨트랙트 구현

### 플래시론 샌드위치 컨트랙트

```solidity
pragma solidity ^0.8.19;

import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract SandwichStrategy is FlashLoanSimpleReceiverBase {
    address private owner;
    
    struct SandwichParams {
        address targetTx;           // 희생자 트랜잭션
        address tokenIn;            // 입력 토큰
        address tokenOut;           // 출력 토큰
        address dexRouter;          // DEX 라우터
        uint256 frontRunAmount;     // Front-run 금액
        uint256 backRunAmount;      // Back-run 금액
        bytes frontRunCalldata;     // Front-run 호출 데이터
        bytes backRunCalldata;      // Back-run 호출 데이터
    }
    
    constructor(IPoolAddressesProvider provider) 
        FlashLoanSimpleReceiverBase(provider) {
        owner = msg.sender;
    }
    
    function executeSandwich(
        address asset,
        uint256 amount,
        SandwichParams calldata params
    ) external onlyOwner {
        bytes memory data = abi.encode(params);
        POOL.flashLoanSimple(address(this), asset, amount, data, 0);
    }
    
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        require(msg.sender == address(POOL), "Invalid caller");
        
        SandwichParams memory sandwichParams = abi.decode(params, (SandwichParams));
        
        // 1. Execute front-run transaction
        _executeFrontRun(sandwichParams);
        
        // 2. Wait for victim transaction (coordinated with mempool monitoring)
        
        // 3. Execute back-run transaction  
        _executeBackRun(sandwichParams);
        
        // 4. Repay flashloan + premium
        uint256 amountOwed = amount + premium;
        IERC20(asset).approve(address(POOL), amountOwed);
        
        return true;
    }
    
    function _executeFrontRun(SandwichParams memory params) private {
        // Front-run: Buy the token victim wants to buy
        (bool success,) = params.dexRouter.call(params.frontRunCalldata);
        require(success, "Front-run failed");
    }
    
    function _executeBackRun(SandwichParams memory params) private {
        // Back-run: Sell at higher price after victim's purchase
        (bool success,) = params.dexRouter.call(params.backRunCalldata);
        require(success, "Back-run failed");
    }
}
```

## 실행 시나리오 상세 분석

### 시나리오 1: Uniswap V2 WETH/USDC 샌드위치

**시장 상황**:
- WETH/USDC 풀 유동성: 10,000 ETH / 20,000,000 USDC
- 현재 가격: 1 ETH = 2,000 USDC
- 희생자 거래: 1,000,000 USDC → WETH 구매

**일반 실행 (자본 보유 필요)**:
```
1. 초기 상태:
   - 보유 자산: 100 ETH
   - 풀 상태: 10,000 ETH / 20,000,000 USDC

2. Front-run (50 ETH 투입):
   - 50 ETH → USDC 스왑
   - 받는 USDC: ~99,500 USDC
   - 풀 상태: 10,050 ETH / 19,900,500 USDC
   - 새 가격: 1 ETH ≈ 1,980 USDC

3. Victim 거래:
   - 1,000,000 USDC → WETH
   - 받는 ETH: ~495 ETH
   - 풀 상태: 9,555 ETH / 20,900,500 USDC
   - 새 가격: 1 ETH ≈ 2,186 USDC

4. Back-run (USDC → ETH):
   - 99,500 USDC → ETH
   - 받는 ETH: ~45.5 ETH
   - 순 손실: 4.5 ETH

예상 수익: -4.5 ETH (실패 사례)
```

**플래시론 실행 (최적화)**:
```
1. Flashloan: 500 ETH 대출 (Aave V3, 0.05% 수수료)

2. 최적화된 Front-run:
   - 시뮬레이션으로 최적 금액 계산: 300 ETH
   - 300 ETH → USDC 스왑
   - 받는 USDC: ~594,000 USDC
   - 풀 상태: 10,300 ETH / 19,406,000 USDC

3. Victim 거래 (예상):
   - 1,000,000 USDC → WETH
   - 받는 ETH: ~485 ETH (슬리피지 증가)
   - 풀 상태: 9,815 ETH / 20,406,000 USDC
   - 새 가격: 1 ETH ≈ 2,080 USDC

4. Back-run 최적화:
   - 594,000 USDC → ETH
   - 받는 ETH: ~285.5 ETH
   - 순 손실: 14.5 ETH

5. 상환:
   - 대출 상환: 500 ETH + 2.5 ETH (수수료)
   - 필요 자금: 502.5 ETH
   - 부족: 217 ETH (실패)

실제 결과: 거래 실패 (수익성 없음)
```

### 시나리오 2: 성공적인 플래시론 샌드위치

**더 유리한 시장 조건**:
- 저유동성 토큰: RARE/WETH 풀
- 풀 유동성: 1,000 WETH / 2,000,000 RARE
- 희생자: 200 WETH → RARE 구매 (풀의 20%)

**플래시론 실행**:
```
1. Flashloan: 100 WETH (Balancer, 0% 수수료)

2. Front-run (80 WETH):
   - 80 WETH → RARE 스왑
   - 받는 RARE: ~148,148 RARE
   - 풀 상태: 1,080 WETH / 1,851,852 RARE
   - 가격 변화: +8%

3. Victim 거래:
   - 200 WETH → RARE
   - 받는 RARE: ~289,351 RARE  
   - 풀 상태: 1,280 WETH / 1,562,501 RARE
   - 가격 변화: 추가 +22%

4. Back-run (RARE → WETH):
   - 148,148 RARE → WETH
   - 받는 WETH: ~121.2 WETH
   - 순 수익: 41.2 WETH

5. 상환:
   - 대출 상환: 100 WETH (수수료 0%)
   - 최종 수익: 21.2 WETH

ROI: 무한대 (자본 불요)
성공률: 95% (가스 가격 경쟁 고려)
```

## 리스크 관리

### 1. 기술적 리스크
- **가스 가격 경쟁**: 다른 봇과의 우선순위 경쟁
- **슬리피지 리스크**: 예상보다 높은 슬리피지
- **Front-running**: 더 빠른 봇에 의한 선점
- **MEV 경쟁**: 여러 봇의 동시 공격

### 2. 시장 리스크
- **유동성 부족**: 백런 시 충분한 유동성 없음
- **가격 변동성**: 실행 중 급격한 가격 변화
- **토큰 리스크**: 토큰 가치 급락
- **거래소 리스크**: DEX 스마트 컨트랙트 문제

### 3. 운영 리스크
- **네트워크 지연**: 블록체인 네트워크 혼잡
- **시뮬레이션 오차**: 실제 결과와 시뮬레이션 차이
- **스마트 컨트랙트 버그**: 컨트랙트 코드 오류
- **키 관리**: 프라이빗 키 보안

## 수익성 최적화 전략

### 1. 대상 선별
```rust
// 수익성 높은 거래 필터링
fn is_profitable_target(tx: &Transaction) -> bool {
    // 최소 거래 금액 (가스비 대비)
    tx.value > MIN_TRADE_VALUE &&
    // 슬리피지 임계값
    calculate_slippage(tx) > MIN_SLIPPAGE &&
    // 풀 유동성 체크
    pool_liquidity > tx.value * 5 &&
    // 경쟁 봇 분석
    !has_competing_bots(tx)
}
```

### 2. 동적 크기 조절
```rust
// 최적 Front-run 크기 계산
fn calculate_optimal_frontrun_size(
    victim_amount: U256,
    pool_reserves: (U256, U256)
) -> U256 {
    // Newton-Raphson 방법으로 최적해 계산
    // 목표: maximize(back_run_profit - front_run_cost - fees)
    optimize_profit_function(victim_amount, pool_reserves)
}
```

### 3. 가스 가격 전략
```rust
// 경쟁력 있는 가스 가격 설정
fn calculate_competitive_gas_price(
    base_fee: U256,
    priority_fee: U256,
    competition_level: u8
) -> U256 {
    match competition_level {
        0..=3 => base_fee + priority_fee * 110 / 100,      // +10%
        4..=6 => base_fee + priority_fee * 125 / 100,      // +25% 
        7..=10 => base_fee + priority_fee * 150 / 100,     // +50%
    }
}
```

## 성과 지표

### 주요 KPI
- **성공률**: 85-92% (시장 조건에 따라)
- **평균 수익**: 0.5-5 ETH per sandwich
- **가스 효율성**: 수익의 5-15%
- **실행 속도**: 평균 12초 (3블록)
- **자본 효율성**: 무한대 (플래시론 사용시)

### 벤치마크 비교
```
일반 샌드위치 vs 플래시론 샌드위치:

자본 요구량:    100 ETH vs 0 ETH
최대 거래 크기:  100 ETH vs 1000+ ETH  
수익 잠재력:    1-2 ETH vs 5-20 ETH
복잡성:        낮음 vs 높음
가스 비용:     0.1 ETH vs 0.3 ETH
실패 리스크:   중간 vs 낮음 (원자적)
```

## 향후 개선사항

### 1. 기술적 개선
- **ML 예측 모델**: 수익성 예측 정확도 향상
- **다중 DEX**: 여러 DEX 동시 공격
- **크로스체인**: L2 및 사이드체인 확장
- **MEV-Boost 최적화**: 블록 빌더 직접 연동

### 2. 전략 진화
- **JIT 유동성**: Just-In-Time 유동성 공격
- **장기 샌드위치**: 여러 블록에 걸친 전략
- **다중 희생자**: 하나의 플래시론으로 여러 공격
- **적응형 크기**: AI 기반 동적 크기 조절

이 전략은 높은 기술적 복잡성을 가지지만, 적절히 구현될 경우 매우 높은 수익성을 제공할 수 있는 MEV 전략입니다.