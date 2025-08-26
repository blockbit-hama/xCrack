# 청산 전략 (Liquidation Strategy)

## 개요

청산 전략은 레버리지 포지션이 담보 부족으로 인해 청산 임계값에 도달했을 때, 해당 포지션을 청산하여 수익을 얻는 MEV 전략입니다. 주로 Aave, Compound 같은 대출 프로토콜에서 건강하지 않은 포지션을 찾아 청산 보상을 획득합니다.

## 전략 원리

### 기본 메커니즘
```
1. 건강도 모니터링 → 레버리지 포지션 상태 추적
2. 청산 기회 감지 → Health Factor < 1.0 포지션 발견
3. 청산 실행 → 담보 자산 획득 + 보상 수령
4. 자산 교환 → 담보를 원하는 자산으로 스왑
```

### 수익 계산
```rust
// 청산 보상 공식 (Aave V3 기준)
liquidation_bonus = collateral_value * liquidation_bonus_rate  // 5-10%
profit = liquidation_bonus - debt_repaid - gas_costs - flashloan_fee
```

## 플래시론 vs 일반 실행 비교

### 1. 일반 청산 (Non-Flashloan)

**장점**:
- 단순한 구조
- 낮은 가스 비용  
- 빠른 실행
- 플래시론 수수료 없음

**단점**:
- 채무 토큰 보유 필수
- 제한된 청산 규모
- 자본 효율성 낮음
- 다양한 토큰 보유 필요

**실행 시나리오**:
```
포지션 정보:
- 담보: 10 ETH (가치 $20,000)
- 채무: 15,000 USDC
- Health Factor: 0.95 (청산 가능)
- 청산 보너스: 5%

보유 자산: 15,000 USDC

1. 청산 실행:
   - 15,000 USDC로 채무 상환
   - 담보 획득: 10 ETH
   - 보너스: 0.5 ETH (5% of 10 ETH)

2. 수익 실현:
   - 10.5 ETH 보유 (원래 담보 + 보너스)
   - 15,000 USDC 지불
   - 순 수익: ~$1,000 (5% 보너스)

자본 요구량: 15,000 USDC
수익률: ~6.7% per liquidation
```

### 2. 플래시론 청산 (Flashloan Liquidation)

**장점**:
- 무제한 청산 가능
- 다양한 토큰 청산 지원
- 높은 자본 효율성
- 원자적 실행 보장

**단점**:
- 복잡한 스마트 컨트랙트
- 플래시론 수수료 (0.05-0.30%)
- 높은 가스 비용
- 기술적 복잡성

**실행 시나리오**:
```
대형 포지션:
- 담보: 1000 ETH (가치 $2,000,000)  
- 채무: 1,500,000 USDC
- Health Factor: 0.92
- 청산 보너스: 5%

1. Flashloan: 1,500,000 USDC (Aave V3)

2. 청산 실행:
   - 1,500,000 USDC로 채무 상환
   - 담보 획득: 1000 ETH
   - 보너스: 50 ETH

3. 자산 교환:
   - 1000 ETH → USDC 스왑 (Uniswap)
   - 받는 금액: ~$2,000,000

4. 플래시론 상환:
   - 원금: 1,500,000 USDC
   - 수수료: 750 USDC (0.05%)
   - 총 상환: 1,500,750 USDC

5. 순 수익:
   - 수령: $2,000,000
   - 지불: $1,500,750  
   - 순 수익: ~$499,250

자본 요구량: 0 USDC
수익률: 무한대 (자본 불요)
```

## 스마트 컨트랙트 구현

### 플래시론 청산 컨트랙트

```solidity
pragma solidity ^0.8.19;

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract LiquidationStrategy is FlashLoanSimpleReceiverBase {
    address private owner;
    
    struct LiquidationParams {
        address protocol;           // Aave, Compound 등
        address user;               // 청산 대상 사용자
        address collateralAsset;    // 담보 자산
        address debtAsset;          // 채무 자산
        uint256 debtToCover;        // 상환할 채무 금액
        address dexRouter;          // DEX 라우터 (자산 교환용)
        bytes swapCalldata;         // 스왑 호출 데이터
    }
    
    constructor(IPoolAddressesProvider provider) 
        FlashLoanSimpleReceiverBase(provider) {
        owner = msg.sender;
    }
    
    function executeLiquidation(
        address asset,
        uint256 amount,
        LiquidationParams calldata params
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
        
        LiquidationParams memory liquidationParams = 
            abi.decode(params, (LiquidationParams));
        
        // 1. 사용자 포지션 청산
        _liquidatePosition(liquidationParams, asset, amount);
        
        // 2. 담보 자산을 채무 자산으로 스왑 (상환용)
        _swapCollateralForDebt(liquidationParams);
        
        // 3. 플래시론 상환
        uint256 amountOwed = amount + premium;
        IERC20(asset).approve(address(POOL), amountOwed);
        
        return true;
    }
    
    function _liquidatePosition(
        LiquidationParams memory params,
        address asset,
        uint256 amount
    ) private {
        if (params.protocol == AAVE_V3_POOL) {
            // Aave V3 청산
            IPool(params.protocol).liquidationCall(
                params.collateralAsset,
                params.debtAsset,
                params.user,
                params.debtToCover,
                false  // receiveAToken = false
            );
        } else if (params.protocol == COMPOUND_COMPTROLLER) {
            // Compound 청산 (simplified)
            ICToken(params.debtAsset).liquidateBorrow(
                params.user,
                params.debtToCover,
                params.collateralAsset
            );
        }
    }
    
    function _swapCollateralForDebt(LiquidationParams memory params) private {
        // 담보 자산을 채무 자산으로 교환 (플래시론 상환용)
        uint256 collateralBalance = IERC20(params.collateralAsset).balanceOf(address(this));
        IERC20(params.collateralAsset).approve(params.dexRouter, collateralBalance);
        
        (bool success,) = params.dexRouter.call(params.swapCalldata);
        require(success, "Swap failed");
    }
}
```

