# SandwichAttackStrategy.sol - 스마트 컨트랙트 완전 분석

> **컨트랙트**: xCrack Sandwich Attack Strategy
> **언어**: Solidity ^0.8.19
> **총 라인 수**: 543 lines
> **작성자**: xCrack Team
> **라이센스**: MIT

---

## 📚 목차

1. [개요](#1-개요)
2. [컨트랙트 구조](#2-컨트랙트-구조)
3. [핵심 함수 분석](#3-핵심-함수-분석)
4. [보안 분석](#4-보안-분석)
5. [가스 최적화](#5-가스-최적화)
6. [배포 가이드](#6-배포-가이드)
7. [테스트 시나리오](#7-테스트-시나리오)
8. [위험 요소 및 대응](#8-위험-요소-및-대응)

---

## 1. 개요

### 1.1 컨트랙트 목적

`SandwichAttackStrategy.sol`은 DEX의 큰 스왑 트랜잭션을 대상으로 샌드위치 공격을 실행하는 스마트 컨트랙트입니다. Aave v3 FlashLoan을 활용하여 자본 효율을 극대화하고, MEV-Boost를 통해 프라이빗 멤풀로 제출됩니다.

### 1.2 핵심 기능

- ✅ **Aave v3 FlashLoan 통합**: 자본 없이 큰 포지션 실행
- ✅ **원자적 실행**: Front-run → Victim TX → Back-run이 하나의 블록에서 실행
- ✅ **Kelly Criterion 계산**: 온체인 최적 포지션 크기 계산
- ✅ **슬리피지 보호**: 최소 수익 검증 및 가격 임팩트 제한
- ✅ **재진입 방어**: ReentrancyGuard 적용
- ✅ **가스 최적화**: 효율적인 스토리지 사용 및 조기 revert

### 1.3 의존성

```solidity
import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
```

---

## 2. 컨트랙트 구조

### 2.1 상속 구조

```
SandwichAttackStrategy
├── FlashLoanSimpleReceiverBase (Aave v3)
│   └── IFlashLoanSimpleReceiver
└── ReentrancyGuard (OpenZeppelin)
```

### 2.2 주요 데이터 구조

#### SandwichParams (Line 41-53)

```solidity
struct SandwichParams {
    address targetToken;         // 타겟 토큰 (피해자가 매수하려는 토큰)
    address pairedToken;         // 페어 토큰 (WETH, USDC 등)
    address router;              // DEX 라우터 (Uniswap V2 등)
    uint256 frontRunAmount;      // Front-run 매수 금액
    uint256 minVictimAmount;     // 피해자 최소 거래량 (필터링용)
    uint256 minProfitWei;        // 최소 순이익 (wei)
    uint256 maxGasPrice;         // 최대 가스 가격 (경쟁 고려)
    uint256 maxPriceImpact;      // 최대 가격 임팩트 (basis points, 500 = 5%)
    bytes frontRunCalldata;      // Front-run 스왑 calldata
    bytes backRunCalldata;       // Back-run 스왑 calldata
    uint256 deadline;            // 실행 마감 시간
}
```

**설계 의도**:
- `frontRunCalldata`와 `backRunCalldata`는 Rust 백엔드에서 사전 계산된 DEX 호출 데이터
- `maxPriceImpact`는 basis points (10000 = 100%)로 표현하여 정밀도 유지
- `minVictimAmount`는 사용되지 않는 필드 (제거 권장)

#### ExecutionResult (Line 56-63)

```solidity
struct ExecutionResult {
    uint256 frontRunPrice;       // Front-run 가격
    uint256 backRunPrice;        // Back-run 가격
    uint256 priceImpact;         // 가격 임팩트 (basis points)
    uint256 grossProfit;         // 총 수익
    uint256 netProfit;           // 순이익 (가스 차감)
    uint256 gasUsed;             // 사용된 가스
}
```

**데이터 흐름**:
```
SandwichParams (입력)
    ↓
executeSandwich() → FlashLoan 트리거
    ↓
executeOperation() → 콜백
    ↓
_executeSandwichLogic() → Front-run + Back-run
    ↓
ExecutionResult (출력)
```

### 2.3 이벤트 (Line 68-115)

```solidity
event FlashLoanInitiated(address indexed token, uint256 amount, uint256 timestamp);
event FrontRunExecuted(address indexed router, address indexed tokenIn, address indexed tokenOut,
                       uint256 amountIn, uint256 amountOut, uint256 priceImpact, uint256 timestamp);
event BackRunExecuted(address indexed router, address indexed tokenIn, address indexed tokenOut,
                      uint256 amountIn, uint256 amountOut, uint256 priceRealized, uint256 timestamp);
event SandwichSuccess(address indexed targetToken, uint256 flashLoanAmount, uint256 premium,
                      uint256 grossProfit, uint256 netProfit, uint256 priceImpact, uint256 timestamp);
event SandwichFailed(address indexed targetToken, uint256 flashLoanAmount, string reason);
event TokensRescued(address indexed token, uint256 amount, address indexed to);
```

**이벤트 사용 패턴**:
- `indexed` 파라미터로 오프체인 필터링 최적화
- `timestamp` 포함으로 시계열 분석 가능
- `SandwichSuccess`는 수익성 메트릭 포함

### 2.4 에러 정의 (Line 120-131)

```solidity
error Unauthorized();
error InvalidCaller();
error InvalidToken();
error InvalidAmount();
error DeadlineExpired();
error PriceImpactTooHigh(uint256 actual, uint256 max);
error InsufficientProfit(uint256 actual, uint256 required);
error GasPriceTooHigh(uint256 actual, uint256 max);
error VictimAmountTooLow(uint256 actual, uint256 min);
error RouterCallFailed(address router, bytes reason);
error InvalidContract(address account);
```

**가스 효율**:
- Custom errors (Solidity 0.8.4+)는 `require(condition, "string")` 대비 **~50% 가스 절약**
- 파라미터를 포함하여 디버깅 용이

---

## 3. 핵심 함수 분석

### 3.1 executeSandwich() - 진입점 (Line 153-180)

```solidity
function executeSandwich(
    address asset,
    uint256 amount,
    SandwichParams calldata params
) external onlyOwner nonReentrant {
    // 입력 검증
    if (asset != params.pairedToken) revert InvalidToken();
    if (amount != params.frontRunAmount) revert InvalidAmount();
    if (block.timestamp > params.deadline) revert DeadlineExpired();
    if (tx.gasprice > params.maxGasPrice) revert GasPriceTooHigh(tx.gasprice, params.maxGasPrice);

    // 컨트랙트 주소 검증
    _assertContract(params.router);
    _assertContract(params.targetToken);
    _assertContract(params.pairedToken);

    // FlashLoan 실행
    bytes memory data = abi.encode(params);
    emit FlashLoanInitiated(asset, amount, block.timestamp);

    POOL.flashLoanSimple(
        address(this),
        asset,
        amount,
        data,
        0 // referralCode
    );
}
```

**코드 분석**:

**Line 157**: `onlyOwner`로 소유자 전용 실행 보장
```solidity
modifier onlyOwner() {
    if (msg.sender != owner) revert Unauthorized();
    _;
}
```

**Line 157**: `nonReentrant`로 재진입 공격 방지
```solidity
// OpenZeppelin ReentrancyGuard
uint256 private _status;

modifier nonReentrant() {
    require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
    _status = _ENTERED;
    _;
    _status = _NOT_ENTERED;
}
```

**Line 159-162**: 조기 검증으로 가스 절약
- 잘못된 입력 시 FlashLoan 실행 전 revert

**Line 165-167**: 컨트랙트 주소 검증
```solidity
function _assertContract(address account) private view {
    uint256 size;
    assembly {
        size := extcodesize(account)
    }
    if (size == 0) revert InvalidContract(account);
}
```
- `extcodesize`로 EOA vs Contract 구분
- EOA 전송 시 의미없는 호출 방지

**Line 170**: `abi.encode(params)` 로 FlashLoan 콜백 데이터 전달
- `abi.encodePacked()`보다 안전 (타입 안전성)

**Line 173-179**: Aave v3 FlashLoanSimple 호출
```solidity
POOL.flashLoanSimple(
    address receiverAddress,   // address(this)
    address asset,              // params.pairedToken (WETH, USDC 등)
    uint256 amount,             // params.frontRunAmount
    bytes calldata params,      // abi.encode(params)
    uint16 referralCode         // 0 (사용 안함)
);
```

### 3.2 executeOperation() - Aave 콜백 (Line 189-230)

```solidity
function executeOperation(
    address asset,
    uint256 amount,
    uint256 premium,
    address initiator,
    bytes calldata params
) external override returns (bool) {
    // 호출자 검증
    if (msg.sender != address(POOL)) revert InvalidCaller();
    if (initiator != address(this)) revert InvalidCaller();

    SandwichParams memory p = abi.decode(params, (SandwichParams));
    if (asset != p.pairedToken) revert InvalidToken();

    // 마감 시간 체크
    if (block.timestamp > p.deadline) revert DeadlineExpired();

    try this._executeSandwichLogic(p, amount, premium) returns (ExecutionResult memory result) {
        // 상환 준비
        uint256 amountOwed = amount + premium;
        IERC20(asset).safeApprove(address(POOL), 0);
        IERC20(asset).safeApprove(address(POOL), amountOwed);

        emit SandwichSuccess(
            p.targetToken,
            amount,
            premium,
            result.grossProfit,
            result.netProfit,
            result.priceImpact,
            block.timestamp
        );

        return true;
    } catch Error(string memory reason) {
        emit SandwichFailed(p.targetToken, amount, reason);
        revert(reason);
    } catch (bytes memory lowLevelData) {
        emit SandwichFailed(p.targetToken, amount, "Low-level call failed");
        revert RouterCallFailed(address(0), lowLevelData);
    }
}
```

**코드 분석**:

**Line 197-198**: **중요 보안 체크**
```solidity
if (msg.sender != address(POOL)) revert InvalidCaller();
if (initiator != address(this)) revert InvalidCaller();
```
- `msg.sender`가 Aave Pool인지 확인 (재진입 공격 방지)
- `initiator`가 자기 자신인지 확인 (다른 컨트랙트의 FlashLoan 오용 방지)

**Line 206**: **try-catch 패턴**
```solidity
try this._executeSandwichLogic(p, amount, premium) returns (ExecutionResult memory result) {
    // 성공 처리
} catch Error(string memory reason) {
    // revert("reason") 캐치
} catch (bytes memory lowLevelData) {
    // low-level revert 캐치
}
```
- `external` 함수만 try-catch 가능 (`internal`은 불가)
- 실패 시 `SandwichFailed` 이벤트 발생 후 revert

**Line 208-210**: **Aave FlashLoan 상환**
```solidity
uint256 amountOwed = amount + premium;
IERC20(asset).safeApprove(address(POOL), 0);      // 기존 allowance 제거
IERC20(asset).safeApprove(address(POOL), amountOwed);  // 새 allowance 설정
```
- `safeApprove(0)` 먼저 호출 (USDT 등 non-standard ERC20 대응)
- `premium`은 Aave v3 FlashLoan 수수료 (0.09%)

### 3.3 _executeSandwichLogic() - 실행 로직 (Line 235-283)

```solidity
function _executeSandwichLogic(
    SandwichParams memory params,
    uint256 borrowed,
    uint256 premium
) external returns (ExecutionResult memory result) {
    // 재진입 방지
    require(msg.sender == address(this), "Internal only");

    // 1단계: Front-run (pairedToken -> targetToken 매수)
    uint256 targetTokenReceived = _frontRun(params, borrowed);

    // 가격 임팩트 계산 및 검증
    uint256 priceImpact = _calculatePriceImpact(borrowed, targetTokenReceived);
    if (priceImpact > params.maxPriceImpact) {
        revert PriceImpactTooHigh(priceImpact, params.maxPriceImpact);
    }

    // 2단계: Victim TX 대기 (블록 내 자동 실행)
    // 실제로는 victim이 우리 뒤에 같은 블록에 포함되어야 함
    // 이는 MEV-Boost/Flashbots를 통해 bundle로 제출하여 보장

    // 3단계: Back-run (targetToken -> pairedToken 매도)
    uint256 pairedTokenReceived = _backRun(params, targetTokenReceived);

    // 4단계: 수익 검증
    uint256 amountOwed = borrowed + premium;
    if (pairedTokenReceived <= amountOwed) {
        revert InsufficientProfit(pairedTokenReceived - amountOwed, 0);
    }

    uint256 grossProfit = pairedTokenReceived - borrowed;
    uint256 netProfit = pairedTokenReceived - amountOwed;

    if (netProfit < params.minProfitWei) {
        revert InsufficientProfit(netProfit, params.minProfitWei);
    }

    // 결과 반환
    result = ExecutionResult({
        frontRunPrice: (targetTokenReceived * 1e18) / borrowed,
        backRunPrice: (pairedTokenReceived * 1e18) / targetTokenReceived,
        priceImpact: priceImpact,
        grossProfit: grossProfit,
        netProfit: netProfit,
        gasUsed: gasleft() // 근사값
    });

    return result;
}
```

**코드 분석**:

**Line 241**: **internal only 체크**
```solidity
require(msg.sender == address(this), "Internal only");
```
- `external`로 선언했지만 외부 호출 방지
- `executeOperation()`의 try-catch를 위해 `external` 필요

**Line 244**: **Front-run 실행**
```solidity
uint256 targetTokenReceived = _frontRun(params, borrowed);
```
- FlashLoan으로 빌린 `pairedToken`으로 `targetToken` 매수
- 가격 상승 → 희생자가 높은 가격에 매수

**Line 247-250**: **가격 임팩트 검증**
```solidity
uint256 priceImpact = _calculatePriceImpact(borrowed, targetTokenReceived);
if (priceImpact > params.maxPriceImpact) {
    revert PriceImpactTooHigh(priceImpact, params.maxPriceImpact);
}
```
- 가격 임팩트가 너무 크면 revert (슬리피지 보호)
- `maxPriceImpact`: 500 = 5%, 1000 = 10%

**Line 252-254**: **Victim TX 대기 (주석)**
- 실제로는 MEV 번들로 제출되어 원자적 실행 보장
- 컨트랙트 코드에서는 별도 대기 로직 불필요

**Line 257**: **Back-run 실행**
```solidity
uint256 pairedTokenReceived = _backRun(params, targetTokenReceived);
```
- `targetToken`을 `pairedToken`으로 매도
- 높은 가격에 매도하여 차익 실현

**Line 260-263**: **수익 검증**
```solidity
uint256 amountOwed = borrowed + premium;
if (pairedTokenReceived <= amountOwed) {
    revert InsufficientProfit(pairedTokenReceived - amountOwed, 0);
}
```
- FlashLoan 상환액보다 수익이 적으면 revert
- `premium`: Aave v3 FlashLoan 수수료 (0.09%)

**Line 268-270**: **최소 수익 검증**
```solidity
if (netProfit < params.minProfitWei) {
    revert InsufficientProfit(netProfit, params.minProfitWei);
}
```
- 가스 비용 고려한 최소 수익 보장

### 3.4 _frontRun() - Front-run 실행 (Line 292-328)

```solidity
function _frontRun(
    SandwichParams memory params,
    uint256 amount
) private returns (uint256 targetTokenReceived) {
    IERC20 pairedToken = IERC20(params.pairedToken);
    IERC20 targetToken = IERC20(params.targetToken);

    // 잔고 스냅샷
    uint256 targetBefore = targetToken.balanceOf(address(this));

    // Approve
    _safeApprove(pairedToken, params.router, amount);

    // Router 호출 (매수)
    (bool success, bytes memory result) = params.router.call(params.frontRunCalldata);
    if (!success) revert RouterCallFailed(params.router, result);

    // 수령량 계산
    uint256 targetAfter = targetToken.balanceOf(address(this));
    targetTokenReceived = targetAfter - targetBefore;

    // 최소 수령량 검증 (슬리피지 보호)
    require(targetTokenReceived > 0, "Zero output");

    // 가격 임팩트 계산
    uint256 priceImpact = _calculatePriceImpact(amount, targetTokenReceived);

    emit FrontRunExecuted(
        params.router,
        params.pairedToken,
        params.targetToken,
        amount,
        targetTokenReceived,
        priceImpact,
        block.timestamp
    );
}
```

**코드 분석**:

**Line 300**: **잔고 스냅샷 패턴**
```solidity
uint256 targetBefore = targetToken.balanceOf(address(this));
// ... 스왑 실행
uint256 targetAfter = targetToken.balanceOf(address(this));
targetTokenReceived = targetAfter - targetBefore;
```
- DEX 라우터의 반환값에 의존하지 않고 실제 잔고 변화로 계산
- Fee-on-transfer 토큰 대응

**Line 303**: **안전한 Approve**
```solidity
function _safeApprove(IERC20 token, address spender, uint256 amount) private {
    uint256 currentAllowance = token.allowance(address(this), spender);
    if (currentAllowance != 0) {
        token.safeApprove(spender, 0);  // 기존 allowance 제거
    }
    token.safeApprove(spender, amount);  // 새 allowance 설정
}
```
- USDT 등 non-standard ERC20 대응 (approve 0 먼저 호출)

**Line 306**: **Low-level call**
```solidity
(bool success, bytes memory result) = params.router.call(params.frontRunCalldata);
if (!success) revert RouterCallFailed(params.router, result);
```
- `call(bytes calldata)` 사용하여 임의 함수 호출
- `params.frontRunCalldata`는 Rust 백엔드에서 사전 계산
- 예: `swapExactTokensForTokens(amountIn, amountOutMin, path, to, deadline)`

**Line 314**: **슬리피지 보호**
```solidity
require(targetTokenReceived > 0, "Zero output");
```
- 0 수령 시 revert (슬리피지 100%)

### 3.5 _backRun() - Back-run 실행 (Line 333-369)

```solidity
function _backRun(
    SandwichParams memory params,
    uint256 targetAmount
) private returns (uint256 pairedTokenReceived) {
    IERC20 pairedToken = IERC20(params.pairedToken);
    IERC20 targetToken = IERC20(params.targetToken);

    // 잔고 스냅샷
    uint256 pairedBefore = pairedToken.balanceOf(address(this));

    // Approve
    _safeApprove(targetToken, params.router, targetAmount);

    // Router 호출 (매도)
    (bool success, bytes memory result) = params.router.call(params.backRunCalldata);
    if (!success) revert RouterCallFailed(params.router, result);

    // 수령량 계산
    uint256 pairedAfter = pairedToken.balanceOf(address(this));
    pairedTokenReceived = pairedAfter - pairedBefore;

    // 최소 수령량 검증
    require(pairedTokenReceived > 0, "Zero output");

    // 실현 가격 계산
    uint256 priceRealized = (pairedTokenReceived * 1e18) / targetAmount;

    emit BackRunExecuted(
        params.router,
        params.targetToken,
        params.pairedToken,
        targetAmount,
        pairedTokenReceived,
        priceRealized,
        block.timestamp
    );
}
```

**코드 분석**:

**Front-run과 동일한 패턴**:
1. 잔고 스냅샷 (`pairedBefore`)
2. Approve
3. Low-level call (`params.backRunCalldata`)
4. 실제 수령량 계산 (`pairedAfter - pairedBefore`)
5. 슬리피지 보호 (`> 0` 체크)

**Line 358**: **실현 가격 계산**
```solidity
uint256 priceRealized = (pairedTokenReceived * 1e18) / targetAmount;
```
- 18 decimals 정규화
- Front-run 가격과 비교하여 수익 분석 가능

### 3.6 calculateOptimalSize() - Kelly Criterion (Line 459-496)

```solidity
function calculateOptimalSize(
    uint256 successProbability,
    uint256 priceImpactBps,
    uint256 availableCapital
) external pure returns (uint256 optimalSize) {
    // p = successProbability / 10000
    // b = priceImpactBps / 10000 (단순화)
    // Kelly % = (p * b - (1-p)) / b

    uint256 p = successProbability;
    uint256 q = 10000 - successProbability;
    uint256 b = priceImpactBps;

    if (b == 0) return 0;

    // Kelly % in basis points
    uint256 kellyBps;
    if (p * b > q * 10000) {
        kellyBps = ((p * b - q * 10000) * 10000) / (b * 10000);
    } else {
        return 0; // 음수면 공격 안함
    }

    // 안전을 위해 Kelly의 50%만 사용 (Half Kelly)
    kellyBps = kellyBps / 2;

    // 최적 크기 계산
    optimalSize = (availableCapital * kellyBps) / 10000;

    // 최소/최대 제한
    uint256 minSize = availableCapital / 100; // 최소 1%
    uint256 maxSize = availableCapital / 4;   // 최대 25%

    if (optimalSize < minSize) optimalSize = minSize;
    if (optimalSize > maxSize) optimalSize = maxSize;

    return optimalSize;
}
```

**코드 분석**:

**Kelly Criterion 공식** (Line 464-465):
```
Kelly % = (p * b - q) / b

여기서:
- p: 성공 확률 (0.8 = 80%)
- q: 실패 확률 (1 - p)
- b: 예상 수익률 (0.05 = 5%)
```

**Basis Points 사용** (Line 475-478):
```solidity
uint256 kellyBps;
if (p * b > q * 10000) {
    kellyBps = ((p * b - q * 10000) * 10000) / (b * 10000);
} else {
    return 0; // 음수면 공격 안함
}
```
- `10000 = 100%` (basis points)
- 정수 연산으로 소수점 처리 (Solidity는 float 미지원)

**Half Kelly** (Line 481):
```solidity
kellyBps = kellyBps / 2;
```
- Full Kelly는 변동성이 높아 위험
- Half Kelly로 안전성 확보

**포지션 크기 제한** (Line 487-493):
```solidity
uint256 minSize = availableCapital / 100; // 최소 1%
uint256 maxSize = availableCapital / 4;   // 최대 25%

if (optimalSize < minSize) optimalSize = minSize;
if (optimalSize > maxSize) optimalSize = maxSize;
```
- 최소 1%: 가스 비용 대비 너무 작은 공격 방지
- 최대 25%: 단일 공격 리스크 분산

---

## 4. 보안 분석

### 4.1 보안 강점

#### ✅ ReentrancyGuard (Line 35)

```solidity
contract SandwichAttackStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard {
    // ...
    function executeSandwich(...) external onlyOwner nonReentrant {
        // ...
    }
}
```

**재진입 공격 시나리오**:
1. 악의적인 ERC20 토큰이 `transfer()` 중 재진입
2. `executeSandwich()` 재호출
3. 중복 FlashLoan 실행

**방어**:
- `nonReentrant` modifier로 재진입 차단
- OpenZeppelin의 검증된 구현 사용

#### ✅ onlyOwner 제한 (Line 133-136)

```solidity
modifier onlyOwner() {
    if (msg.sender != owner) revert Unauthorized();
    _;
}
```

**공격 시나리오**:
- 공격자가 `executeSandwich()` 직접 호출
- FlashLoan 수수료를 공격자가 아닌 컨트랙트가 부담

**방어**:
- `onlyOwner`로 소유자만 실행 가능
- 프라이빗 키 관리 중요 (Gnosis Safe 권장)

#### ✅ FlashLoan 콜백 검증 (Line 197-198)

```solidity
if (msg.sender != address(POOL)) revert InvalidCaller();
if (initiator != address(this)) revert InvalidCaller();
```

**공격 시나리오**:
1. 공격자가 `executeOperation()` 직접 호출
2. 또는 다른 컨트랙트가 FlashLoan으로 호출

**방어**:
- `msg.sender == POOL`: Aave Pool만 호출 가능
- `initiator == this`: 자기 자신이 시작한 FlashLoan만 처리

#### ✅ 컨트랙트 주소 검증 (Line 402-408)

```solidity
function _assertContract(address account) private view {
    uint256 size;
    assembly {
        size := extcodesize(account)
    }
    if (size == 0) revert InvalidContract(account);
}
```

**공격 시나리오**:
- `params.router`를 EOA 주소로 설정
- `call()` 실패하지 않고 false 반환

**방어**:
- `extcodesize`로 컨트랙트 존재 확인
- EOA 주소 사용 방지

#### ✅ Custom Errors (Line 120-131)

```solidity
error PriceImpactTooHigh(uint256 actual, uint256 max);
error InsufficientProfit(uint256 actual, uint256 required);
```

**장점**:
- `require(condition, "string")` 대비 **~50% 가스 절약**
- 파라미터 포함으로 디버깅 용이
- ABI 인코딩 효율

### 4.2 잠재적 취약점

#### ⚠️ Approval Race Condition

**문제**:
```solidity
function _safeApprove(IERC20 token, address spender, uint256 amount) private {
    uint256 currentAllowance = token.allowance(address(this), spender);
    if (currentAllowance != 0) {
        token.safeApprove(spender, 0);
    }
    token.safeApprove(spender, amount);
}
```

**시나리오**:
- 트랜잭션 1: `approve(router, 100)`
- 트랜잭션 2: `approve(router, 50)`
- Router가 150 사용 가능 (race condition)

**완화**:
- 단일 트랜잭션 내 실행으로 race condition 없음
- `safeApprove(0)` 먼저 호출

#### ⚠️ FlashLoan Premium 변동

**문제**:
```solidity
uint256 amountOwed = amount + premium;
```

**시나리오**:
- Aave v3 governance가 premium을 0.09% → 1%로 변경
- 수익성 계산 오차 발생

**완화**:
- Off-chain에서 `premium` 사전 계산하여 `minProfitWei`에 반영
- 실행 전 `POOL.FLASHLOAN_PREMIUM_TOTAL()` 조회

#### ⚠️ Front-running (Paradox)

**문제**:
- 샌드위치 공격 컨트랙트 자체가 front-run 당할 수 있음
- 공격자가 우리의 `executeSandwich()` TX를 관찰하고 먼저 실행

**완화**:
- **MEV-Boost/Flashbots로 Private mempool 제출**
- `maxGasPrice` 설정으로 경쟁 제한
- `deadline` 짧게 설정하여 시간 제한

#### ⚠️ Victim TX 실패

**문제**:
```solidity
// 2단계: Victim TX 대기 (블록 내 자동 실행)
```

**시나리오**:
- Victim TX가 revert (슬리피지 초과, 가스 부족 등)
- Front-run만 실행되고 Back-run 실패 → 손실

**완화**:
- MEV 번들로 제출하여 원자적 실행 보장
- Victim TX 시뮬레이션 (Off-chain)

### 4.3 권장 보안 개선

1. **Gnosis Safe 사용**: `owner`를 Gnosis Safe 멀티시그로 설정
2. **Flashbots Protect**: Private mempool로 front-running 방지
3. **Circuit Breaker**: 연속 실패 시 자동 중지
4. **Rate Limiting**: 블록당 최대 실행 횟수 제한
5. **Emergency Pause**: Pausable 패턴 추가

---

## 5. 가스 최적화

### 5.1 최적화 기법

#### 1. Custom Errors (Line 120-131)

```solidity
// Before (Solidity <0.8.4)
require(msg.sender == owner, "Unauthorized");  // ~50 gas per character

// After (Solidity >=0.8.4)
if (msg.sender != owner) revert Unauthorized();  // ~24 gas (base)
```

**절감**: ~50% 가스 절약

#### 2. Early Revert (Line 159-162)

```solidity
// 조기 검증
if (asset != params.pairedToken) revert InvalidToken();
if (amount != params.frontRunAmount) revert InvalidAmount();
if (block.timestamp > params.deadline) revert DeadlineExpired();
// ... FlashLoan 실행
```

**절감**: 잘못된 입력 시 21,000 gas (base TX) + α 만 소비

#### 3. Storage 최소화

```solidity
contract SandwichAttackStrategy {
    address private owner;  // ← 유일한 storage 변수
}
```

**절감**:
- `SandwichParams`는 `calldata` (storage 사용 안함)
- `ExecutionResult`는 `memory` (storage 사용 안함)

#### 4. Batch Operations

```solidity
// 단일 FlashLoan으로 Front-run + Back-run 실행
POOL.flashLoanSimple(address(this), asset, amount, data, 0);
```

**절감**: FlashLoan 2회 → 1회로 감소

### 5.2 가스 비용 추정

```
executeSandwich() 전체 가스:
├─ Base TX: 21,000 gas
├─ FlashLoanSimple: ~30,000 gas
├─ executeOperation():
│  ├─ Front-run Swap: ~120,000 gas
│  ├─ Back-run Swap: ~120,000 gas
│  ├─ Approve (2회): ~50,000 gas
│  └─ Misc: ~10,000 gas
└─ Total: ~351,000 gas

At 50 Gwei:
- Total Cost: 0.01755 ETH (~$35 at $2000/ETH)
```

**최적화 후**:
- Custom errors: -5,000 gas
- Early revert 최적화: -2,000 gas
- **Total**: ~344,000 gas (-2%)

---

## 6. 배포 가이드

### 6.1 Foundry 배포

```bash
# 1. 환경 변수 설정
export PRIVATE_KEY="0x..."
export RPC_URL="https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY"
export ETHERSCAN_API_KEY="YOUR_KEY"
export AAVE_POOL_PROVIDER="0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e"  # Mainnet

# 2. 컴파일
forge build

# 3. 배포
forge create --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --etherscan-api-key $ETHERSCAN_API_KEY \
    --verify \
    contracts/SandwichAttackStrategy.sol:SandwichAttackStrategy \
    --constructor-args $AAVE_POOL_PROVIDER

# 4. 배포 주소 확인
# 출력: Deployed to: 0x...
```

### 6.2 초기 설정

```solidity
// Etherscan에서 Verify 후 Write Contract

// 1. Owner 확인
getOwner()  // 배포자 주소 확인

// 2. (선택) Owner 변경 (Gnosis Safe)
setOwner(0x...GnosisSafeAddress...)
```

### 6.3 테스트넷 배포

```bash
# Sepolia 배포
export RPC_URL="https://eth-sepolia.g.alchemy.com/v2/YOUR_KEY"
export AAVE_POOL_PROVIDER="0x0496275d34753A48320CA58103d5220d394FF77F"  # Sepolia

forge create --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --verify \
    contracts/SandwichAttackStrategy.sol:SandwichAttackStrategy \
    --constructor-args $AAVE_POOL_PROVIDER
```

---

## 7. 테스트 시나리오

### 7.1 Foundry 테스트

```solidity
// test/SandwichAttackStrategy.t.sol

pragma solidity ^0.8.19;

import "forge-std/Test.sol";
import "../contracts/SandwichAttackStrategy.sol";

contract SandwichAttackStrategyTest is Test {
    SandwichAttackStrategy strategy;
    address constant AAVE_POOL_PROVIDER = 0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e;
    address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;

    function setUp() public {
        strategy = new SandwichAttackStrategy(IPoolAddressesProvider(AAVE_POOL_PROVIDER));
    }

    function testExecuteSandwich() public {
        // 1. SandwichParams 준비
        SandwichAttackStrategy.SandwichParams memory params = SandwichAttackStrategy.SandwichParams({
            targetToken: USDC,
            pairedToken: WETH,
            router: UNISWAP_V2_ROUTER,
            frontRunAmount: 1 ether,
            minVictimAmount: 10 ether,
            minProfitWei: 0.01 ether,
            maxGasPrice: 200 gwei,
            maxPriceImpact: 500, // 5%
            frontRunCalldata: abi.encodeWithSignature(
                "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)",
                1 ether,
                0,
                _createPath(WETH, USDC),
                address(strategy),
                block.timestamp + 300
            ),
            backRunCalldata: abi.encodeWithSignature(
                "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)",
                0, // will be filled at runtime
                0,
                _createPath(USDC, WETH),
                address(strategy),
                block.timestamp + 300
            ),
            deadline: block.timestamp + 300
        });

        // 2. executeSandwich 호출
        strategy.executeSandwich(WETH, 1 ether, params);

        // 3. 성공 확인 (이벤트 체크)
        // vm.expectEmit()을 사용하여 SandwichSuccess 이벤트 확인
    }

    function _createPath(address from, address to) internal pure returns (address[] memory) {
        address[] memory path = new address[](2);
        path[0] = from;
        path[1] = to;
        return path;
    }
}
```

### 7.2 수동 테스트 (Mainnet Fork)

```bash
# 1. Mainnet fork
anvil --fork-url https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY

# 2. 배포
forge create --rpc-url http://localhost:8545 \
    --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
    contracts/SandwichAttackStrategy.sol:SandwichAttackStrategy \
    --constructor-args 0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e

# 3. 테스트 실행
forge test --fork-url http://localhost:8545 -vvv
```

### 7.3 시뮬레이션 (Tenderly)

1. **Tenderly 프로젝트 생성**: https://dashboard.tenderly.co/
2. **Simulation API 호출**:
```typescript
const response = await axios.post(
    `https://api.tenderly.co/api/v1/account/${ACCOUNT}/project/${PROJECT}/simulate`,
    {
        network_id: "1", // Mainnet
        from: "0x...", // 소유자 주소
        to: "0x...", // 컨트랙트 주소
        input: encodedCalldata, // executeSandwich calldata
        gas: 500000,
        gas_price: "50000000000", // 50 Gwei
        value: "0",
        save: true
    },
    {
        headers: {
            "X-Access-Key": TENDERLY_ACCESS_KEY
        }
    }
);

console.log(response.data);
```

---

## 8. 위험 요소 및 대응

### 8.1 Smart Contract 위험

| 위험 | 설명 | 대응 |
|------|------|------|
| **재진입 공격** | 악의적 ERC20이 재진입 | ✅ ReentrancyGuard |
| **FlashLoan 오용** | 다른 컨트랙트가 콜백 호출 | ✅ Caller 검증 |
| **Approval 경쟁** | Approve race condition | ✅ safeApprove(0) 먼저 |
| **가스 부족** | 복잡한 스왑 시 가스 초과 | ⚠️ 충분한 gas limit 설정 |
| **Victim TX 실패** | 희생자 TX가 revert | ⚠️ MEV 번들로 원자성 보장 |

### 8.2 경제적 위험

| 위험 | 설명 | 대응 |
|------|------|------|
| **가격 임팩트** | 큰 스왑 시 슬리피지 | ✅ maxPriceImpact 검증 |
| **FlashLoan 수수료** | Premium 변동 (0.09%) | ⚠️ 실행 전 조회 |
| **가스 비용** | 높은 가스 시 손실 | ✅ maxGasPrice 제한 |
| **최소 수익** | 수익률 부족 | ✅ minProfitWei 검증 |
| **Pool 유동성** | 유동성 부족 시 revert | ⚠️ Off-chain 검증 |

### 8.3 운영 위험

| 위험 | 설명 | 대응 |
|------|------|------|
| **Private Key 유출** | 소유자 키 도난 | ✅ Gnosis Safe 사용 |
| **Front-running** | 샌드위치가 샌드위치 당함 | ✅ Flashbots Private TX |
| **Nonce 관리** | 연속 TX 시 nonce 충돌 | ⚠️ Off-chain nonce 추적 |
| **Deadline 만료** | 느린 블록 시 만료 | ⚠️ 충분한 deadline 설정 |
| **컨트랙트 업그레이드** | Aave/DEX 업그레이드 | ⚠️ 정기 모니터링 |

### 8.4 긴급 대응 절차

1. **Circuit Breaker**: 연속 3회 실패 시 자동 중지
2. **Emergency Withdraw**: `rescueTokens()`로 자금 회수
3. **Owner 변경**: `setOwner()`로 새 주소 설정
4. **Pause**: (미구현) Pausable 패턴 추가 권장

---

**마지막 업데이트**: 2025-01-XX
**버전**: 1.0.0
**작성자**: xCrack Development Team

---

## 참고 자료

- [Aave v3 Docs](https://docs.aave.com/developers/core-contracts/pool)
- [Uniswap V2 Docs](https://docs.uniswap.org/contracts/v2/overview)
- [OpenZeppelin Contracts](https://docs.openzeppelin.com/contracts/)
- [Flashbots Docs](https://docs.flashbots.net/)
- [Solidity Security Best Practices](https://consensys.github.io/smart-contract-best-practices/)