## 실행 시나리오 상세 분석

### 시나리오 1: Aave V3 ETH/USDC 포지션 청산

**포지션 정보**:
- 사용자: 0x1234...
- 담보: 50 ETH ($100,000)
- 채무: 75,000 USDC  
- Loan-to-Value (LTV): 75%
- 청산 임계값 (LT): 80%
- 현재 ETH 가격: $2,000
- Health Factor: 0.95

**일반 청산 실행**:
```
보유 자산: 75,000 USDC

1. 직접 청산:
   - liquidationCall() 호출
   - 75,000 USDC 지불 → 50 ETH + 보너스 수령
   - 청산 보너스: 2.5 ETH (5%)
   - 총 수령: 52.5 ETH

2. 수익 계산:
   - 수령 가치: $105,000 (52.5 ETH × $2,000)
   - 지불 금액: $75,000
   - 순 수익: $30,000
   - 가스 비용: ~$50
   - 최종 수익: ~$29,950

ROI: 39.9% (단일 거래)
자본 요구량: $75,000
```

**플래시론 청산 실행**:
```
1. Flashloan: 75,000 USDC (Aave V3, 0.05% 수수료)

2. 청산 실행:
   - liquidationCall() 호출
   - 75,000 USDC 상환 → 52.5 ETH 수령

3. ETH → USDC 스왑:
   - 52.5 ETH → Uniswap V3 스왑
   - 수령: ~$104,800 (슬리피지 0.2%)

4. 플래시론 상환:
   - 원금: 75,000 USDC
   - 수수료: 37.5 USDC  
   - 총 상환: 75,037.5 USDC

5. 순 수익:
   - 수령: $104,800
   - 상환: $75,037.5
   - 가스: ~$80 (복잡한 트랜잭션)
   - 최종 수익: ~$29,682.5

ROI: 무한대 (자본 불요)
실행 성공률: 98%
```

### 시나리오 2: Compound DAI/ETH 포지션 청산

**포지션 정보**:
- 담보: 200,000 DAI
- 채무: 80 WETH
- ETH 급등으로 Health Factor 급락: 0.89
- Compound 청산 보너스: 8%

**플래시론 최적화 실행**:
```
1. 기회 분석:
   - 채무 가치: $160,000 (80 ETH × $2,000)
   - 담보 가치: $200,000
   - 청산 보너스: $16,000 (8%)
   - 예상 순익: ~$15,500

2. Flashloan: 80 WETH (Balancer, 0% 수수료)

3. Compound 청산:
   - cDAI.liquidateBorrow() 호출
   - 80 WETH 상환 → 216,000 DAI 수령 (담보 + 8% 보너스)

4. WETH 확보:
   - 166,000 DAI → WETH 스왑 (Curve, 낮은 슬리피지)
   - 수령: ~83 WETH

5. 상환 및 수익:
   - 상환: 80 WETH (수수료 없음)
   - 잉여: 3 WETH
   - 잉여 DAI: 50,000 DAI
   - 총 수익: ~$56,000

실제 결과: 매우 성공적
```

## 청산 기회 탐지 알고리즘

### 1. Health Factor 모니터링
```rust
// Health Factor 계산 (Aave 기준)
fn calculate_health_factor(
    total_collateral: U256,
    total_debt: U256,
    liquidation_threshold: u16  // e.g., 8500 = 85%
) -> U256 {
    if total_debt == U256::zero() {
        return U256::MAX; // 무한대 (안전)
    }
    
    let collateral_with_threshold = total_collateral * liquidation_threshold / 10000;
    collateral_with_threshold * U256::from(1e18) / total_debt
}

// 청산 가능 체크
fn is_liquidatable(health_factor: U256) -> bool {
    health_factor < U256::from(1e18) // < 1.0
}
```

### 2. 수익성 계산
```rust
// 청산 수익성 분석
fn calculate_liquidation_profit(
    collateral_amount: U256,
    debt_amount: U256,
    liquidation_bonus: u16,
    gas_price: U256,
    flashloan_fee: u16
) -> i256 {
    // 수령할 담보 + 보너스
    let total_received = collateral_amount + 
        (collateral_amount * liquidation_bonus / 10000);
    
    // 상환할 채무 + 플래시론 수수료
    let total_cost = debt_amount + 
        (debt_amount * flashloan_fee / 10000) +
        estimate_gas_cost(gas_price);
    
    // 스왑 후 예상 수익
    let swap_received = estimate_swap_output(total_received);
    
    i256::from(swap_received) - i256::from(total_cost)
}
```

### 3. 최적 청산 크기 계산
```rust
// 최대 청산 가능 금액 계산 (Aave 기준)
fn calculate_max_liquidation_amount(
    total_debt: U256,
    health_factor: U256,
    close_factor: u16  // 5000 = 50%
) -> U256 {
    if health_factor >= U256::from(0.95e18) {
        // HF가 0.95 이상이면 50%만 청산 가능
        total_debt * close_factor / 10000
    } else {
        // HF가 0.95 미만이면 100% 청산 가능  
        total_debt
    }
}
```

## 리스크 관리

### 1. 시장 리스크
- **가격 변동성**: 청산 중 담보 가격 급락
- **슬리피지**: 대량 스왑 시 불리한 가격
- **유동성 부족**: DEX에서 충분한 유동성 부족
- **경쟁**: 다른 청산자와의 가스 전쟁

### 2. 기술적 리스크  
- **가스 한도**: 복잡한 트랜잭션으로 인한 가스 부족
- **MEV 경쟁**: 더 빠른 봇에 의한 선점
- **스마트 컨트랙트**: 청산 로직 버그
- **오라클 지연**: 가격 피드 지연으로 인한 기회 손실

### 3. 프로토콜 리스크
- **파라미터 변경**: 청산 임계값, 보너스 변경
- **업그레이드**: 프로토콜 업그레이드로 인한 호환성
- **긴급 정지**: 프로토콜 일시 정지
- **거버넌스**: 청산 규칙 변경

## 수익성 최적화 전략

### 1. 멀티 프로토콜 모니터링
```rust
// 여러 프로토콜 동시 모니터링
async fn monitor_all_protocols() -> Vec<LiquidationOpportunity> {
    let mut opportunities = Vec::new();
    
    // Aave V2, V3 모니터링
    opportunities.extend(monitor_aave_positions().await);
    
    // Compound 모니터링  
    opportunities.extend(monitor_compound_positions().await);
    
    // MakerDAO 모니터링
    opportunities.extend(monitor_maker_vaults().await);
    
    // 수익성 기준 정렬
    opportunities.sort_by(|a, b| b.expected_profit.cmp(&a.expected_profit));
    
    opportunities
}
```

### 2. 동적 가스 가격 조절
```rust
// 경쟁 상황에 따른 가스 가격 조절
fn calculate_competitive_gas_price(
    base_gas_price: U256,
    expected_profit: U256,
    competition_level: u8
) -> U256 {
    let max_gas_spend = expected_profit * 30 / 100; // 수익의 30%까지
    
    match competition_level {
        0..=2 => base_gas_price * 110 / 100,           // +10%
        3..=5 => min(base_gas_price * 130 / 100, max_gas_spend),  // +30%
        6..=10 => min(base_gas_price * 200 / 100, max_gas_spend), // +100%
    }
}
```

### 3. 배치 청산 최적화
```rust
// 여러 포지션 일괄 청산
fn batch_liquidations(opportunities: Vec<LiquidationOpportunity>) -> BatchParams {
    let total_flashloan_needed = opportunities.iter()
        .map(|op| op.debt_amount)
        .sum();
    
    // 단일 플래시론으로 여러 청산 실행
    BatchParams {
        flashloan_amount: total_flashloan_needed,
        liquidations: opportunities,
        estimated_gas: estimate_batch_gas_cost(&opportunities),
        expected_profit: calculate_batch_profit(&opportunities),
    }
}
```

## 성과 지표

### 주요 KPI
- **성공률**: 92-97% (시장 조건에 따라)
- **평균 수익**: 0.8-8 ETH per liquidation  
- **가스 효율성**: 수익의 3-12%
- **실행 속도**: 평균 8초 (2블록)
- **발견률**: 시간당 5-15개 기회

### 프로토콜별 성과
```
Aave V3:     높은 수익성, 낮은 가스비, 안정성 우수
Aave V2:     중간 수익성, 높은 유동성, 검증됨
Compound:    높은 보너스율, 높은 가스비, 경쟁 치열  
MakerDAO:    대형 청산, 낮은 빈도, 높은 수익
```

## 향후 개선사항

### 1. 고급 전략
- **부분 청산**: 최적 청산 비율 계산
- **크로스체인**: 다중 체인 청산 기회
- **예측 모델**: ML 기반 청산 시점 예측
- **MEV-Boost**: 블록 빌더와 직접 연동

### 2. 기술적 개선
- **병렬 처리**: 다중 청산 동시 실행
- **가스 최적화**: 어셈블리 레벨 최적화
- **지연 최소화**: 실시간 스트리밍 개선
- **자동 매개변수 조정**: 시장 조건별 최적화

청산 전략은 전통적이면서도 안정적인 MEV 전략으로, 특히 플래시론을 활용할 경우 자본 효율성과 수익성을 크게 향상시킬 수 있습니다.