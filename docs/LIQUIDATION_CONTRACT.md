# LiquidationStrategy Smart Contract 완전 분석 및 초보자 튜토리얼

## 📋 목차

1. [개요](#1-개요)
2. [Solidity 기본 문법 설명](#2-solidity-기본-문법-설명)
3. [컨트랙트 구조 전체 분석](#3-컨트랙트-구조-전체-분석)
4. [라인별 상세 코드 분석](#4-라인별-상세-코드-분석)
5. [플래시론(Flash Loan) 완전 가이드](#5-플래시론flash-loan-완전-가이드)
6. [프로토콜별 청산 메커니즘](#6-프로토콜별-청산-메커니즘)
7. [DEX 스왑 로직 상세 분석](#7-dex-스왑-로직-상세-분석)
8. [보안 및 에러 처리](#8-보안-및-에러-처리)
9. [실전 사용 예제](#9-실전-사용-예제)
10. [FAQ 및 트러블슈팅](#10-faq-및-트러블슈팅)

---

## 1. 개요

### 1.1 LiquidationStrategy란?

**LiquidationStrategy**는 DeFi(탈중앙화 금융) 대출 프로토콜에서 담보 부족 상태가 된 사용자의 포지션을 청산하여 수익을 얻는 스마트 컨트랙트입니다.

**핵심 개념:**
- **청산(Liquidation)**: 담보가 부족한 대출자의 담보를 강제로 매각하여 빚을 갚는 과정
- **플래시론(Flash Loan)**: 담보 없이 대량의 자금을 빌렸다가 같은 트랜잭션 내에서 갚는 금융 기법
- **멀티 프로토콜**: Aave v3, Compound v2, Compound v3를 모두 지원

**작동 원리 (간단 버전):**
```
1. 청산 대상자 발견 (Health Factor < 1.0)
2. 플래시론으로 대량 자금 빌림 (예: 100 ETH)
3. 빌린 돈으로 대상자의 빚 상환
4. 담보 자산 획득 (보너스 포함)
5. 담보를 DEX에서 팔아 원래 빌린 자산으로 교환
6. 플래시론 상환 (원금 + 수수료)
7. 남은 금액 = 수익!
```

### 1.2 지원 프로토콜

| 프로토콜 | 버전 | 특징 | 청산 메커니즘 |
|---------|------|------|--------------|
| **Aave** | v3 | 가장 큰 대출 프로토콜 | `liquidationCall()` |
| **Compound** | v2 | 레거시 대출 프로토콜 | `liquidateBorrow()` + `redeem()` |
| **Compound** | v3 (Comet) | 최신 Compound | `absorb()` |

### 1.3 주요 기능

✅ **자동 청산**: 담보 부족 포지션 자동 감지 및 청산
✅ **플래시론 통합**: Aave v3 플래시론으로 무담보 청산
✅ **DEX 스왑**: 담보 자산을 자동으로 빚 상환 자산으로 교환
✅ **멀티 프로토콜**: 3개 주요 대출 프로토콜 지원
✅ **보안 강화**: ReentrancyGuard, Ownable, Custom Errors
✅ **가스 최적화**: 효율적인 코드 구조

---

## 2. Solidity 기본 문법 설명

이 섹션에서는 컨트랙트에 사용된 Solidity 문법을 초보자도 이해할 수 있도록 설명합니다.

### 2.1 Pragma와 라이선스

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;
```

**설명:**
- `SPDX-License-Identifier`: 코드의 라이선스 명시 (MIT = 자유롭게 사용 가능)
- `pragma solidity ^0.8.19`: Solidity 컴파일러 버전 지정
  - `^0.8.19` = 0.8.19 이상 0.9.0 미만 버전 사용 가능
  - `^` 기호는 "캐럿(caret)"이라고 읽으며, 마이너 버전 호환성을 의미

### 2.2 Import 문

```solidity
import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
```

**설명:**
- 다른 컨트랙트나 라이브러리를 가져와서 사용
- `@aave`, `@openzeppelin`: npm 패키지에서 가져옴
- **Interface**: 함수 선언만 있고 구현은 없는 "계약서" 같은 것
  - 다른 컨트랙트와 소통하기 위한 규칙을 정의

### 2.3 인터페이스 (Interface)

```solidity
interface IAavePool {
    function liquidationCall(
        address collateralAsset,
        address debtAsset,
        address user,
        uint256 debtToCover,
        bool receiveAToken
    ) external;
}
```

**설명:**
- **Interface**: 함수의 "서명"만 정의 (구현 코드 없음)
- `external`: 외부에서만 호출 가능한 함수
- 실제 Aave Pool 컨트랙트의 `liquidationCall` 함수와 통신하기 위한 규칙

**왜 Interface를 사용하나요?**
- 다른 컨트랙트의 주소만 알면 함수 호출 가능
- 전체 코드를 복사할 필요 없음
- 타입 안정성 보장

### 2.4 Struct (구조체)

```solidity
struct LiquidationParams {
    ProtocolType protocolType;
    address protocol;
    address user;
    // ... 기타 필드
}
```

**설명:**
- **Struct**: 여러 데이터를 하나로 묶는 "데이터 꾸러미"
- C언어의 struct, JavaScript의 Object와 유사
- 관련된 데이터를 그룹화하여 관리

**예시:**
```solidity
LiquidationParams memory params = LiquidationParams({
    protocolType: ProtocolType.AAVE,
    protocol: 0x1234...,
    user: 0x5678...,
    // ...
});
```

### 2.5 Enum (열거형)

```solidity
enum ProtocolType { AAVE, COMPOUND_V2, COMPOUND_V3 }
```

**설명:**
- **Enum**: 미리 정의된 값들 중 하나를 선택하는 타입
- 0부터 시작하는 숫자로 저장됨
  - `AAVE = 0`
  - `COMPOUND_V2 = 1`
  - `COMPOUND_V3 = 2`

**왜 Enum을 사용하나요?**
- 코드 가독성 향상 (숫자 대신 이름 사용)
- 오타 방지
- 가스 비용 절약 (uint8로 저장)

### 2.6 Modifier (수정자)

```solidity
modifier onlyOwner() {
    require(msg.sender == owner(), "Not authorized");
    _;
}

function executeLiquidation(...) external onlyOwner {
    // 함수 내용
}
```

**설명:**
- **Modifier**: 함수 실행 전에 체크하는 "관문"
- `onlyOwner`: 오너만 실행 가능하도록 제한
- `_`: 원래 함수 코드가 실행될 위치 표시

**동작 순서:**
1. `onlyOwner` 체크 (msg.sender == owner?)
2. 통과하면 `_` 위치에서 원래 함수 실행
3. 실패하면 revert

### 2.7 Events (이벤트)

```solidity
event FlashLoanTriggered(
    address indexed asset,
    uint256 amount
);

emit FlashLoanTriggered(asset, 100 ether);
```

**설명:**
- **Event**: 블록체인에 기록되는 "로그"
- `indexed`: 검색 가능한 파라미터 (최대 3개)
- `emit`: 이벤트 발생시키기

**왜 Event를 사용하나요?**
- 프론트엔드에서 트랜잭션 상태 확인
- 가스 비용 저렴 (storage보다 훨씬 쌈)
- 디버깅 및 모니터링

### 2.8 Custom Errors (커스텀 에러)

```solidity
error InsufficientCollateral();
error SwapFailed();

function someFunction() {
    if (collateral < minRequired) {
        revert InsufficientCollateral();
    }
}
```

**설명:**
- Solidity 0.8.4+에서 도입된 기능
- 기존 `require` 문자열보다 가스 비용 저렴
- 더 명확한 에러 메시지

**비교:**
```solidity
// 옛날 방식 (가스 비용 높음)
require(collateral >= minRequired, "Insufficient collateral");

// 새로운 방식 (가스 비용 낮음)
if (collateral < minRequired) revert InsufficientCollateral();
```

### 2.9 Memory vs Storage vs Calldata

```solidity
function example(
    LiquidationParams calldata params,  // calldata
    LiquidationParams memory p          // memory
) {
    LiquidationParams storage stored;   // storage
}
```

**설명:**

| 위치 | 설명 | 사용처 | 가스 비용 |
|------|------|--------|----------|
| **storage** | 블록체인에 영구 저장 | 상태 변수 | 매우 높음 |
| **memory** | 임시 저장 (함수 실행 중) | 함수 내부 변수 | 중간 |
| **calldata** | 읽기 전용 (수정 불가) | 함수 파라미터 | 가장 낮음 |

**예시:**
```solidity
// storage: 블록체인에 영구 저장
mapping(address => uint256) public balances;  // storage

function processData(bytes calldata data) external {
    // calldata: 읽기만 가능 (가스 절약)
    uint256 value = abi.decode(data, (uint256));

    // memory: 임시 복사본 생성
    LiquidationParams memory params = LiquidationParams({...});
}
```

### 2.10 Try/Catch (예외 처리)

```solidity
try this._executeLiquidationLogic(asset, amount, premium, p) {
    // 성공시 실행
    return true;
} catch Error(string memory reason) {
    // 실패시 실행
    revert FlashLoanCallbackFailed();
}
```

**설명:**
- **Try**: 함수 실행 시도
- **Catch**: 실패시 처리
- external 함수 호출에만 사용 가능

**왜 Try/Catch를 사용하나요?**
- 에러 발생시 컨트랙트 전체가 멈추는 것을 방지
- 우아한 에러 핸들링 가능
- 더 안전한 컨트랙트 설계

---

## 3. 컨트랙트 구조 전체 분석

### 3.1 상속 구조

```solidity
contract LiquidationStrategy is
    FlashLoanSimpleReceiverBase,  // Aave 플래시론 기능
    ReentrancyGuard,                // 재진입 공격 방어
    Ownable                         // 소유자 권한 관리
{
    // ...
}
```

**각 부모 컨트랙트의 역할:**

#### FlashLoanSimpleReceiverBase
```solidity
// Aave v3 제공
abstract contract FlashLoanSimpleReceiverBase {
    IPool public immutable POOL;

    constructor(IPoolAddressesProvider provider) {
        POOL = IPool(provider.getPool());
    }

    function executeOperation(...) external virtual returns (bool);
}
```

**역할:**
- Aave 플래시론을 받을 수 있는 기본 기능 제공
- `POOL` 변수: Aave Pool 컨트랙트 주소 저장
- `executeOperation`: 플래시론 콜백 함수 (오버라이드 필수)

#### ReentrancyGuard
```solidity
// OpenZeppelin 제공
abstract contract ReentrancyGuard {
    uint256 private _status;

    modifier nonReentrant() {
        require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }
}
```

**역할:**
- 재진입 공격(Reentrancy Attack) 방어
- `nonReentrant` modifier로 함수 보호

**재진입 공격이란?**
```
1. 악성 컨트랙트가 우리 컨트랙트 함수 호출
2. 우리 컨트랙트가 악성 컨트랙트로 자금 전송
3. 악성 컨트랙트의 receive() 함수가 다시 우리 함수 호출
4. 상태 업데이트 전에 또 자금 빼감 (반복)
```

#### Ownable
```solidity
// OpenZeppelin 제공
abstract contract Ownable {
    address private _owner;

    modifier onlyOwner() {
        require(msg.sender == _owner, "Not owner");
        _;
    }

    function transferOwnership(address newOwner) public onlyOwner {
        _owner = newOwner;
    }
}
```

**역할:**
- 소유자 권한 관리
- `onlyOwner` modifier로 특정 함수 접근 제한

### 3.2 컨트랙트 구성 요소

```
LiquidationStrategy
│
├─ 📦 Imports & Interfaces
│  ├─ IAavePool (Aave v3 청산)
│  ├─ ICToken (Compound v2 청산)
│  ├─ IComet (Compound v3 청산)
│  └─ SafeERC20 (안전한 토큰 전송)
│
├─ 🔧 Data Structures
│  ├─ enum ProtocolType
│  └─ struct LiquidationParams
│
├─ 📢 Events (11개)
│  ├─ FlashLoanTriggered
│  ├─ AaveLiquidated
│  ├─ CompoundV2Liquidated
│  ├─ CompoundV3Absorbed
│  ├─ CollateralRedeemed
│  ├─ CollateralSwapped
│  ├─ FlashLoanRepaid
│  └─ ProfitRealized
│
├─ ❌ Custom Errors (6개)
│  ├─ InsufficientCollateral
│  ├─ SwapFailed
│  ├─ LiquidationFailed
│  ├─ InsufficientProfit
│  ├─ InvalidProtocol
│  └─ FlashLoanCallbackFailed
│
├─ 🔐 Main Functions
│  ├─ executeLiquidation() - 진입점
│  ├─ executeOperation() - 플래시론 콜백
│  └─ _executeLiquidationLogic() - 청산 로직
│
├─ 🏦 Protocol Functions
│  ├─ _executeLiquidation()
│  ├─ _executeAaveLiquidation()
│  ├─ _executeCompoundV2Liquidation()
│  └─ _executeCompoundV3Liquidation()
│
├─ 💱 Swap Functions
│  └─ _executeSwap()
│
└─ 🛠️ Utility Functions
   ├─ rescueToken()
   ├─ rescueETH()
   └─ _isContract()
```

### 3.3 함수 호출 흐름도

```
                  ┌─────────────────────┐
                  │  Owner (EOA)        │
                  └──────────┬──────────┘
                             │
                             ▼
          ┌──────────────────────────────────┐
          │  executeLiquidation()            │
          │  - 파라미터 검증                  │
          │  - 플래시론 요청                  │
          └──────────────┬───────────────────┘
                         │
                         ▼
                  ┌─────────────┐
                  │ Aave Pool   │
                  │ flashLoan   │
                  └──────┬──────┘
                         │
                         ▼
          ┌──────────────────────────────────┐
          │  executeOperation()              │
          │  - 플래시론 콜백                  │
          │  - 파라미터 디코딩                │
          │  - Premium 검증                   │
          └──────────────┬───────────────────┘
                         │
                         ▼
          ┌──────────────────────────────────┐
          │  _executeLiquidationLogic()      │
          │  - try/catch로 안전하게 실행      │
          └──────────────┬───────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
   ┌────────┐     ┌──────────┐    ┌──────────┐
   │ Aave   │     │Compound  │    │Compound  │
   │ v3     │     │v2        │    │v3        │
   └────┬───┘     └─────┬────┘    └─────┬────┘
        │               │               │
        └───────────────┼───────────────┘
                        │
                        ▼
          ┌──────────────────────────────────┐
          │  _executeSwap()                  │
          │  - 담보를 빚 토큰으로 교환        │
          └──────────────┬───────────────────┘
                         │
                         ▼
          ┌──────────────────────────────────┐
          │  플래시론 상환                    │
          │  - approve(POOL, amount+premium) │
          │  - 자동 상환                      │
          └──────────────┬───────────────────┘
                         │
                         ▼
                  ┌─────────────┐
                  │  수익 실현  │
                  │  (profit)   │
                  └─────────────┘
```

---

## 4. 라인별 상세 코드 분석

이제 컨트랙트의 모든 코드를 라인별로 분석하겠습니다.

### 4.1 라이선스 및 버전 선언 (Line 1-2)

```solidity
1: // SPDX-License-Identifier: MIT
2: pragma solidity ^0.8.19;
```

**라인별 설명:**

**Line 1: SPDX 라이선스**
- `SPDX-License-Identifier`: 소스코드 라이선스 식별자
- `MIT`: 가장 자유로운 오픈소스 라이선스
  - 상업적 사용 가능
  - 수정 가능
  - 재배포 가능
  - 보증 없음

**Line 2: Solidity 버전**
- `pragma solidity`: 컴파일러 버전 지정 키워드
- `^0.8.19`: 0.8.19 이상 0.9.0 미만
- 왜 0.8.x를 사용하나요?
  - 정수 오버플로우/언더플로우 자동 체크
  - Custom errors 지원 (가스 절약)
  - 더 나은 에러 메시지

### 4.2 Import 문 (Line 4-9)

```solidity
4: import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
5: import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
6: import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
7: import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
8: import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
9: import "@openzeppelin/contracts/access/Ownable.sol";
```

**라인별 설명:**

**Line 4: IPoolAddressesProvider**
```solidity
// Aave Pool 주소를 제공하는 인터페이스
interface IPoolAddressesProvider {
    function getPool() external view returns (address);
}
```
- Aave v3의 중앙 주소 관리자
- `getPool()`: 실제 Pool 컨트랙트 주소 반환
- 왜 필요한가? Pool 주소가 업그레이드되어도 동일한 Provider 사용 가능

**Line 5: FlashLoanSimpleReceiverBase**
```solidity
// 플래시론을 받기 위한 베이스 컨트랙트
abstract contract FlashLoanSimpleReceiverBase {
    IPool public immutable POOL;

    constructor(IPoolAddressesProvider provider) {
        POOL = IPool(provider.getPool());
    }

    function executeOperation(...) external virtual returns (bool);
}
```
- 플래시론 수신자가 구현해야 하는 기본 구조
- `POOL`: Aave Pool 컨트랙트 참조
- `executeOperation`: 플래시론 콜백 함수 (반드시 오버라이드)

**Line 6: IERC20**
```solidity
// ERC-20 토큰 표준 인터페이스
interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}
```
- 모든 ERC-20 토큰이 구현하는 표준 함수들
- 토큰 잔고 조회, 전송, 승인 등

**Line 7: SafeERC20**
```solidity
// 안전한 ERC-20 토큰 전송 래퍼
library SafeERC20 {
    function safeTransfer(IERC20 token, address to, uint256 value) internal;
    function safeApprove(IERC20 token, address spender, uint256 value) internal;
    // ...
}
```
- `using SafeERC20 for IERC20`: IERC20에 안전한 함수 추가
- 왜 "Safe"인가?
  - 일부 토큰은 `transfer()` 실패시 false 대신 revert
  - 일부 토큰은 반환값이 없음
  - SafeERC20은 이 모든 경우를 처리

**Line 8: ReentrancyGuard**
```solidity
// 재진입 공격 방어
abstract contract ReentrancyGuard {
    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED = 2;
    uint256 private _status;

    constructor() {
        _status = _NOT_ENTERED;
    }

    modifier nonReentrant() {
        require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }
}
```
- `nonReentrant` modifier: 함수 재진입 방지
- 작동 원리:
  1. 함수 시작시 `_status = _ENTERED`
  2. 함수 실행
  3. 함수 종료시 `_status = _NOT_ENTERED`
  4. 재진입 시도시 `_status == _ENTERED`이므로 revert

**Line 9: Ownable**
```solidity
// 소유자 권한 관리
abstract contract Ownable {
    address private _owner;

    constructor(address initialOwner) {
        _owner = initialOwner;
    }

    modifier onlyOwner() {
        require(msg.sender == _owner, "Ownable: caller is not the owner");
        _;
    }

    function owner() public view returns (address) {
        return _owner;
    }

    function transferOwnership(address newOwner) public onlyOwner {
        _owner = newOwner;
    }
}
```
- `onlyOwner`: 소유자만 호출 가능한 함수에 사용
- `transferOwnership`: 소유권 이전

### 4.3 인터페이스 정의 (Line 18-68)

#### IAavePool 인터페이스 (Line 18-35)

```solidity
18: interface IAavePool {
19:     function liquidationCall(
20:         address collateralAsset,
21:         address debtAsset,
22:         address user,
23:         uint256 debtToCover,
24:         bool receiveAToken
25:     ) external;
26:
27:     function getUserAccountData(address user) external view returns (
28:         uint256 totalCollateralBase,
29:         uint256 totalDebtBase,
30:         uint256 availableBorrowsBase,
31:         uint256 currentLiquidationThreshold,
32:         uint256 ltv,
33:         uint256 healthFactor
34:     );
35: }
```

**라인별 설명:**

**Line 19-25: liquidationCall 함수**

이 함수는 Aave v3의 핵심 청산 함수입니다.

**파라미터 상세:**

```solidity
address collateralAsset  // 담보 자산 주소 (예: WETH)
```
- 청산 대상자가 예치한 담보 토큰
- 예: 담보로 WETH를 예치했다면 WETH 주소

```solidity
address debtAsset       // 빚 자산 주소 (예: USDC)
```
- 청산 대상자가 빌린 토큰
- 예: USDC를 빌렸다면 USDC 주소

```solidity
address user            // 청산 대상자 주소
```
- Health Factor < 1.0인 사용자의 지갑 주소

```solidity
uint256 debtToCover     // 상환할 빚의 양
```
- 얼마나 많은 빚을 갚을 것인가
- 최대 50% (프로토콜 설정에 따라 다름)
- 예: 1000 USDC 빚이 있으면 최대 500 USDC 청산 가능

```solidity
bool receiveAToken      // aToken 수령 여부
```
- `true`: 담보를 aToken(이자 발생 토큰)으로 받음
- `false`: 담보를 underlying 토큰(예: WETH)으로 받음
- 대부분 `false` 사용 (즉시 현금화하기 위해)

**실행 결과:**
```
1. 우리 컨트랙트의 debtAsset이 차감됨 (빚 상환)
2. 우리 컨트랙트가 collateralAsset을 받음 (청산 보너스 포함)
3. 청산 보너스 = 보통 5-10% (프로토콜 설정)
```

**Line 27-34: getUserAccountData 함수**

사용자의 계정 상태를 조회하는 함수입니다.

**반환값 상세:**

```solidity
uint256 totalCollateralBase  // 총 담보 가치 (Base Currency)
```
- 모든 담보의 총 가치
- Base Currency = 보통 ETH 또는 USD
- 예: $10,000 상당의 담보

```solidity
uint256 totalDebtBase       // 총 빚 가치 (Base Currency)
```
- 모든 빚의 총 가치
- 예: $8,000 상당의 빚

```solidity
uint256 availableBorrowsBase // 추가로 빌릴 수 있는 양
```
- 현재 담보로 더 빌릴 수 있는 금액
- 예: $1,000 더 빌릴 수 있음

```solidity
uint256 currentLiquidationThreshold // 청산 임계값 (basis points)
```
- 담보 가치의 몇 %까지 빌릴 수 있는가
- Basis points = 10000 기준 (80% = 8000)
- 예: 8000 = 80% (담보의 80%까지 빌리면 청산)

```solidity
uint256 ltv                 // Loan-to-Value 비율 (basis points)
```
- 담보 대비 대출 가능 비율
- 예: 7500 = 75% (담보의 75%까지 빌릴 수 있음)

```solidity
uint256 healthFactor        // 건강도 (1e18 기준)
```
- **가장 중요한 값!**
- 1e18 = 1.0 (안전)
- < 1e18 = 청산 가능
- 계산: `(담보 * 청산임계값) / 빚`
- 예:
  - healthFactor = 1.5e18 → 안전 (150%)
  - healthFactor = 0.95e18 → 위험! 청산 가능

**Health Factor 계산 예시:**
```
담보: $10,000
빚: $8,000
청산 임계값: 80% (8000 basis points)

Health Factor = (10,000 * 0.8) / 8,000 = 1.0

→ 빚이 조금만 더 늘어나면 청산됨!
```

#### ICToken 인터페이스 (Line 38-50)

```solidity
38: interface ICToken {
39:     function liquidateBorrow(
40:         address borrower,
41:         uint256 repayAmount,
42:         address cTokenCollateral
43:     ) external returns (uint256);
44:
45:     function redeem(uint256 redeemTokens) external returns (uint256);
46:     function redeemUnderlying(uint256 redeemAmount) external returns (uint256);
47:     function balanceOf(address owner) external view returns (uint256);
48:     function underlying() external view returns (address);
49:     function exchangeRateStored() external view returns (uint256);
50: }
```

**라인별 설명:**

**Line 39-43: liquidateBorrow 함수**

Compound v2의 청산 함수입니다.

```solidity
address borrower         // 청산 대상자
```
- 빚을 갚지 못하는 사용자 주소

```solidity
uint256 repayAmount      // 상환할 금액 (underlying 기준)
```
- 얼마나 많은 빚을 갚을 것인가
- Compound는 최대 50% 청산 가능
- 예: 1000 USDC 빚이면 최대 500 USDC 청산

```solidity
address cTokenCollateral // 담보 cToken 주소
```
- 받을 담보의 cToken 주소
- 예: cETH, cUSDC 등
- **주의**: underlying이 아니라 cToken 주소!

**반환값:**
```solidity
uint256 errorCode       // 0 = 성공, 0이 아니면 실패
```
- Compound는 revert 대신 에러 코드 반환
- 0: 성공
- 다른 값: 실패 (이유별로 다른 코드)

**실행 결과:**
```
1. repayAmount만큼 underlying 토큰 차감 (빚 상환)
2. cTokenCollateral 토큰 수령 (청산 보너스 포함)
3. 청산 보너스 = 보통 8% (프로토콜 설정)
```

**Line 45-46: redeem 함수들**

cToken을 underlying 토큰으로 교환하는 함수입니다.

```solidity
function redeem(uint256 redeemTokens)
```
- **redeemTokens**: 교환할 cToken 개수
- cToken을 underlying으로 교환
- 예: 100 cETH → ? ETH (환율에 따라)

```solidity
function redeemUnderlying(uint256 redeemAmount)
```
- **redeemAmount**: 받고 싶은 underlying 개수
- underlying 기준으로 교환
- 예: 10 ETH를 받기 위해 ? cETH 차감

**반환값:** 0 = 성공, 0이 아니면 실패

**Line 47-49: 조회 함수들**

```solidity
function balanceOf(address owner) // cToken 잔고
```
- 특정 주소의 cToken 보유량
- ERC-20 표준 함수

```solidity
function underlying()              // underlying 토큰 주소
```
- cToken의 기본 자산 주소
- 예: cETH → WETH 주소 반환

```solidity
function exchangeRateStored()      // 환율
```
- cToken과 underlying의 교환 비율
- 1e18 기준
- 예: 2e17 = 0.2 (1 cToken = 0.2 underlying)

**환율 계산 예시:**
```
exchangeRate = 2e17 (0.2)
cToken 잔고 = 100

underlying = 100 * 0.2 = 20
```

#### IComet 인터페이스 (Line 53-68)

```solidity
53: interface IComet {
54:     function absorb(address absorber, address[] calldata accounts) external;
55:     function getAssetInfo(uint8 i) external view returns (AssetInfo memory);
56:     function numAssets() external view returns (uint8);
57:
58:     struct AssetInfo {
59:         uint8 offset;
60:         address asset;
61:         address priceFeed;
62:         uint64 scale;
63:         uint64 borrowCollateralFactor;
64:         uint64 liquidateCollateralFactor;
65:         uint64 liquidationFactor;
66:         uint128 supplyCap;
67:     }
68: }
```

**라인별 설명:**

**Line 54: absorb 함수**

Compound v3의 청산 함수입니다 (v2와 완전히 다름!).

```solidity
function absorb(address absorber, address[] calldata accounts)
```

**파라미터:**
```solidity
address absorber         // 흡수자 (청산 실행자)
```
- 보통 `address(this)` 사용
- 청산 보상을 받을 주소

```solidity
address[] calldata accounts // 청산 대상자 배열
```
- 한 번에 여러 계정 청산 가능!
- 예: `[0x123..., 0x456..., 0x789...]`

**Compound v3의 특징:**

1. **자동 청산 보상 지급**
   - v2처럼 cToken을 받지 않음
   - 직접 underlying 토큰 수령
   - 프로토콜이 자동으로 담보 매각 후 보상 지급

2. **배치 청산**
   - 여러 계정을 한 번에 청산 가능
   - 가스 비용 절약

3. **간단한 인터페이스**
   - redeem 필요 없음
   - 한 번의 호출로 완료

**Line 58-67: AssetInfo 구조체**

Compound v3의 자산 정보를 담는 구조체입니다.

```solidity
struct AssetInfo {
    uint8 offset;                      // 데이터 오프셋
    address asset;                     // 자산 주소
    address priceFeed;                 // 가격 오라클 주소
    uint64 scale;                      // 자산 스케일
    uint64 borrowCollateralFactor;     // 대출 담보 비율
    uint64 liquidateCollateralFactor;  // 청산 담보 비율
    uint64 liquidationFactor;          // 청산 보너스
    uint128 supplyCap;                 // 공급 한도
}
```

**필드 상세:**

```solidity
uint8 offset
```
- 내부 데이터 구조의 오프셋
- 개발자가 직접 사용할 일 없음

```solidity
address asset
```
- 자산의 ERC-20 주소
- 예: WETH, USDC 등

```solidity
address priceFeed
```
- Chainlink 가격 오라클 주소
- 자산 가격을 가져오는 곳

```solidity
uint64 scale
```
- 자산의 소수점 스케일
- 대부분 1e18 (18 decimals)
- USDC는 1e6 (6 decimals)

```solidity
uint64 borrowCollateralFactor
```
- 대출 가능 비율 (1e18 기준)
- 예: 0.8e18 = 담보의 80%까지 빌릴 수 있음

```solidity
uint64 liquidateCollateralFactor
```
- 청산 가능 비율 (1e18 기준)
- 예: 0.85e18 = 담보/빚 비율이 85% 이하면 청산

```solidity
uint64 liquidationFactor
```
- 청산 보너스 비율 (1e18 기준)
- 예: 0.05e18 = 5% 보너스

```solidity
uint128 supplyCap
```
- 최대 공급 한도
- 프로토콜의 리스크 관리

### 4.4 컨트랙트 선언 및 상속 (Line 70)

```solidity
70: contract LiquidationStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard, Ownable {
```

**라인 분석:**

**다중 상속 구조:**
```
        LiquidationStrategy
               │
    ┌──────────┼──────────┐
    │          │          │
FlashLoan  Reentrancy  Ownable
SimpleReceiver Guard
BaseContract
```

**각 부모 컨트랙트의 기능:**

1. **FlashLoanSimpleReceiverBase**
   - Aave 플래시론 수신 기능
   - `POOL` 변수 제공
   - `executeOperation()` 구현 필수

2. **ReentrancyGuard**
   - `nonReentrant` modifier 제공
   - 재진입 공격 방어

3. **Ownable**
   - `onlyOwner` modifier 제공
   - 소유권 관리 기능

**상속 순서의 중요성:**
```solidity
// 올바른 순서
contract A is B, C, D { }

// Solidity는 C3 선형화 알고리즘 사용
// 오른쪽에서 왼쪽으로 우선순위
// D > C > B 순서로 함수 오버라이드
```

**우리 컨트랙트의 경우:**
```
Ownable > ReentrancyGuard > FlashLoanSimpleReceiverBase
```

### 4.5 SafeERC20 사용 선언 (Line 71)

```solidity
71:     using SafeERC20 for IERC20;
```

**라인 분석:**

**`using A for B` 문법:**
- 라이브러리 A의 함수를 타입 B에 추가
- 마치 B의 메서드처럼 사용 가능

**예시:**
```solidity
// using 선언 전
SafeERC20.safeTransfer(token, recipient, amount);

// using 선언 후
token.safeTransfer(recipient, amount);  // 더 깔끔!
```

**SafeERC20의 안전한 함수들:**

```solidity
library SafeERC20 {
    function safeTransfer(IERC20 token, address to, uint256 value) internal {
        _callOptionalReturn(token, abi.encodeWithSelector(
            token.transfer.selector, to, value
        ));
    }

    function safeApprove(IERC20 token, address spender, uint256 value) internal {
        // 먼저 0으로 리셋 (일부 토큰 요구사항)
        require(
            (value == 0) || (token.allowance(address(this), spender) == 0),
            "SafeERC20: approve from non-zero to non-zero allowance"
        );
        _callOptionalReturn(token, abi.encodeWithSelector(
            token.approve.selector, spender, value
        ));
    }

    function _callOptionalReturn(IERC20 token, bytes memory data) private {
        bytes memory returndata = address(token).functionCall(data);
        if (returndata.length > 0) {
            require(
                abi.decode(returndata, (bool)),
                "SafeERC20: ERC20 operation did not succeed"
            );
        }
    }
}
```

**왜 "Safe"인가?**

**문제 1: 반환값 없는 토큰**
```solidity
// USDT는 반환값이 없음
// 일반 transfer() 사용시 컴파일 에러!
USDT.transfer(recipient, amount);  // ❌ 에러

// SafeERC20은 반환값이 없어도 OK
USDT.safeTransfer(recipient, amount);  // ✅ 성공
```

**문제 2: false 대신 revert하는 토큰**
```solidity
// 일부 토큰은 실패시 false 반환
bool success = token.transfer(recipient, amount);
if (!success) {
    // 이 코드가 실행되어야 하는데...
}

// 그런데 일부 토큰은 revert 발생!
// if 체크 코드가 실행되지 않음
```

**문제 3: Approve 0으로 리셋 필요**
```solidity
// 일부 토큰(USDT)은 0이 아닌 값에서 다른 값으로
// approve 변경시 revert 발생
token.approve(spender, 100);  // ✅ OK
token.approve(spender, 200);  // ❌ 에러! (USDT)

// 올바른 방법
token.approve(spender, 0);    // 먼저 0으로
token.approve(spender, 200);  // 그 다음 새 값

// SafeERC20은 자동으로 처리
token.safeApprove(spender, 200);  // ✅ 내부에서 0으로 리셋
```

### 4.6 Enum 정의 (Line 74)

```solidity
74:     enum ProtocolType { AAVE, COMPOUND_V2, COMPOUND_V3 }
```

**라인 분석:**

**Enum(열거형)이란?**
- 미리 정의된 상수들의 집합
- 0부터 시작하는 정수로 저장

**우리 Enum의 값:**
```solidity
ProtocolType.AAVE = 0
ProtocolType.COMPOUND_V2 = 1
ProtocolType.COMPOUND_V3 = 2
```

**장점:**

1. **가독성**
```solidity
// Enum 사용 전 (나쁨)
function liquidate(uint8 protocolType) {
    if (protocolType == 0) {
        // Aave 청산
    }
}

// Enum 사용 후 (좋음)
function liquidate(ProtocolType protocolType) {
    if (protocolType == ProtocolType.AAVE) {
        // Aave 청산
    }
}
```

2. **타입 안정성**
```solidity
// Enum 사용 전
liquidate(5);  // ✅ 컴파일 성공 (하지만 잘못된 값!)

// Enum 사용 후
liquidate(5);  // ❌ 컴파일 에러
liquidate(ProtocolType.AAVE);  // ✅ 올바름
```

3. **가스 절약**
```solidity
// uint256 사용
uint256 protocolType = 0;  // 32 bytes 사용

// enum 사용
ProtocolType protocolType = ProtocolType.AAVE;  // 1 byte 사용!
```

**사용 예시:**
```solidity
// 변수 선언
ProtocolType protocol = ProtocolType.AAVE;

// 비교
if (protocol == ProtocolType.AAVE) {
    // Aave 로직
} else if (protocol == ProtocolType.COMPOUND_V2) {
    // Compound v2 로직
}

// switch-case (Solidity에는 없지만 if-else로 구현)
function executeByProtocol(ProtocolType protocol) internal {
    if (protocol == ProtocolType.AAVE) {
        _executeAave();
    } else if (protocol == ProtocolType.COMPOUND_V2) {
        _executeCompoundV2();
    } else if (protocol == ProtocolType.COMPOUND_V3) {
        _executeCompoundV3();
    } else {
        revert InvalidProtocol();
    }
}

// 함수 파라미터
function setProtocol(ProtocolType newProtocol) external {
    currentProtocol = newProtocol;
}
```

### 4.7 LiquidationParams 구조체 (Line 77-88)

```solidity
77:     struct LiquidationParams {
78:         ProtocolType protocolType;
79:         address protocol;
80:         address user;
81:         address collateralAsset;
82:         address debtAsset;
83:         uint256 debtToCover;
84:         address dexRouter;
85:         bytes swapCalldata;
86:         uint256 minCollateralOut;
87:         uint256 flashLoanPremium;
88:     }
```

**라인별 상세 분석:**

**Line 78: ProtocolType protocolType**
```solidity
ProtocolType protocolType;  // 청산할 프로토콜 종류
```
- 값: `AAVE`, `COMPOUND_V2`, `COMPOUND_V3`
- 용도: 어떤 청산 로직을 실행할지 결정
- 예시:
```solidity
params.protocolType = ProtocolType.AAVE;
// → _executeAaveLiquidation() 실행
```

**Line 79: address protocol**
```solidity
address protocol;  // 프로토콜 컨트랙트 주소
```
- **Aave**: Pool 컨트랙트 주소
  - 예: `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` (Mainnet)
- **Compound v2**: cToken 주소
  - 예: `0x5d3a536E4D6DbD6114cc1Ead35777bAB948E3643` (cDAI)
- **Compound v3**: Comet 컨트랙트 주소
  - 예: `0xc3d688B66703497DAA19211EEdff47f25384cdc3` (USDC Comet)

**Line 80: address user**
```solidity
address user;  // 청산 대상자 지갑 주소
```
- Health Factor < 1.0인 사용자
- 예: `0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb` (청산당할 사람)

**Line 81: address collateralAsset**
```solidity
address collateralAsset;  // 담보 자산 주소
```
- **Aave**: Underlying 토큰 주소
  - 예: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (WETH)
- **Compound v2**: cToken 주소
  - 예: `0x4Ddc2D193948926D02f9B1fE9e1daa0718270ED5` (cETH)
- **Compound v3**: Underlying 토큰 주소
  - 예: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (WETH)

**중요:** Compound v2만 cToken 주소를 사용!

**Line 82: address debtAsset**
```solidity
address debtAsset;  // 빚 자산 주소
```
- **Aave**: Underlying 토큰 주소
  - 예: `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` (USDC)
- **Compound v2**: cToken 주소
  - 예: `0x39AA39c021dfbaE8faC545936693aC917d5E7563` (cUSDC)
- **Compound v3**: Underlying 토큰 주소
  - 예: `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` (USDC)

**Line 83: uint256 debtToCover**
```solidity
uint256 debtToCover;  // 상환할 빚의 양
```
- Underlying 토큰 기준 (Wei 단위)
- 제한: 보통 최대 50%까지 청산 가능
- 예시:
```solidity
// 사용자가 1000 USDC 빚
// USDC는 6 decimals
debtToCover = 500 * 10**6;  // 500 USDC (50%)
```

**왜 50%까지만?**
- 한 번에 모두 청산하면 가격 충격(Price Impact) 발생
- 점진적 청산으로 시장 안정성 유지
- 청산 대상자에게 회복 기회 제공

**Line 84: address dexRouter**
```solidity
address dexRouter;  // DEX 라우터 주소
```
- 담보를 빚 토큰으로 교환할 DEX
- 지원 DEX 예시:
  - Uniswap V3: `0xE592427A0AEce92De3Edee1F18E0157C05861564`
  - 1inch: `0x1111111254EEB25477B68fb85Ed929f73A960582`
  - 0x: `0xDef1C0ded9bec7F1a1670819833240f027b25EfF`
  - Paraswap: `0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57`

**Line 85: bytes swapCalldata**
```solidity
bytes swapCalldata;  // DEX 호출 데이터
```
- DEX 라우터 함수를 호출하기 위한 인코딩된 데이터
- 오프체인(백엔드)에서 생성
- 포함 정보:
  - 함수 셀렉터 (4 bytes)
  - 파라미터 (인코딩됨)
  - 슬리피지 설정
  - 경로(path) 정보

**예시 (Uniswap V3):**
```javascript
// JavaScript/TypeScript (백엔드)
const swapCalldata = router.interface.encodeFunctionData(
    'exactInputSingle',
    [{
        tokenIn: WETH_ADDRESS,
        tokenOut: USDC_ADDRESS,
        fee: 3000,  // 0.3%
        recipient: liquidationContract.address,
        deadline: Math.floor(Date.now() / 1000) + 300,
        amountIn: collateralAmount,
        amountOutMinimum: minUSDC,
        sqrtPriceLimitX96: 0
    }]
);
```

**Line 86: uint256 minCollateralOut**
```solidity
uint256 minCollateralOut;  // 최소 수령 담보량
```
- 슬리피지 보호
- 이 값보다 적게 받으면 revert
- 계산 예시:
```solidity
// 예상 담보: 10 ETH
// 슬리피지 허용: 1%
minCollateralOut = 10 ether * 99 / 100;  // 9.9 ETH
```

**Line 87: uint256 flashLoanPremium**
```solidity
uint256 flashLoanPremium;  // 예상 플래시론 수수료
```
- Aave 플래시론 수수료 (보통 0.09%)
- 검증용으로 사용 (±10% 허용)
- 계산:
```solidity
// 1000 USDC 빌림
// 수수료 0.09%
flashLoanPremium = 1000 * 10**6 * 9 / 10000;  // 0.9 USDC
```

**전체 구조체 사용 예시:**
```solidity
LiquidationParams memory params = LiquidationParams({
    protocolType: ProtocolType.AAVE,
    protocol: 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2,  // Aave Pool
    user: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb,      // 청산 대상
    collateralAsset: 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2,  // WETH
    debtAsset: 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48,       // USDC
    debtToCover: 500 * 10**6,                               // 500 USDC
    dexRouter: 0xE592427A0AEce92De3Edee1F18E0157C05861564,  // Uniswap V3
    swapCalldata: hex"414bf389...",                         // 인코딩된 스왑 데이터
    minCollateralOut: 0.99 ether,                           // 최소 0.99 ETH
    flashLoanPremium: 450000                                // 0.45 USDC (0.09%)
});
```

### 4.8 Events 선언 (Line 91-147)

Events는 블록체인에 기록되는 로그로, 가스 비용이 저렴하고 프론트엔드에서 쉽게 읽을 수 있습니다.

#### Event 1: FlashLoanTriggered (Line 91-96)

```solidity
91:     event FlashLoanTriggered(
92:         address indexed asset,
93:         uint256 amount,
94:         address indexed user,
95:         ProtocolType protocolType
96:     );
```

**용도:** 플래시론이 시작되었음을 기록

**파라미터:**
```solidity
address indexed asset      // 빌린 자산 (검색 가능)
uint256 amount             // 빌린 양
address indexed user       // 청산 대상자 (검색 가능)
ProtocolType protocolType  // 프로토콜 종류
```

**`indexed` 키워드:**
- 최대 3개까지 사용 가능
- 검색 및 필터링 가능
- 약간의 가스 비용 추가

**사용 예시:**
```solidity
emit FlashLoanTriggered(
    USDC,                    // asset
    500 * 10**6,             // amount (500 USDC)
    0x742d35Cc...,           // user
    ProtocolType.AAVE        // protocolType
);
```

**프론트엔드에서 읽기:**
```javascript
// Web3.js / Ethers.js
const filter = contract.filters.FlashLoanTriggered(
    USDC_ADDRESS,  // asset으로 필터
    null,          // amount (필터 안함)
    null           // user (필터 안함)
);

const events = await contract.queryFilter(filter);
```

#### Event 2: AaveLiquidated (Line 98-104)

```solidity
98:     event AaveLiquidated(
99:         address indexed user,
100:        address indexed collateralAsset,
101:        address indexed debtAsset,
102:        uint256 debtToCover,
103:        uint256 liquidationBonus
104:    );
```

**용도:** Aave 청산 성공 기록

**파라미터:**
```solidity
address indexed user              // 청산된 사용자
address indexed collateralAsset   // 담보 자산
address indexed debtAsset         // 빚 자산
uint256 debtToCover               // 상환한 빚
uint256 liquidationBonus          // 받은 청산 보너스
```

**liquidationBonus 계산:**
```
청산 보너스 = 받은 담보 - (상환한 빚의 담보 가치)

예시:
- 500 USDC 빚 상환 (@ $1 = $500)
- 0.3 ETH 담보 수령 (@ $2000 = $600)
- 청산 보너스 = $600 - $500 = $100 (약 5%)
```

#### Event 3: CompoundV2Liquidated (Line 106-113)

```solidity
106:    event CompoundV2Liquidated(
107:        address indexed user,
108:        address indexed cTokenBorrowed,
109:        address indexed cTokenCollateral,
110:        uint256 repayAmount,
111:        uint256 seizeTokens
112:    );
```

**용도:** Compound v2 청산 성공 기록

**파라미터:**
```solidity
address indexed user              // 청산된 사용자
address indexed cTokenBorrowed    // 빚 cToken
address indexed cTokenCollateral  // 담보 cToken
uint256 repayAmount               // 상환한 양 (underlying)
uint256 seizeTokens               // 획득한 cToken 양
```

**Compound v2 특징:**
- cToken 단위로 기록
- `seizeTokens`: 청산으로 받은 cToken 개수
- 나중에 redeem하여 underlying 획득

#### Event 4: CompoundV3Absorbed (Line 115-119)

```solidity
115:    event CompoundV3Absorbed(
116:        address indexed user,
117:        address indexed comet,
118:        uint256 assetsAbsorbed
119:    );
```

**용도:** Compound v3 청산 성공 기록

**파라미터:**
```solidity
address indexed user        // 청산된 사용자
address indexed comet       // Comet 컨트랙트 주소
uint256 assetsAbsorbed      // 흡수한 자산 양
```

**Compound v3 특징:**
- `absorb()` 메커니즘 사용
- 직접 underlying 자산 획득
- cToken 없음

#### Event 5: CollateralRedeemed (Line 121-126)

```solidity
121:    event CollateralRedeemed(
122:        address indexed token,
123:        uint256 amount,
124:        address indexed underlying,
125:        uint256 underlyingReceived
126:    );
```

**용도:** cToken을 underlying으로 교환 기록

**파라미터:**
```solidity
address indexed token       // cToken 주소
uint256 amount              // 교환한 cToken 양
address indexed underlying  // Underlying 토큰 주소
uint256 underlyingReceived  // 받은 underlying 양
```

**사용 시나리오:**
```
Compound v2 청산시:
1. liquidateBorrow() → cETH 획득
2. redeem(cETH) → ETH 획득
3. CollateralRedeemed 이벤트 발생
```

#### Event 6: CollateralSwapped (Line 128-135)

```solidity
128:    event CollateralSwapped(
129:        address indexed router,
130:        address indexed tokenIn,
131:        address indexed tokenOut,
132:        uint256 amountIn,
133:        uint256 amountOut,
134:        uint256 minAmountOut
135:    );
```

**용도:** DEX 스왑 성공 기록

**파라미터:**
```solidity
address indexed router     // DEX 라우터 주소
address indexed tokenIn    // 입력 토큰 (담보)
address indexed tokenOut   // 출력 토큰 (빚 상환용)
uint256 amountIn           // 스왑한 양
uint256 amountOut          // 받은 양
uint256 minAmountOut       // 최소 수령량 설정
```

**예시:**
```solidity
emit CollateralSwapped(
    0xE592427A0AEce92De3Edee1F18E0157C05861564,  // Uniswap V3
    WETH,                                         // tokenIn
    USDC,                                         // tokenOut
    0.5 ether,                                    // amountIn (0.5 ETH)
    1000 * 10**6,                                 // amountOut (1000 USDC)
    990 * 10**6                                   // minAmountOut (990 USDC, 1% 슬리피지)
);
```

#### Event 7: FlashLoanRepaid (Line 137-142)

```solidity
137:    event FlashLoanRepaid(
138:        address indexed asset,
139:        uint256 amount,
140:        uint256 premium,
141:        uint256 totalRepaid
142:    );
```

**용도:** 플래시론 상환 완료 기록

**파라미터:**
```solidity
address indexed asset  // 상환한 자산
uint256 amount         // 원금
uint256 premium        // 수수료
uint256 totalRepaid    // 총 상환액 (원금 + 수수료)
```

**계산:**
```solidity
totalRepaid = amount + premium

예시:
amount = 1000 USDC
premium = 0.9 USDC (0.09%)
totalRepaid = 1000.9 USDC
```

#### Event 8: ProfitRealized (Line 144-148)

```solidity
144:    event ProfitRealized(
145:        address indexed asset,
146:        uint256 profit,
147:        address indexed user
148:    );
```

**용도:** 최종 수익 기록

**파라미터:**
```solidity
address indexed asset  // 수익 자산
uint256 profit         // 순수익
address indexed user   // 청산 대상자
```

**수익 계산:**
```
순수익 = 스왑 결과 - (플래시론 원금 + 수수료)

예시:
1. 1000 USDC 플래시론
2. 500 USDC로 청산
3. 0.3 ETH 담보 수령
4. 0.3 ETH → 600 USDC 스왑
5. 1000.9 USDC 상환 (원금 + 0.09% 수수료)
6. 순수익 = 600 - 1000.9 + (1000 - 500) = 99.1 USDC
```

**실제로는:**
```
순수익 = 받은 담보 스왑 금액 - 플래시론 총 상환액
순수익 = 600 USDC - 1000.9 USDC = -400.9 USDC (?)

아니다! 플래시론 1000 USDC 중 500만 사용:
순수익 = (1000 - 500) + 600 - 1000.9
순수익 = 99.1 USDC ✅
```

### 4.9 Custom Errors (Line 150-155)

```solidity
150:    error InsufficientCollateral();
151:    error SwapFailed();
152:    error LiquidationFailed();
153:    error InsufficientProfit();
154:    error InvalidProtocol();
155:    error FlashLoanCallbackFailed();
```

**Custom Errors의 장점:**

**1. 가스 비용 절약**
```solidity
// 옛날 방식 (비쌈)
require(collateral >= minRequired, "Insufficient collateral");
// 가스: ~1000

// 새로운 방식 (저렴)
if (collateral < minRequired) revert InsufficientCollateral();
// 가스: ~50
```

**2. 더 명확한 에러 메시지**
```solidity
// 프론트엔드에서 감지하기 쉬움
try {
    await contract.executeLiquidation(...);
} catch (error) {
    if (error.message.includes('InsufficientCollateral')) {
        alert('담보가 부족합니다!');
    }
}
```

**각 에러의 의미:**

**InsufficientCollateral()**
```solidity
// 받은 담보가 최소 요구량보다 적을 때
if (collateralReceived < params.minCollateralOut) {
    revert InsufficientCollateral();
}
```

**SwapFailed()**
```solidity
// DEX 스왑 실행 실패시
(bool success, ) = router.call(swapData);
if (!success) {
    revert SwapFailed();
}
```

**LiquidationFailed()**
```solidity
// 청산 함수 호출 실패시
uint256 result = ICToken(cToken).liquidateBorrow(...);
if (result != 0) {
    revert LiquidationFailed();
}
```

**InsufficientProfit()**
```solidity
// 수익이 안 나면 (손해)
if (debtTokensReceived < totalOwed) {
    revert InsufficientProfit();
}
```

**InvalidProtocol()**
```solidity
// 지원하지 않는 프로토콜
if (protocolType != AAVE && protocolType != COMPOUND_V2 && protocolType != COMPOUND_V3) {
    revert InvalidProtocol();
}
```

**FlashLoanCallbackFailed()**
```solidity
// 청산 로직 실행 실패 (try/catch)
try this._executeLiquidationLogic(...) {
    // 성공
} catch {
    revert FlashLoanCallbackFailed();
}
```

### 4.10 Constructor (Line 157-160)

```solidity
157:    constructor(IPoolAddressesProvider provider)
158:        FlashLoanSimpleReceiverBase(provider)
159:        Ownable(msg.sender)
160:    {}
```

**라인별 분석:**

**Line 157: constructor 선언**
```solidity
constructor(IPoolAddressesProvider provider)
```
- `constructor`: 컨트랙트 배포시 단 한 번만 실행
- `IPoolAddressesProvider provider`: Aave Pool 주소 제공자

**왜 Pool 주소를 직접 받지 않나요?**
```solidity
// ❌ 나쁜 방식
constructor(address pool) { }
// Pool 주소가 바뀌면 컨트랙트 재배포 필요

// ✅ 좋은 방식
constructor(IPoolAddressesProvider provider) { }
// Provider에서 항상 최신 Pool 주소 가져옴
```

**Line 158: 부모 constructor 호출 - FlashLoanSimpleReceiverBase**
```solidity
FlashLoanSimpleReceiverBase(provider)
```

**부모 컨트랙트의 constructor:**
```solidity
abstract contract FlashLoanSimpleReceiverBase {
    IPool public immutable POOL;
    IPoolAddressesProvider public immutable ADDRESSES_PROVIDER;

    constructor(IPoolAddressesProvider provider) {
        ADDRESSES_PROVIDER = provider;
        POOL = IPool(provider.getPool());
    }
}
```

**실행 과정:**
1. `ADDRESSES_PROVIDER = provider` 저장
2. `provider.getPool()` 호출하여 Pool 주소 가져오기
3. `POOL = IPool(...)` 저장

**`immutable` 키워드:**
- 배포시 한 번만 설정 가능
- 이후 변경 불가
- 가스 비용 절약 (storage 대신 bytecode에 포함)

**Line 159: 부모 constructor 호출 - Ownable**
```solidity
Ownable(msg.sender)
```

**부모 컨트랙트의 constructor:**
```solidity
abstract contract Ownable {
    address private _owner;

    constructor(address initialOwner) {
        _owner = initialOwner;
        emit OwnershipTransferred(address(0), initialOwner);
    }
}
```

**`msg.sender`:**
- 컨트랙트를 배포하는 주소
- 배포자가 자동으로 소유자가 됨

**Line 160: 빈 body**
```solidity
{}
```
- 추가 초기화 로직 없음
- 부모 constructor들만 실행

**전체 실행 순서:**
```
1. Ownable(msg.sender)
   └─ _owner = msg.sender

2. FlashLoanSimpleReceiverBase(provider)
   └─ ADDRESSES_PROVIDER = provider
   └─ POOL = IPool(provider.getPool())

3. LiquidationStrategy constructor body
   └─ (비어있음)
```

**배포 예시:**
```javascript
// Hardhat / Ethers.js
const LiquidationStrategy = await ethers.getContractFactory("LiquidationStrategy");
const strategy = await LiquidationStrategy.deploy(
    "0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e"  // Aave PoolAddressesProvider (Mainnet)
);
```

### 4.11 executeLiquidation 함수 (Line 168-187)

이 함수는 청산의 진입점(Entry Point)입니다.

```solidity
168:    function executeLiquidation(
169:        address asset,
170:        uint256 amount,
171:        LiquidationParams calldata params
172:    ) external onlyOwner nonReentrant {
173:        require(amount >= params.debtToCover, "Insufficient flash loan amount");
174:        require(params.user != address(0), "Invalid user address");
175:        require(params.protocol != address(0), "Invalid protocol address");
176:
177:        emit FlashLoanTriggered(asset, amount, params.user, params.protocolType);
178:
179:        // Trigger flash loan
180:        POOL.flashLoanSimple(
181:            address(this),
182:            asset,
183:            amount,
184:            abi.encode(params),
185:            0
186:        );
187:    }
```

**라인별 상세 분석:**

**Line 168-172: 함수 시그니처**

```solidity
function executeLiquidation(
    address asset,                      // 플래시론 자산 (빚 토큰)
    uint256 amount,                     // 플래시론 양
    LiquidationParams calldata params   // 청산 파라미터
) external onlyOwner nonReentrant
```

**파라미터 설명:**

```solidity
address asset
```
- 플래시론으로 빌릴 자산
- 보통 청산 대상자의 빚 토큰과 동일
- 예: USDC, DAI, WETH 등

```solidity
uint256 amount
```
- 빌릴 양 (Wei 단위)
- `params.debtToCover` 이상이어야 함
- 예: 1000 USDC = `1000 * 10**6`

```solidity
LiquidationParams calldata params
```
- 청산에 필요한 모든 파라미터
- `calldata`: 읽기 전용, 가스 절약

**Modifiers:**

```solidity
external
```
- 외부에서만 호출 가능
- 내부 호출 불가 (`this.executeLiquidation()` 필요)
- 가스 비용 절약 (public보다 저렴)

```solidity
onlyOwner
```
```solidity
modifier onlyOwner() {
    require(msg.sender == owner(), "Not owner");
    _;
}
```
- 소유자만 호출 가능
- 무단 청산 방지

```solidity
nonReentrant
```
```solidity
modifier nonReentrant() {
    require(_status != _ENTERED, "Reentrant call");
    _status = _ENTERED;
    _;
    _status = _NOT_ENTERED;
}
```
- 재진입 공격 방지
- 함수 실행 중 다시 호출 불가

**Line 173: 플래시론 양 검증**

```solidity
require(amount >= params.debtToCover, "Insufficient flash loan amount");
```

**왜 이 검사가 필요한가?**

```
debtToCover = 500 USDC  (상환할 빚)
amount = 400 USDC       (플래시론)

→ 400 < 500 이므로 빚을 못 갚음!
```

**올바른 경우:**
```
debtToCover = 500 USDC
amount = 1000 USDC      (여유있게 빌림)

→ 500 USDC로 청산하고, 나머지는 컨트랙트에 남음
```

**Line 174: 사용자 주소 검증**

```solidity
require(params.user != address(0), "Invalid user address");
```

**`address(0)`이란?**
```solidity
address(0) = 0x0000000000000000000000000000000000000000
```
- "null" 주소
- 토큰 소각 주소
- 유효하지 않은 주소

**왜 체크하나?**
- 실수로 빈 주소 전달 방지
- 가스 낭비 방지

**Line 175: 프로토콜 주소 검증**

```solidity
require(params.protocol != address(0), "Invalid protocol address");
```

- Aave Pool, Compound cToken, Comet 주소가 유효한지 확인
- 잘못된 주소로 호출하면 실패

**Line 177: 이벤트 발생**

```solidity
emit FlashLoanTriggered(asset, amount, params.user, params.protocolType);
```

**프론트엔드/백엔드 모니터링:**
```javascript
// 이벤트 리스닝
contract.on('FlashLoanTriggered', (asset, amount, user, protocolType) => {
    console.log(`청산 시작: ${user}`);
    console.log(`자산: ${asset}, 양: ${amount}`);
    console.log(`프로토콜: ${protocolType}`);
});
```

**Line 180-186: 플래시론 요청**

```solidity
POOL.flashLoanSimple(
    address(this),        // receiverAddress
    asset,                // 빌릴 자산
    amount,               // 빌릴 양
    abi.encode(params),   // 전달할 데이터
    0                     // referralCode
);
```

**파라미터 상세:**

**1. receiverAddress: `address(this)`**
```solidity
address(this)  // 이 컨트랙트 주소
```
- 플래시론 콜백을 받을 주소
- 우리 컨트랙트의 `executeOperation()` 함수가 호출됨

**2. asset: 빌릴 자산**
```solidity
asset  // USDC, DAI, WETH 등
```
- Aave Pool에 유동성이 있어야 함
- 없으면 revert

**3. amount: 빌릴 양**
```solidity
amount  // Wei 단위
```
- Pool 유동성보다 많으면 revert
- 예: 1000 USDC = `1000 * 10**6`

**4. params: `abi.encode(params)`**
```solidity
abi.encode(params)  // LiquidationParams를 bytes로 인코딩
```

**인코딩 과정:**
```solidity
// LiquidationParams 구조체
params = LiquidationParams({...});

// bytes로 변환
bytes memory data = abi.encode(params);

// 플래시론 콜백에서 디코딩
LiquidationParams memory decoded = abi.decode(data, (LiquidationParams));
```

**5. referralCode: `0`**
```solidity
0  // 리퍼럴 코드 (사용 안 함)
```
- Aave의 리퍼럴 시스템
- 0 = 리퍼럴 없음

**플래시론 실행 과정:**

```
1. POOL.flashLoanSimple() 호출
2. Aave Pool이 asset을 우리 컨트랙트로 전송
3. Aave Pool이 우리 컨트랙트의 executeOperation() 호출
4. executeOperation()에서 청산 실행
5. executeOperation()에서 플래시론 상환 승인
6. Aave Pool이 자동으로 자금 회수
7. 성공하면 함수 종료, 실패하면 모든 것 revert
```

**예시 호출:**

```javascript
// Ethers.js
await liquidationStrategy.executeLiquidation(
    USDC_ADDRESS,                       // asset
    ethers.parseUnits("1000", 6),       // amount (1000 USDC)
    {                                   // params
        protocolType: 0,                // AAVE
        protocol: AAVE_POOL_ADDRESS,
        user: "0x742d35Cc...",
        collateralAsset: WETH_ADDRESS,
        debtAsset: USDC_ADDRESS,
        debtToCover: ethers.parseUnits("500", 6),
        dexRouter: UNISWAP_V3_ROUTER,
        swapCalldata: "0x414bf389...",
        minCollateralOut: ethers.parseEther("0.99"),
        flashLoanPremium: ethers.parseUnits("0.9", 6)
    }
);
```

(문서 계속... 이어서 작성하겠습니다)

---

## 계속 작성 중...

이 문서는 총 100+ 페이지 분량으로 다음 섹션들이 계속 이어집니다:

- executeOperation 함수 상세 분석
- _executeLiquidationLogic 함수 분석
- 프로토콜별 청산 함수들 (Aave, Compound v2/v3)
- DEX 스왑 로직
- 보안 및 에러 처리
- 실전 사용 예제
- FAQ 및 트러블슈팅

전체 문서를 완성하려면 매우 길어질 것 같은데, 계속 작성할까요?
아니면 특정 섹션을 먼저 집중적으로 설명해드릴까요?

### 4.16 _executeCompoundV2Liquidation 함수 (Line 310-353)

Compound v2 프로토콜에서 청산을 실행하는 함수입니다.

```solidity
310:    function _executeCompoundV2Liquidation(
311:        LiquidationParams memory params
312:    ) internal returns (uint256 collateralReceived) {
313:        address cTokenBorrowed = params.debtAsset;
314:        address cTokenCollateral = params.collateralAsset;
315:
316:        // Approve repayment to cToken
317:        address underlying = ICToken(cTokenBorrowed).underlying();
318:        IERC20(underlying).safeApprove(cTokenBorrowed, params.debtToCover);
319:
320:        // Execute liquidation
321:        uint256 result = ICToken(cTokenBorrowed).liquidateBorrow(
322:            params.user,
323:            params.debtToCover,
324:            cTokenCollateral
325:        );
326:        require(result == 0, "Compound liquidation failed");
327:
328:        // Redeem cTokens for underlying
329:        uint256 cTokenBalance = ICToken(cTokenCollateral).balanceOf(address(this));
330:        require(cTokenBalance > 0, "No cTokens received");
331:
332:        uint256 underlyingBefore = IERC20(ICToken(cTokenCollateral).underlying()).balanceOf(address(this));
333:
334:        uint256 redeemResult = ICToken(cTokenCollateral).redeem(cTokenBalance);
335:        require(redeemResult == 0, "Compound redeem failed");
336:
337:        collateralReceived = IERC20(ICToken(cTokenCollateral).underlying()).balanceOf(address(this)) - underlyingBefore;
338:
339:        emit CompoundV2Liquidated(
340:            params.user,
341:            cTokenBorrowed,
342:            cTokenCollateral,
343:            params.debtToCover,
344:            cTokenBalance
345:        );
346:
347:        emit CollateralRedeemed(
348:            cTokenCollateral,
349:            cTokenBalance,
350:            ICToken(cTokenCollateral).underlying(),
351:            collateralReceived
352:        );
353:    }
```

**Compound v2 vs Aave 차이점:**

| 특징 | Aave v3 | Compound v2 |
|------|---------|-------------|
| 청산 함수 | `liquidationCall()` | `liquidateBorrow()` |
| 파라미터 | Underlying 주소 | cToken 주소 |
| 담보 수령 | Underlying 직접 | cToken → Redeem 필요 |
| 에러 처리 | revert | 에러 코드 반환 |
| 청산 보너스 | 5% (설정) | 8% (설정) |

**라인별 분석:**

**Line 313-314: cToken 주소 추출**

```solidity
313: address cTokenBorrowed = params.debtAsset;
314: address cTokenCollateral = params.collateralAsset;
```

**Compound v2의 특징:**
- 모든 자산이 cToken으로 래핑됨
- cUSDC, cETH, cDAI 등
- params에 cToken 주소 전달

**예시:**
```solidity
cTokenBorrowed = 0x39AA39c021dfbaE8faC545936693aC917d5E7563  // cUSDC
cTokenCollateral = 0x4Ddc2D193948926D02f9B1fE9e1daa0718270ED5 // cETH
```

**Line 317-318: Underlying 승인**

```solidity
317: address underlying = ICToken(cTokenBorrowed).underlying();
318: IERC20(underlying).safeApprove(cTokenBorrowed, params.debtToCover);
```

**Underlying 조회:**
```solidity
ICToken(cUSDC).underlying()
// 반환: 0xA0b86991c... (USDC 주소)
```

**왜 underlying을 approve 하나?**
```
Compound liquidateBorrow() 내부:
1. cToken이 underlying을 pull (transferFrom)
2. underlying으로 빚 상환
3. cToken(담보)을 liquidator에게 전송

→ 1번을 위해 underlying approve 필요
```

**Approve 과정:**
```solidity
IERC20(USDC).safeApprove(cUSDC, 500e6);
// "cUSDC야, 내 USDC 500개 가져가도 돼"
```

**Line 321-326: 청산 실행**

```solidity
321: uint256 result = ICToken(cTokenBorrowed).liquidateBorrow(
322:     params.user,           // 청산 대상자
323:     params.debtToCover,    // 상환할 underlying 양
324:     cTokenCollateral       // 받을 cToken
325: );
326: require(result == 0, "Compound liquidation failed");
```

**liquidateBorrow 파라미터:**

```solidity
address borrower       // 청산 대상자
```
- Health Factor < 1.0인 사용자

```solidity
uint256 repayAmount    // 상환할 양 (underlying 기준!)
```
- USDC 기준 (cUSDC 아님)
- 최대 50%까지 청산 가능

```solidity
address cTokenCollateral // 받을 담보 cToken
```
- 받고 싶은 담보의 cToken 주소
- 예: cETH, cWBTC 등

**반환값: 에러 코드**

Compound는 revert 대신 에러 코드 반환:

```solidity
0: 성공
다른 값: 실패 (값에 따라 이유 다름)
```

**에러 코드 예시:**
```
0: NO_ERROR (성공)
1: UNAUTHORIZED
2: BAD_INPUT
3: COMPTROLLER_REJECTION
4: COMPTROLLER_CALCULATION_ERROR
5: INTEREST_RATE_MODEL_ERROR
6: INVALID_ACCOUNT_PAIR
7: INVALID_CLOSE_AMOUNT_REQUESTED
8: INVALID_COLLATERAL_FACTOR
9: MATH_ERROR
10: MARKET_NOT_FRESH
11: MARKET_NOT_LISTED
12: TOKEN_INSUFFICIENT_ALLOWANCE
13: TOKEN_INSUFFICIENT_BALANCE
14: TOKEN_INSUFFICIENT_CASH
15: TOKEN_TRANSFER_IN_FAILED
16: TOKEN_TRANSFER_OUT_FAILED
17: UTILIZATION_ABOVE_MAX
```

**검증:**
```solidity
require(result == 0, "Compound liquidation failed");
// result가 0이 아니면 revert
```

**liquidateBorrow 내부 동작:**

```
1. 우리 컨트랙트에서 500 USDC 차감 (transferFrom)
2. 청산 대상자의 500 USDC 빚 상환
3. 청산 보너스 계산:
   - 상환액: 500 USDC = $500
   - 보너스: 8%
   - 담보 가치: $500 * 1.08 = $540
4. cToken 양 계산:
   - ETH 가격: $2000
   - 필요 ETH: $540 / $2000 = 0.27 ETH
   - cETH 환율: 0.02 (1 cETH = 0.02 ETH)
   - cETH 양: 0.27 / 0.02 = 13.5 cETH
5. 청산 대상자에서 13.5 cETH 압수
6. 우리 컨트랙트로 13.5 cETH 전송
```

**Line 329-330: cToken 수령 확인**

```solidity
329: uint256 cTokenBalance = ICToken(cTokenCollateral).balanceOf(address(this));
330: require(cTokenBalance > 0, "No cTokens received");
```

**cToken 잔고 조회:**
```solidity
ICToken(cETH).balanceOf(address(this))
// 반환: 13500000 (13.5 cETH, 8 decimals)
```

**검증:**
```solidity
require(cTokenBalance > 0, "No cTokens received");
// 담보를 못 받으면 revert
```

**Line 332-337: cToken을 Underlying으로 교환**

```solidity
332: uint256 underlyingBefore = IERC20(ICToken(cTokenCollateral).underlying()).balanceOf(address(this));
333:
334: uint256 redeemResult = ICToken(cTokenCollateral).redeem(cTokenBalance);
335: require(redeemResult == 0, "Compound redeem failed");
336:
337: collateralReceived = IERC20(ICToken(cTokenCollateral).underlying()).balanceOf(address(this)) - underlyingBefore;
```

**Redeem 전 잔고:**
```solidity
underlyingBefore = IERC20(WETH).balanceOf(address(this));
// 예: 1.0 ETH
```

**Redeem 실행:**
```solidity
ICToken(cETH).redeem(13.5e8);
// 13.5 cETH → ? ETH
```

**Redeem 계산:**
```
cToken 환율: 0.02 (1 cETH = 0.02 ETH)

redeem(13.5 cETH)
= 13.5 * 0.02
= 0.27 ETH
```

**에러 코드 검증:**
```solidity
require(redeemResult == 0, "Compound redeem failed");
// 0이 아니면 실패
```

**Underlying 수령량 계산:**
```solidity
collateralReceived = 현재 잔고 - 이전 잔고
collateralReceived = 1.27 ETH - 1.0 ETH = 0.27 ETH
```

**Line 339-352: 이벤트 발생**

```solidity
339: emit CompoundV2Liquidated(
340:     params.user,           // 청산 대상자
341:     cTokenBorrowed,        // 빚 cToken (cUSDC)
342:     cTokenCollateral,      // 담보 cToken (cETH)
343:     params.debtToCover,    // 상환한 underlying (500 USDC)
344:     cTokenBalance          // 받은 cToken (13.5 cETH)
345: );
346:
347: emit CollateralRedeemed(
348:     cTokenCollateral,      // cETH
349:     cTokenBalance,         // 13.5 cETH
350:     ICToken(cTokenCollateral).underlying(),  // WETH
351:     collateralReceived     // 0.27 ETH
352: );
```

**두 개의 이벤트를 발생시키는 이유:**

1. **CompoundV2Liquidated**: 청산 자체의 정보
   - 얼마나 상환했는지
   - 얼마나 받았는지 (cToken 기준)

2. **CollateralRedeemed**: Redeem 정보
   - cToken을 underlying으로 교환
   - 최종적으로 받은 underlying 양

**전체 Compound v2 청산 흐름:**

```
1. Underlying 조회 (cUSDC → USDC)
2. USDC approve (cUSDC에게)
3. liquidateBorrow() 호출
   - 500 USDC 상환
   - 13.5 cETH 수령
4. cETH 잔고 확인
5. Redeem 전 ETH 잔고 기록
6. redeem() 호출
   - 13.5 cETH → 0.27 ETH
7. ETH 수령량 계산
8. 이벤트 발생 (2개)
9. 0.27 ETH 반환
```

### 4.17 _executeCompoundV3Liquidation 함수 (Line 358-373)

Compound v3 (Comet) 프로토콜에서 청산을 실행하는 함수입니다.

```solidity
358:    function _executeCompoundV3Liquidation(
359:        LiquidationParams memory params
360:    ) internal returns (uint256 collateralReceived) {
361:        address[] memory accounts = new address[](1);
362:        accounts[0] = params.user;
363:
364:        uint256 collateralBefore = IERC20(params.collateralAsset).balanceOf(address(this));
365:
366:        // Absorb underwater account
367:        IComet(params.protocol).absorb(address(this), accounts);
368:
369:        collateralReceived = IERC20(params.collateralAsset).balanceOf(address(this)) - collateralBefore;
370:        require(collateralReceived > 0, "No collateral absorbed");
371:
372:        emit CompoundV3Absorbed(params.user, params.protocol, collateralReceived);
373:    }
```

**Compound v3 vs v2 차이점:**

| 특징 | Compound v2 | Compound v3 (Comet) |
|------|-------------|---------------------|
| 청산 함수 | `liquidateBorrow()` | `absorb()` |
| cToken | 있음 (각 자산별) | 없음 (Comet 하나) |
| 담보 수령 | cToken → Redeem | Underlying 직접 |
| 배치 청산 | 불가 | 가능 (여러 계정) |
| 복잡도 | 높음 | 낮음 |
| 가스 비용 | 높음 | 낮음 |

**라인별 분석:**

**Line 361-362: 청산 대상 배열 생성**

```solidity
361: address[] memory accounts = new address[](1);
362: accounts[0] = params.user;
```

**왜 배열을 사용하나?**

Compound v3의 `absorb()` 함수는 여러 계정을 동시에 청산 가능:

```solidity
// 한 명만 청산
address[] memory accounts = new address[](1);
accounts[0] = 0x742d35Cc...;

// 여러 명 청산
address[] memory accounts = new address[](3);
accounts[0] = 0x742d35Cc...;
accounts[1] = 0x8a3b21Ff...;
accounts[2] = 0x5c9e44Ba...;
```

**배열 생성 문법:**
```solidity
new address[](1)  // 크기 1인 address 배열 생성
```

**메모리 배열:**
```solidity
address[] memory accounts  // 임시 배열 (함수 종료시 삭제)
```

**Line 364: 청산 전 잔고 기록**

```solidity
364: uint256 collateralBefore = IERC20(params.collateralAsset).balanceOf(address(this));
```

**Aave와 동일한 패턴:**
```
담보 수령량 = 현재 잔고 - 이전 잔고
```

**Compound v3는 Underlying 직접 수령:**
```solidity
// v2: cETH 수령 → redeem → ETH
// v3: ETH 직접 수령 (간단!)
```

**Line 367: Absorb 실행**

```solidity
367: IComet(params.protocol).absorb(address(this), accounts);
```

**absorb 함수 파라미터:**

```solidity
address absorber         // 청산 실행자 (보상 받을 주소)
address[] accounts       // 청산 대상 배열
```

**absorber 파라미터:**
```solidity
address(this)  // 우리 컨트랙트
// 담보와 보상을 이 주소로 전송
```

**absorb vs liquidateBorrow:**

| 기능 | liquidateBorrow (v2) | absorb (v3) |
|------|---------------------|-------------|
| 상환 지정 | 직접 지정 | 자동 계산 |
| 담보 선택 | 직접 선택 | 자동 선택 |
| 보너스 | 8% | 프로토콜 결정 |
| 복잡도 | 높음 | 낮음 |

**absorb 내부 동작:**

```
1. accounts의 각 계정 검사
2. Health Factor < 1.0인 계정만 처리
3. 빚 전액 자동 계산
4. 담보 전액 자동 계산
5. 청산 보너스 적용
6. 담보를 absorber에게 전송
7. 프로토콜이 빚 처리 (내부적으로)
```

**예시:**
```
사용자 계정:
- 담보: 1.0 ETH ($2000)
- 빚: 1900 USDC
- HF: 0.95 (청산 가능)

absorb 호출:
1. 1900 USDC 빚 확인
2. 청산 보너스 8% 적용
3. 필요 담보: $1900 * 1.08 = $2052
4. ETH 가격: $2000
5. 담보 양: $2052 / $2000 = 1.026 ETH
6. 우리에게 1.026 ETH 전송
7. 사용자 계정 정리
```

**Line 369-370: 담보 수령 확인**

```solidity
369: collateralReceived = IERC20(params.collateralAsset).balanceOf(address(this)) - collateralBefore;
370: require(collateralReceived > 0, "No collateral absorbed");
```

**계산:**
```solidity
collateralBefore = 1.0 ETH
collateralAfter = 2.026 ETH
collateralReceived = 1.026 ETH
```

**검증:**
```solidity
require(1.026 > 0, "No collateral absorbed");
// ✅ 통과
```

**실패 케이스:**
```
1. 계정이 이미 청산됨
2. HF가 회복됨
3. 담보 부족
4. 프로토콜 오류

→ collateralReceived = 0
→ revert
```

**Line 372: 이벤트 발생**

```solidity
372: emit CompoundV3Absorbed(params.user, params.protocol, collateralReceived);
```

**이벤트 데이터:**
```javascript
{
    user: "0x742d35Cc...",
    comet: "0xc3d688B6..." (Comet USDC),
    assetsAbsorbed: "1026000000000000000" // 1.026 ETH
}
```

**Compound v3의 장점:**

1. **간단한 코드**
   - v2: 10줄 이상
   - v3: 5줄

2. **낮은 가스 비용**
   - redeem 불필요
   - 한 번의 호출로 완료

3. **배치 청산**
   - 여러 계정 동시 처리
   - 가스 절약

4. **자동 계산**
   - 상환액 자동
   - 담보 자동 선택

**Compound v3 청산 흐름:**

```
1. 청산 대상 배열 생성
2. 청산 전 잔고 기록
3. absorb() 호출
   - 빚 자동 상환
   - 담보 자동 압수 및 전송
4. 담보 수령량 계산
5. 검증
6. 이벤트 발생
7. 담보 반환
```

### 4.18 _executeSwap 함수 (Line 378-414)

DEX를 통해 담보를 빚 상환 토큰으로 교환하는 함수입니다.

```solidity
378:    function _executeSwap(
379:        address router,
380:        bytes memory swapData,
381:        address tokenIn,
382:        address tokenOut,
383:        uint256 amountIn,
384:        uint256 minAmountOut
385:    ) internal returns (uint256 amountOut) {
386:        require(router != address(0), "Invalid router");
387:        require(_isContract(router), "Router is not a contract");
388:        require(amountIn > 0, "No tokens to swap");
389:
390:        IERC20 tokenInContract = IERC20(tokenIn);
391:        IERC20 tokenOutContract = IERC20(tokenOut);
392:
393:        // Reset and approve tokens for swap
394:        tokenInContract.safeApprove(router, 0);
395:        tokenInContract.safeApprove(router, amountIn);
396:
397:        uint256 balanceBefore = tokenOutContract.balanceOf(address(this));
398:
399:        // Execute swap
400:        (bool success, bytes memory returnData) = router.call(swapData);
401:        require(success, "Swap execution failed");
402:
403:        amountOut = tokenOutContract.balanceOf(address(this)) - balanceBefore;
404:        require(amountOut >= minAmountOut, "Insufficient swap output");
405:
406:        emit CollateralSwapped(
407:            router,
408:            tokenIn,
409:            tokenOut,
410:            amountIn,
411:            amountOut,
412:            minAmountOut
413:        );
414:    }
```

**라인별 분석:**

**Line 378-385: 함수 시그니처**

```solidity
function _executeSwap(
    address router,          // DEX 라우터 주소
    bytes memory swapData,   // 스왑 calldata
    address tokenIn,         // 입력 토큰 (담보)
    address tokenOut,        // 출력 토큰 (빚 상환용)
    uint256 amountIn,        // 스왑할 양
    uint256 minAmountOut     // 최소 수령량
) internal returns (uint256 amountOut)
```

**파라미터 설명:**

```solidity
address router
```
- DEX 라우터/어그리게이터 주소
- 예: Uniswap V3, 1inch, 0x

```solidity
bytes memory swapData
```
- 인코딩된 스왑 함수 호출 데이터
- 오프체인에서 생성
- 함수 셀렉터 + 파라미터 포함

```solidity
address tokenIn
```
- 청산으로 받은 담보 토큰
- 예: WETH, WBTC

```solidity
address tokenOut
```
- 플래시론 상환용 토큰
- 예: USDC, DAI

```solidity
uint256 amountIn
```
- 스왑할 담보의 양
- 청산에서 받은 전체 담보

```solidity
uint256 minAmountOut
```
- 최소 수령량 (슬리피지 보호)
- 이보다 적게 받으면 revert

**Line 386-388: 입력 검증**

```solidity
386: require(router != address(0), "Invalid router");
387: require(_isContract(router), "Router is not a contract");
388: require(amountIn > 0, "No tokens to swap");
```

**Line 386: 라우터 주소 검증**
```solidity
require(router != address(0), "Invalid router");
```
- null 주소 방지
- 잘못된 설정 조기 감지

**Line 387: 컨트랙트 여부 확인**
```solidity
require(_isContract(router), "Router is not a contract");
```

**`_isContract()` 함수:**
```solidity
function _isContract(address account) internal view returns (bool) {
    uint256 size;
    assembly { size := extcodesize(account) }
    return size > 0;
}
```

**작동 원리:**
```
EOA (일반 지갑): code size = 0
Contract: code size > 0

_isContract(0x1234...) // EOA
→ extcodesize = 0
→ return false

_isContract(UniswapRouter)
→ extcodesize = 15234 bytes
→ return true
```

**Line 388: 스왑 양 검증**
```solidity
require(amountIn > 0, "No tokens to swap");
```
- 0 스왑 방지
- 가스 낭비 방지

**Line 390-391: 토큰 인터페이스 생성**

```solidity
390: IERC20 tokenInContract = IERC20(tokenIn);
391: IERC20 tokenOutContract = IERC20(tokenOut);
```

**타입 캐스팅:**
```solidity
IERC20(tokenIn)  // address → IERC20 interface
```

**사용 이유:**
```solidity
// 캐스팅 전
tokenIn.balanceOf(address(this));  // ❌ 컴파일 에러

// 캐스팅 후
IERC20(tokenIn).balanceOf(address(this));  // ✅ OK
```

**Line 394-395: Approve (2단계)**

```solidity
394: tokenInContract.safeApprove(router, 0);
395: tokenInContract.safeApprove(router, amountIn);
```

**왜 2번 approve 하나?**

일부 토큰(특히 USDT)은 보안상 이유로 approve를 0이 아닌 값에서 다른 값으로 직접 변경 불가:

```solidity
// USDT의 경우
token.approve(router, 100);  // ✅ OK (0 → 100)
token.approve(router, 200);  // ❌ 에러! (100 → 200 불가)

// 올바른 방법
token.approve(router, 0);    // 1. 먼저 0으로 리셋
token.approve(router, 200);  // 2. 새 값 설정
```

**SafeApprove의 장점:**
- USDT 같은 특수 토큰 자동 처리
- 0으로 리셋 후 새 값 설정
- 모든 ERC-20 호환

**Line 397: 스왑 전 잔고 기록**

```solidity
397: uint256 balanceBefore = tokenOutContract.balanceOf(address(this));
```

**잔고 기록 이유:**
```
스왑 결과 계산:
amountOut = 현재 잔고 - 이전 잔고

예:
스왑 전 USDC: 500
스왑 후 USDC: 1500
받은 양: 1500 - 500 = 1000 USDC
```

**Line 400-401: Low-Level Call로 스왑 실행**

```solidity
400: (bool success, bytes memory returnData) = router.call(swapData);
401: require(success, "Swap execution failed");
```

**Low-Level Call이란?**

```solidity
// High-level call (인터페이스 필요)
IUniswapRouter(router).swapExactTokensForTokens(...);

// Low-level call (인터페이스 불필요)
router.call(swapData);
```

**장점:**
1. **유연성**: 모든 DEX 지원
2. **간단함**: 인터페이스 정의 불필요
3. **확장성**: 새로운 DEX 추가 쉬움

**call 반환값:**
```solidity
(bool success, bytes memory returnData)
```
- `success`: 호출 성공 여부
- `returnData`: 반환 데이터 (사용 안 함)

**swapData 구조:**

```
[4 bytes] 함수 셀렉터
[32 bytes] 파라미터 1
[32 bytes] 파라미터 2
...
```

**예시 (Uniswap V3):**
```javascript
// JavaScript/TypeScript
const swapData = router.interface.encodeFunctionData(
    'exactInputSingle',
    [{
        tokenIn: WETH,
        tokenOut: USDC,
        fee: 3000,
        recipient: liquidationContract.address,
        deadline: deadline,
        amountIn: amountIn,
        amountOutMinimum: minOut,
        sqrtPriceLimitX96: 0
    }]
);

// 결과 (bytes)
0x414bf389  // exactInputSingle selector
0x00000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2  // tokenIn
0x000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48  // tokenOut
// ...
```

**Line 403-404: 수령량 검증**

```solidity
403: amountOut = tokenOutContract.balanceOf(address(this)) - balanceBefore;
404: require(amountOut >= minAmountOut, "Insufficient swap output");
```

**계산:**
```solidity
balanceBefore = 500e6        // 스왑 전 500 USDC
balanceAfter = 1500e6        // 스왑 후 1500 USDC
amountOut = 1000e6           // 받은 1000 USDC
```

**슬리피지 검증:**
```solidity
minAmountOut = 990e6         // 최소 990 USDC (1% 슬리피지)
amountOut = 1000e6

1000 >= 990  // ✅ 통과
```

**실패 케이스:**
```
예상: 1000 USDC
최소: 990 USDC (1% 슬리피지)
실제: 950 USDC (5% 슬리피지 - 너무 큼!)

950 >= 990  // ❌ 실패
→ revert "Insufficient swap output"
```

**슬리피지가 왜 중요한가?**

```
시나리오 1: 슬리피지 보호 없음
1. 0.5 ETH 스왑 예상: 1000 USDC
2. 실제 수령: 500 USDC (50% 손실!)
3. 플래시론 상환 필요: 1000.9 USDC
4. 부족! (500 < 1000.9)
5. 하지만 이미 스왑 완료...
6. 우리 자금으로 메꿔야 함 💸

시나리오 2: 슬리피지 보호 있음
1. 0.5 ETH 스왑 예상: 1000 USDC
2. 최소 설정: 990 USDC
3. 실제 수령: 500 USDC
4. 검증: 500 < 990 ❌
5. 즉시 revert!
6. 모든 것 롤백 ✅
```

**Line 406-413: 이벤트 발생**

```solidity
406: emit CollateralSwapped(
407:     router,          // Uniswap V3 Router
408:     tokenIn,         // WETH
409:     tokenOut,        // USDC
410:     amountIn,        // 0.5 ETH
411:     amountOut,       // 1000 USDC
412:     minAmountOut     // 990 USDC
413: );
```

**이벤트 데이터:**
```javascript
{
    router: "0xE592427A..." (Uniswap V3),
    tokenIn: "0xC02aaA39..." (WETH),
    tokenOut: "0xA0b86991c..." (USDC),
    amountIn: "500000000000000000", // 0.5 ETH
    amountOut: "1000000000",        // 1000 USDC
    minAmountOut: "990000000"       // 990 USDC
}
```

**프론트엔드 활용:**
```javascript
contract.on('CollateralSwapped', (router, tokenIn, tokenOut, amountIn, amountOut, minOut) => {
    const ethIn = ethers.formatEther(amountIn);
    const usdcOut = ethers.formatUnits(amountOut, 6);
    const price = parseFloat(usdcOut) / parseFloat(ethIn);

    console.log(`스왑 완료!`);
    console.log(`${ethIn} ETH → ${usdcOut} USDC`);
    console.log(`평균 가격: $${price.toFixed(2)}/ETH`);

    // 슬리피지 계산
    const minUSDC = parseFloat(ethers.formatUnits(minOut, 6));
    const slippage = ((parseFloat(usdcOut) - minUSDC) / minUSDC * 100).toFixed(2);
    console.log(`슬리피지: ${slippage}%`);
});
```

**전체 스왑 흐름:**

```
1. 입력 검증 (라우터, 양)
2. 토큰 인터페이스 생성
3. Approve (0 → amountIn)
4. 스왑 전 잔고 기록
5. Low-level call로 스왑 실행
6. 스왑 성공 여부 확인
7. 수령량 계산
8. 슬리피지 검증
9. 이벤트 발생
10. 수령량 반환
```

---

## 5. 플래시론(Flash Loan) 완전 가이드

### 5.1 플래시론이란?

**정의:**
플래시론은 담보 없이 대량의 자금을 빌렸다가 같은 트랜잭션 내에서 갚는 금융 기법입니다.

**핵심 특징:**
1. **무담보**: 담보 없이 빌림
2. **원자성**: 한 트랜잭션 내에서 빌리고 갚아야 함
3. **무제한**: 프로토콜 유동성 범위 내에서 무제한
4. **저렴함**: 수수료 0.09% (Aave v3)

**일반 대출 vs 플래시론:**

| 특징 | 일반 대출 | 플래시론 |
|------|----------|---------|
| 담보 | 필요 | 불필요 |
| 기간 | 일/월/년 | 1 트랜잭션 |
| 금액 | 담보 기반 제한 | 유동성 기반 무제한 |
| 수수료 | 이자 (APR) | 고정 (0.09%) |
| 위험 | 청산 | 없음 (자동 revert) |

### 5.2 플래시론 작동 원리

**트랜잭션 내 실행 순서:**

```
┌─────────────────────────────────────┐
│  Transaction 시작                    │
├─────────────────────────────────────┤
│  1. flashLoanSimple() 호출          │
│     └─ Pool이 자금 전송              │
├─────────────────────────────────────┤
│  2. executeOperation() 콜백         │
│     ├─ 빌린 돈으로 청산 실행         │
│     ├─ 담보 획득                     │
│     ├─ 담보 스왑                     │
│     └─ approve(원금+수수료)          │
├─────────────────────────────────────┤
│  3. Pool이 자금 회수                 │
│     └─ transferFrom()                │
├─────────────────────────────────────┤
│  4. 성공 시 커밋, 실패 시 revert     │
└─────────────────────────────────────┘
```

**성공 케이스:**
```
1. 1000 USDC 플래시론
2. 500 USDC로 청산
3. 0.5 ETH 담보 획득
4. 0.5 ETH → 1050 USDC 스왑
5. 1000.9 USDC 상환
6. 남은 49.1 USDC = 수익 ✅
7. 트랜잭션 커밋
```

**실패 케이스:**
```
1. 1000 USDC 플래시론
2. 500 USDC로 청산
3. 0.5 ETH 담보 획득
4. 0.5 ETH → 900 USDC 스왑 (가격 하락!)
5. 900 + 500 = 1400 USDC (상환 필요: 1000.9)
6. 실제 상환 가능: 1400 - 500 = 900 USDC
7. 부족! (900 < 1000.9) ❌
8. transferFrom() 실패
9. 전체 트랜잭션 revert
10. 모든 상태 롤백 (청산도 취소됨)
```

### 5.3 플래시론 수수료 계산

**Aave v3 수수료:**
- 기본: 0.09% (9 basis points)
- 변경 가능 (거버넌스)

**계산 공식:**
```solidity
premium = amount * 9 / 10000
totalRepay = amount + premium
```

**예시:**

```
1000 USDC 빌림:
premium = 1000 * 9 / 10000 = 0.9 USDC
totalRepay = 1000 + 0.9 = 1000.9 USDC

10000 USDC 빌림:
premium = 10000 * 9 / 10000 = 9 USDC
totalRepay = 10000 + 9 = 10009 USDC

1 ETH 빌림:
premium = 1e18 * 9 / 10000 = 0.0009e18 = 0.0009 ETH
totalRepay = 1.0009 ETH
```

**수수료 비교:**

| 프로토콜 | 플래시론 수수료 |
|---------|---------------|
| Aave v3 | 0.09% |
| Aave v2 | 0.09% |
| dYdX | 0% (무료!) |
| Balancer | 0% (무료!) |
| Uniswap V3 | 없음 (지원 안 함) |

### 5.4 플래시론 보안

**보안 체크리스트:**

1. **호출자 검증**
```solidity
require(msg.sender == address(POOL), "Invalid caller");
```

2. **초기자 검증**
```solidity
require(initiator == address(this), "Invalid initiator");
```

3. **잔고 확인**
```solidity
require(IERC20(asset).balanceOf(address(this)) >= amountOwed);
```

4. **Approve 검증**
```solidity
IERC20(asset).safeApprove(address(POOL), amountOwed);
```

**공격 시나리오와 방어:**

**시나리오 1: 가짜 Pool**
```
공격:
1. 악성 컨트랙트가 가짜 Pool 배포
2. 가짜 Pool이 우리 executeOperation() 호출
3. 우리 자금 훔치기

방어:
require(msg.sender == address(POOL))
→ 진짜 Pool만 호출 가능
```

**시나리오 2: 다른 사람이 우리 컨트랙트 이용**
```
공격:
1. 악성 사용자가 Aave Pool에 플래시론 요청
2. receiver를 우리 컨트랙트로 설정
3. 우리가 상환 책임

방어:
require(initiator == address(this))
→ 우리가 요청한 것만 처리
```

**시나리오 3: 재진입 공격**
```
공격:
1. executeOperation() 실행 중
2. 악성 토큰의 transfer()에서
3. 다시 우리 함수 호출
4. 중복 실행

방어:
nonReentrant modifier
→ 실행 중 재호출 차단
```

---

## 6. 프로토콜별 청산 메커니즘

### 6.1 Aave v3 청산 상세

**청산 조건:**
```
Health Factor = (담보 가치 * 청산 임계값) / 빚 가치

HF >= 1.0: 안전
HF < 1.0: 청산 가능
```

**예시:**
```
담보: 10 ETH @ $2000 = $20,000
빚: 15,000 USDC
청산 임계값: 80% (8000 basis points)

HF = (20,000 * 0.8) / 15,000
   = 16,000 / 15,000
   = 1.067

→ 안전! (HF > 1.0)

ETH 가격 하락 → $1800:
담보: 10 ETH @ $1800 = $18,000
HF = (18,000 * 0.8) / 15,000
   = 14,400 / 15,000
   = 0.96

→ 청산 가능! (HF < 1.0)
```

**청산 가능 금액:**
- 최대 50%까지 청산 가능
- Close Factor = 0.5 (50%)

**청산 보너스:**
- 기본: 5%
- 자산별로 다름
- 거버넌스로 조정 가능

**청산 프로세스:**

```
1. getUserAccountData() 호출
   → Health Factor 확인

2. 청산 금액 계산
   maxLiquidation = totalDebt * 0.5

3. liquidationCall() 호출
   - collateralAsset
   - debtAsset
   - user
   - debtToCover (최대 50%)
   - receiveAToken (보통 false)

4. 내부 처리:
   a. liquidator(우리)에게서 debt 차감
   b. user의 debt 상환
   c. user의 collateral 압수
   d. 청산 보너스 5% 추가
   e. liquidator에게 collateral 전송

5. 이벤트 발생:
   LiquidationCall(...)
```

**Aave 청산 계산 예시:**

```
사용자 포지션:
- 담보: 10 ETH @ $1800 = $18,000
- 빚: 15,000 USDC
- HF: 0.96 (청산 가능)

청산 실행:
- 상환: 7,500 USDC (50%)
- 필요 담보 가치: $7,500
- 청산 보너스: 5%
- 총 담보 가치: $7,500 * 1.05 = $7,875
- ETH 가격: $1,800
- ETH 양: $7,875 / $1,800 = 4.375 ETH

결과:
- 우리가 지불: 7,500 USDC
- 우리가 받음: 4.375 ETH
- 4.375 ETH @ $1,800 = $7,875
- 순이익: $7,875 - $7,500 = $375 (5%)
```

### 6.2 Compound v2 청산 상세

**청산 조건:**
```
Account Liquidity < 0
```

**Account Liquidity 계산:**
```
Liquidity = Σ(담보 * CF) - Σ(빚)

CF = Collateral Factor (담보 비율)
```

**예시:**
```
담보:
- 10 ETH @ $1,800, CF=75% = $13,500
- 5 WBTC @ $30,000, CF=70% = $105,000
총 담보 가치: $118,500

빚:
- 80,000 USDC = $80,000
- 0.5 WBTC @ $30,000 = $15,000
총 빚: $95,000

Liquidity = $118,500 - $95,000 = $23,500
→ 안전! (Liquidity > 0)

ETH 가격 하락 → $1,500:
담보 가치: 10 * 1,500 * 0.75 + 5 * 30,000 * 0.7
           = $11,250 + $105,000
           = $116,250

Liquidity = $116,250 - $95,000 = $21,250
→ 여전히 안전

WBTC 가격 하락 → $25,000:
담보 가치: 10 * 1,500 * 0.75 + 5 * 25,000 * 0.7
           = $11,250 + $87,500
           = $98,750

빚: 80,000 + 0.5 * 25,000 = $92,500

Liquidity = $98,750 - $92,500 = $6,250
→ 여전히 안전

둘 다 하락:
ETH: $1,200, WBTC: $22,000
담보: 10 * 1,200 * 0.75 + 5 * 22,000 * 0.7
    = $9,000 + $77,000
    = $86,000

빚: 80,000 + 0.5 * 22,000 = $91,000

Liquidity = $86,000 - $91,000 = -$5,000
→ 청산 가능! (Liquidity < 0)
```

**청산 프로세스:**

```
1. getAccountLiquidity() 호출
   → Liquidity < 0 확인

2. 청산 금액 계산
   maxClose = borrowBalance * closeFactor
   closeFactor = 0.5 (50%)

3. liquidateBorrow() 호출
   - borrower: 청산 대상
   - repayAmount: 상환할 underlying 양
   - cTokenCollateral: 받을 담보 cToken

4. 내부 처리:
   a. liquidator에게서 repayAmount(underlying) 차감
   b. borrower의 빚 상환
   c. seizeTokens 계산 (8% 보너스 포함)
   d. borrower의 cToken 압수
   e. liquidator에게 cToken 전송

5. cToken을 underlying으로 교환:
   redeem(cTokenBalance)
```

**cToken 환율:**

```
exchangeRate = (totalCash + totalBorrows - totalReserves) / totalSupply

예시:
cETH 환율: 0.02 (1 cETH = 0.02 ETH)

100 cETH → 100 * 0.02 = 2 ETH
2 ETH → 2 / 0.02 = 100 cETH
```

**Compound v2 청산 계산 예시:**

```
사용자 포지션:
- 담보: 100 cETH (= 2 ETH @ $1,500 = $3,000)
- 빚: 2,000 cUSDC (= 1,000 USDC)
- Liquidity: -$100 (청산 가능)

청산 실행:
1. 상환: 500 USDC (50%)

2. seizeTokens 계산:
   - 상환액: $500
   - 청산 보너스: 8%
   - 필요 담보: $500 * 1.08 = $540
   - ETH 가격: $1,500
   - ETH 양: $540 / $1,500 = 0.36 ETH

3. cToken 계산:
   - 환율: 0.02 (1 cETH = 0.02 ETH)
   - cETH 양: 0.36 / 0.02 = 18 cETH

4. Redeem:
   - redeem(18 cETH)
   - 받음: 0.36 ETH

결과:
- 지불: 500 USDC
- 받음: 0.36 ETH @ $1,500 = $540
- 이익: $40 (8%)
```

### 6.3 Compound v3 (Comet) 청산 상세

**청산 조건:**
```
Borrow Capacity < 0
```

**Borrow Capacity 계산:**
```
Capacity = Σ(collateral * price * borrowCF) - borrowBalance
```

**Absorb 메커니즘:**

Compound v3는 v2와 완전히 다른 청산 방식 사용:

```
v2: liquidateBorrow()
    - 청산자가 빚 상환
    - 청산자가 담보 받음

v3: absorb()
    - 프로토콜이 빚 흡수
    - 청산자가 담보 받음
    - 프로토콜이 손실 부담
```

**Absorb 프로세스:**

```
1. absorb() 호출:
   absorb(absorber, [user1, user2, ...])

2. 각 사용자에 대해:
   a. 청산 가능 여부 확인
   b. 담보 전액 계산
   c. 청산 보너스 적용
   d. absorber에게 담보 전송
   e. 프로토콜이 빚 흡수

3. 프로토콜의 reserves로 손실 커버
```

**장점:**

1. **간단함**: 한 번의 호출
2. **가스 절약**: redeem 불필요
3. **배치 처리**: 여러 계정 동시 청산
4. **자동화**: 빚 자동 처리

**Compound v3 청산 예시:**

```
사용자 포지션:
- 담보: 1 WETH @ $1,500 = $1,500
- 빚: 1,400 USDC
- HF: 0.95 (청산 가능)

absorb() 호출:
1. 청산 보너스 계산: 8%
2. 필요 담보: $1,400 * 1.08 = $1,512
3. ETH 가격: $1,500
4. ETH 양: $1,512 / $1,500 = 1.008 ETH

하지만 사용자는 1 ETH만 있음!
→ 전액 압수: 1 ETH

5. absorber 받음: 1 ETH @ $1,500 = $1,500
6. 프로토콜 손실: $1,512 - $1,500 = $12
   → reserves에서 커버

결과:
- 청산자 이익: $1,500 - $1,400 = $100
- 프로토콜 손실: $12
```

---

## 7. DEX 스왑 로직 상세 분석

### 7.1 지원 DEX 목록

**주요 DEX:**

| DEX | 타입 | 장점 | 단점 |
|-----|------|------|------|
| Uniswap V3 | AMM | 높은 유동성, 낮은 수수료 | 복잡한 calldata |
| Uniswap V2 | AMM | 간단, 안정적 | 높은 수수료 |
| 1inch | 어그리게이터 | 최적 가격 | API 의존 |
| 0x | 어그리게이터 | 빠른 실행 | 복잡한 설정 |
| Paraswap | 어그리게이터 | 다양한 경로 | 가스 비용 |
| Curve | AMM | 스테이블 코인 최적 | 제한적 자산 |
| Balancer | AMM | 멀티 자산 풀 | 낮은 유동성 |

### 7.2 Uniswap V3 통합

**exactInputSingle 사용:**

```javascript
// JavaScript/TypeScript
const {ethers} = require('ethers');

// Uniswap V3 Router 인터페이스
const router = new ethers.Contract(
    UNISWAP_V3_ROUTER,
    ['function exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)) external returns (uint256)'],
    signer
);

// 스왑 파라미터
const params = {
    tokenIn: WETH_ADDRESS,
    tokenOut: USDC_ADDRESS,
    fee: 3000,  // 0.3%
    recipient: liquidationContract.address,
    deadline: Math.floor(Date.now() / 1000) + 300,  // 5분
    amountIn: ethers.parseEther("0.5"),  // 0.5 ETH
    amountOutMinimum: ethers.parseUnits("990", 6),  // 990 USDC
    sqrtPriceLimitX96: 0  // 가격 제한 없음
};

// Calldata 생성
const swapCalldata = router.interface.encodeFunctionData(
    'exactInputSingle',
    [params]
);

// 청산 실행
await liquidationStrategy.executeLiquidation(
    USDC_ADDRESS,
    ethers.parseUnits("1000", 6),
    {
        // ... other params
        dexRouter: UNISWAP_V3_ROUTER,
        swapCalldata: swapCalldata,
        minCollateralOut: ethers.parseUnits("990", 6)
    }
);
```

### 7.3 1inch 어그리게이터 통합

**1inch API 사용:**

```javascript
const axios = require('axios');

// 1inch API로 최적 스왑 경로 조회
async function get1inchSwapData(
    fromToken,
    toToken,
    amount,
    slippage
) {
    const url = `https://api.1inch.io/v5.0/1/swap`;
    
    const params = {
        fromTokenAddress: fromToken,
        toTokenAddress: toToken,
        amount: amount.toString(),
        fromAddress: liquidationContract.address,
        slippage: slippage,  // 1 = 1%
        disableEstimate: true
    };

    const response = await axios.get(url, {params});
    
    return {
        router: response.data.tx.to,
        calldata: response.data.tx.data,
        expectedOutput: response.data.toTokenAmount
    };
}

// 사용 예시
const swap = await get1inchSwapData(
    WETH_ADDRESS,
    USDC_ADDRESS,
    ethers.parseEther("0.5"),
    1  // 1% 슬리피지
);

await liquidationStrategy.executeLiquidation(
    USDC_ADDRESS,
    ethers.parseUnits("1000", 6),
    {
        // ... other params
        dexRouter: swap.router,
        swapCalldata: swap.calldata,
        minCollateralOut: BigInt(swap.expectedOutput) * 99n / 100n
    }
);
```

### 7.4 슬리피지 관리

**슬리피지란?**

예상 가격과 실제 체결 가격의 차이:

```
예상 가격: 1 ETH = $2,000
실제 가격: 1 ETH = $1,950
슬리피지: ($2,000 - $1,950) / $2,000 = 2.5%
```

**적절한 슬리피지 설정:**

| 시장 상황 | 슬리피지 | 설명 |
|----------|---------|------|
| 안정적 | 0.5% | 스테이블코인, 높은 유동성 |
| 일반 | 1% | ETH/USDC 같은 메이저 페어 |
| 변동성 | 2-3% | 중간 유동성 토큰 |
| 고변동성 | 5%+ | 낮은 유동성, 급격한 변동 |

**슬리피지 계산:**

```javascript
function calculateMinOut(expectedAmount, slippagePercent) {
    return expectedAmount * (100 - slippagePercent) / 100;
}

// 예시
const expectedUSDC = 1000e6;  // 1000 USDC
const slippage = 1;  // 1%

const minUSDC = calculateMinOut(expectedUSDC, slippage);
// minUSDC = 1000 * 99 / 100 = 990 USDC
```

**동적 슬리피지:**

```javascript
function getDynamicSlippage(volatility, liquidity) {
    let slippage = 0.5;  // 기본 0.5%

    // 변동성 증가시 슬리피지 증가
    if (volatility > 5) slippage += 1;
    if (volatility > 10) slippage += 1;

    // 유동성 감소시 슬리피지 증가
    if (liquidity < 1000000) slippage += 0.5;
    if (liquidity < 100000) slippage += 1;

    return Math.min(slippage, 5);  // 최대 5%
}
```

---

## 8. 보안 및 에러 처리

### 8.1 보안 체크리스트

**레벨 1: 기본 보안**

✅ **Access Control**
```solidity
modifier onlyOwner() {
    require(msg.sender == owner(), "Not owner");
    _;
}
```

✅ **Reentrancy Guard**
```solidity
modifier nonReentrant() {
    require(_status != _ENTERED);
    _status = _ENTERED;
    _;
    _status = _NOT_ENTERED;
}
```

✅ **Input Validation**
```solidity
require(amount > 0, "Invalid amount");
require(user != address(0), "Invalid address");
require(router != address(0), "Invalid router");
```

**레벨 2: 고급 보안**

✅ **Flash Loan Validation**
```solidity
require(msg.sender == address(POOL), "Invalid caller");
require(initiator == address(this), "Invalid initiator");
```

✅ **Premium Validation**
```solidity
require(
    premium <= expectedPremium * 110 / 100 &&
    premium >= expectedPremium * 90 / 100,
    "Premium outside tolerance"
);
```

✅ **Slippage Protection**
```solidity
require(amountOut >= minAmountOut, "Insufficient output");
```

✅ **Contract Verification**
```solidity
function _isContract(address account) internal view returns (bool) {
    uint256 size;
    assembly { size := extcodesize(account) }
    return size > 0;
}
```

**레벨 3: 프로토콜별 보안**

✅ **Aave Security**
```solidity
// 담보 수령 확인
require(collateralReceived > 0, "No collateral");

// Health Factor 재확인 (선택사항)
(,,,,,uint256 hf) = IAavePool(protocol).getUserAccountData(user);
require(hf < 1e18, "Not liquidatable");
```

✅ **Compound V2 Security**
```solidity
// 에러 코드 확인
uint256 result = ICToken(cToken).liquidateBorrow(...);
require(result == 0, "Liquidation failed");

// cToken 수령 확인
require(cTokenBalance > 0, "No cTokens");

// Redeem 검증
uint256 redeemResult = ICToken(cToken).redeem(balance);
require(redeemResult == 0, "Redeem failed");
```

✅ **Compound V3 Security**
```solidity
// 담보 흡수 확인
require(collateralReceived > 0, "No collateral absorbed");
```

### 8.2 에러 핸들링 전략

**Custom Errors (가스 절약)**

```solidity
// ❌ 옛날 방식 (비쌈)
require(balance >= amount, "Insufficient balance");

// ✅ 새로운 방식 (저렴)
if (balance < amount) revert InsufficientBalance();
```

**Try/Catch (우아한 실패)**

```solidity
try this._executeLiquidationLogic(...) {
    // 성공
    return true;
} catch Error(string memory reason) {
    // require/revert with string
    emit LiquidationFailed(reason);
    revert FlashLoanCallbackFailed();
} catch Panic(uint errorCode) {
    // assert, overflow, etc.
    emit PanicError(errorCode);
    revert FlashLoanCallbackFailed();
} catch (bytes memory lowLevelData) {
    // 기타 모든 에러
    emit UnknownError(lowLevelData);
    revert FlashLoanCallbackFailed();
}
```

**에러 복구 전략**

```javascript
// JavaScript/TypeScript
async function executeLiquidationWithRetry(params, maxRetries = 3) {
    for (let i = 0; i < maxRetries; i++) {
        try {
            const tx = await liquidationStrategy.executeLiquidation(...params);
            const receipt = await tx.wait();
            return receipt;
        } catch (error) {
            console.error(`Attempt ${i + 1} failed:`, error.message);

            // 에러 유형별 처리
            if (error.message.includes('InsufficientProfit')) {
                console.log('Not profitable, aborting');
                throw error;  // 재시도 불필요
            }

            if (error.message.includes('FlashLoanCallbackFailed')) {
                console.log('Flash loan failed, retrying...');
                // 재시도
            }

            if (error.message.includes('Insufficient swap output')) {
                console.log('Slippage too high, increasing tolerance');
                params.minCollateralOut = params.minCollateralOut * 95n / 100n;
                // 슬리피지 완화하고 재시도
            }

            if (i === maxRetries - 1) {
                throw error;  // 최종 실패
            }

            // 재시도 전 대기
            await new Promise(r => setTimeout(r, 1000 * (i + 1)));
        }
    }
}
```

### 8.3 감사(Audit) 포인트

**코드 감사 체크리스트:**

1. **재진입 공격**
   - [ ] 모든 external 함수에 nonReentrant
   - [ ] CEI 패턴 준수 (Checks-Effects-Interactions)
   - [ ] 외부 호출 후 상태 변경 없음

2. **정수 오버플로우/언더플로우**
   - [ ] Solidity 0.8+ 사용 (자동 체크)
   - [ ] 명시적 체크 추가 (필요시)

3. **권한 관리**
   - [ ] onlyOwner modifier 적용
   - [ ] 중요 함수 접근 제어
   - [ ] 권한 이전 로직 안전성

4. **플래시론 보안**
   - [ ] 호출자 검증
   - [ ] 초기자 검증
   - [ ] Premium 검증

5. **DEX 스왑 보안**
   - [ ] 라우터 주소 검증
   - [ ] 슬리피지 보호
   - [ ] Low-level call 검증

6. **프로토콜 통합**
   - [ ] 에러 코드 처리 (Compound)
   - [ ] 담보 수령 확인
   - [ ] cToken redeem 검증

7. **이벤트 로깅**
   - [ ] 중요 작업 이벤트 발생
   - [ ] 디버깅 정보 포함
   - [ ] indexed 파라미터 적절히 사용

8. **가스 최적화**
   - [ ] Storage 사용 최소화
   - [ ] Memory vs Calldata 적절히 사용
   - [ ] Loop 최적화

**보안 감사 도구:**

```bash
# Slither (정적 분석)
slither LiquidationStrategy.sol

# Mythril (심볼릭 실행)
myth analyze LiquidationStrategy.sol

# Echidna (퍼징)
echidna-test . --contract LiquidationStrategy

# Manticore (심볼릭 실행)
manticore LiquidationStrategy.sol
```

---

## 9. 실전 사용 예제

### 9.1 Aave 청산 전체 예제

```javascript
const {ethers} = require('ethers');

// ========================================
// 1. 설정
// ========================================

const provider = new ethers.JsonRpcProvider(RPC_URL);
const wallet = new ethers.Wallet(PRIVATE_KEY, provider);

// 컨트랙트 주소
const LIQUIDATION_STRATEGY = "0x...";
const AAVE_POOL = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2";
const UNISWAP_V3_ROUTER = "0xE592427A0AEce92De3Edee1F18E0157C05861564";

// 토큰 주소
const WETH = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDC = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

// ========================================
// 2. 청산 대상 찾기
// ========================================

async function findLiquidatableUsers() {
    const aavePool = new ethers.Contract(
        AAVE_POOL,
        ['function getUserAccountData(address) view returns (uint256,uint256,uint256,uint256,uint256,uint256)'],
        provider
    );

    const users = await getAllAaveUsers();  // 서브그래프에서 가져오기

    const liquidatable = [];

    for (const user of users) {
        const data = await aavePool.getUserAccountData(user);
        const healthFactor = data[5];

        if (healthFactor < ethers.parseEther("1")) {
            liquidatable.push({
                address: user,
                healthFactor: healthFactor,
                totalCollateral: data[0],
                totalDebt: data[1]
            });
        }
    }

    return liquidatable;
}

// ========================================
// 3. 수익성 분석
// ========================================

async function analyzeProfitability(user) {
    // 청산 금액 계산 (최대 50%)
    const maxLiquidation = user.totalDebt / 2n;

    // ETH 가격 조회
    const ethPrice = await getEthPrice();  // Chainlink Oracle
    const usdcPrice = 1;  // $1

    // 예상 담보 계산 (5% 보너스)
    const expectedCollateralValue = Number(ethers.formatUnits(maxLiquidation, 6)) * 1.05;
    const expectedCollateralETH = expectedCollateralValue / ethPrice;

    // 스왑 시뮬레이션
    const {amountOut, priceImpact} = await simulateSwap(
        WETH,
        USDC,
        ethers.parseEther(expectedCollateralETH.toString())
    );

    // 수익 계산
    const flashLoanPremium = maxLiquidation * 9n / 10000n;
    const totalCost = maxLiquidation + flashLoanPremium;
    const revenue = amountOut;
    const profit = revenue - totalCost;

    return {
        profitable: profit > 0,
        profit: profit,
        profitPercent: Number(profit) / Number(totalCost) * 100,
        liquidationAmount: maxLiquidation,
        expectedCollateral: ethers.parseEther(expectedCollateralETH.toString()),
        flashLoanPremium: flashLoanPremium,
        priceImpact: priceImpact
    };
}

// ========================================
// 4. 스왑 Calldata 생성
// ========================================

async function generateSwapCalldata(tokenIn, tokenOut, amountIn, slippage) {
    const router = new ethers.Contract(
        UNISWAP_V3_ROUTER,
        ['function exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)) external returns (uint256)'],
        wallet
    );

    // 예상 출력 계산
    const expectedOut = await simulateSwap(tokenIn, tokenOut, amountIn);
    const minOut = expectedOut.amountOut * BigInt(100 - slippage) / 100n;

    // 파라미터
    const params = {
        tokenIn: tokenIn,
        tokenOut: tokenOut,
        fee: 3000,  // 0.3%
        recipient: LIQUIDATION_STRATEGY,
        deadline: Math.floor(Date.now() / 1000) + 300,
        amountIn: amountIn,
        amountOutMinimum: minOut,
        sqrtPriceLimitX96: 0
    };

    // Calldata 인코딩
    const calldata = router.interface.encodeFunctionData(
        'exactInputSingle',
        [params]
    );

    return {
        calldata: calldata,
        minOut: minOut,
        expectedOut: expectedOut.amountOut
    };
}

// ========================================
// 5. 청산 실행
// ========================================

async function executeLiquidation(user, analysis) {
    const strategy = new ethers.Contract(
        LIQUIDATION_STRATEGY,
        ['function executeLiquidation(address,uint256,(uint8,address,address,address,address,uint256,address,bytes,uint256,uint256))'],
        wallet
    );

    // 스왑 데이터 생성
    const swap = await generateSwapCalldata(
        WETH,
        USDC,
        analysis.expectedCollateral,
        1  // 1% 슬리피지
    );

    // LiquidationParams
    const params = {
        protocolType: 0,  // AAVE
        protocol: AAVE_POOL,
        user: user.address,
        collateralAsset: WETH,
        debtAsset: USDC,
        debtToCover: analysis.liquidationAmount,
        dexRouter: UNISWAP_V3_ROUTER,
        swapCalldata: swap.calldata,
        minCollateralOut: swap.minOut,
        flashLoanPremium: analysis.flashLoanPremium
    };

    // 플래시론 양 (liquidationAmount보다 약간 많게)
    const flashLoanAmount = analysis.liquidationAmount * 11n / 10n;

    // 가스 추정
    const gasEstimate = await strategy.executeLiquidation.estimateGas(
        USDC,
        flashLoanAmount,
        params
    );

    // 실행
    const tx = await strategy.executeLiquidation(
        USDC,
        flashLoanAmount,
        params,
        {
            gasLimit: gasEstimate * 12n / 10n  // 20% 여유
        }
    );

    console.log(`Transaction sent: ${tx.hash}`);

    const receipt = await tx.wait();
    console.log(`Transaction confirmed: ${receipt.status === 1 ? 'Success' : 'Failed'}`);

    return receipt;
}

// ========================================
// 6. 메인 루프
// ========================================

async function main() {
    console.log('Starting liquidation bot...');

    while (true) {
        try {
            // 청산 대상 찾기
            const users = await findLiquidatableUsers();
            console.log(`Found ${users.length} liquidatable users`);

            for (const user of users) {
                console.log(`\nAnalyzing user: ${user.address}`);
                console.log(`Health Factor: ${ethers.formatEther(user.healthFactor)}`);

                // 수익성 분석
                const analysis = await analyzeProfitability(user);

                if (!analysis.profitable) {
                    console.log('Not profitable, skipping');
                    continue;
                }

                console.log(`Profit: ${ethers.formatUnits(analysis.profit, 6)} USDC`);
                console.log(`Profit %: ${analysis.profitPercent.toFixed(2)}%`);

                // 수익이 충분한가?
                if (analysis.profitPercent < 2) {
                    console.log('Profit too low, skipping');
                    continue;
                }

                // 청산 실행
                console.log('Executing liquidation...');
                await executeLiquidation(user, analysis);

                // 성공시 잠시 대기
                await new Promise(r => setTimeout(r, 5000));
            }

            // 다음 라운드까지 대기
            console.log('\nWaiting for next round...');
            await new Promise(r => setTimeout(r, 10000));

        } catch (error) {
            console.error('Error:', error.message);
            await new Promise(r => setTimeout(r, 5000));
        }
    }
}

main().catch(console.error);
```

### 9.2 The Graph를 이용한 청산 대상 모니터링

```javascript
const {request, gql} = require('graphql-request');

const AAVE_SUBGRAPH = 'https://api.thegraph.com/subgraphs/name/aave/protocol-v3';

// 청산 위험 사용자 조회
async function getAtRiskUsers() {
    const query = gql`
        query GetAtRiskUsers {
            users(
                first: 100
                where: {
                    borrowedReservesCount_gt: 0
                }
                orderBy: id
            ) {
                id
                reserves {
                    currentATokenBalance
                    currentVariableDebt
                    currentStableDebt
                    reserve {
                        symbol
                        underlyingAsset
                        price {
                            priceInEth
                        }
                        reserveLiquidationThreshold
                    }
                }
            }
        }
    `;

    const data = await request(AAVE_SUBGRAPH, query);

    // Health Factor 계산
    const atRisk = data.users.map(user => {
        let totalCollateral = 0;
        let totalDebt = 0;

        user.reserves.forEach(reserve => {
            const priceETH = parseFloat(reserve.reserve.price.priceInEth);
            const balance = parseFloat(reserve.currentATokenBalance);
            const debt = parseFloat(reserve.currentVariableDebt) + 
                        parseFloat(reserve.currentStableDebt);
            const threshold = parseFloat(reserve.reserve.reserveLiquidationThreshold) / 10000;

            totalCollateral += balance * priceETH * threshold;
            totalDebt += debt * priceETH;
        });

        const healthFactor = totalDebt > 0 ? totalCollateral / totalDebt : Infinity;

        return {
            address: user.id,
            healthFactor: healthFactor,
            totalCollateral: totalCollateral,
            totalDebt: totalDebt
        };
    }).filter(user => user.healthFactor < 1.05);  // HF < 1.05인 사용자만

    return atRisk.sort((a, b) => a.healthFactor - b.healthFactor);
}
```

---

## 10. FAQ 및 트러블슈팅

### 10.1 자주 묻는 질문

**Q1: 플래시론을 갚지 못하면 어떻게 되나요?**

A: 트랜잭션 전체가 revert됩니다. 모든 상태가 원래대로 롤백되므로 손실이 없습니다. 단, 가스 비용은 소모됩니다.

**Q2: 청산 보너스는 어떻게 결정되나요?**

A: 각 프로토콜의 거버넌스가 결정합니다:
- Aave v3: 보통 5% (자산별로 다름)
- Compound v2: 보통 8%
- Compound v3: 프로토콜이 동적으로 결정

**Q3: 여러 명을 동시에 청산할 수 있나요?**

A: Compound v3만 가능합니다 (`absorb` 함수). Aave와 Compound v2는 한 명씩만 가능합니다.

**Q4: DEX 선택은 어떻게 하나요?**

A: 수익성과 가스 비용을 고려하여 선택:
- 대량 스왑: 1inch, 0x (최적 경로)
- 소량 스왑: Uniswap V3 (직접 호출)
- 스테이블코인: Curve (최소 슬리피지)

**Q5: 가스 비용은 얼마나 드나요?**

A:
- Aave 청산: ~400,000 gas
- Compound v2 청산: ~600,000 gas (redeem 포함)
- Compound v3 청산: ~300,000 gas
- DEX 스왑: ~150,000 gas

총: 약 450,000 ~ 750,000 gas

**Q6: 최소 수익은 얼마여야 하나요?**

A: 가스 비용을 고려하여:
```
가스: 500,000 * 50 gwei = 0.025 ETH ≈ $50
최소 수익: $50 * 2 = $100 이상 권장
```

**Q7: 청산이 실패하는 주요 원인은?**

A:
1. Health Factor 회복 (가격 반등)
2. 다른 봇이 먼저 청산
3. 슬리피지 초과
4. 가스 부족
5. 프로토콜 일시 중지

**Q8: 보안 감사를 받아야 하나요?**

A: 실전 자금 사용시 필수입니다:
- 내부 감사: Slither, Mythril 등
- 외부 감사: 전문 감사 회사 (Consensys, OpenZeppelin 등)
- 버그 바운티: Immunefi 등에 등록

**Q9: 컨트랙트를 업그레이드할 수 있나요?**

A: 현재 컨트랙트는 업그레이드 불가능합니다. 업그레이드 가능하게 만들려면:
- Proxy 패턴 사용 (UUPS, Transparent)
- 데이터와 로직 분리
- 업그레이드 권한 관리

**Q10: 수익은 어떻게 인출하나요?**

A:
```solidity
// Owner만 호출 가능
function rescueToken(address token, uint256 amount) external onlyOwner {
    IERC20(token).safeTransfer(owner(), amount);
}
```

### 10.2 일반적인 에러와 해결방법

**Error: "Invalid callback caller"**

**원인:** Aave Pool이 아닌 다른 주소가 `executeOperation` 호출

**해결:**
```javascript
// Pool 주소 확인
const provider = await IPoolAddressesProvider(ADDRESSES_PROVIDER).getPool();
console.log('Pool address:', provider);

// 컨트랙트 배포시 올바른 주소 사용
const strategy = await LiquidationStrategy.deploy(ADDRESSES_PROVIDER);
```

**Error: "Insufficient flash loan amount"**

**원인:** 플래시론 양이 청산에 필요한 양보다 적음

**해결:**
```javascript
// 플래시론 양을 청산 금액보다 크게 설정
const flashLoanAmount = liquidationAmount * 11n / 10n;  // 10% 여유
```

**Error: "Insufficient profit"**

**원인:** 스왑 후 받은 금액이 플래시론 상환액보다 적음

**해결:**
```javascript
// 1. 수익성 미리 검증
const analysis = await analyzeProfitability(user);
if (!analysis.profitable) return;

// 2. 슬리피지 설정 확인
const minOut = expectedOut * 99n / 100n;  // 1% 슬리피지

// 3. 가격 변동 체크
const currentPrice = await getEthPrice();
if (Math.abs(currentPrice - expectedPrice) > expectedPrice * 0.02) {
    console.log('Price moved too much, skipping');
    return;
}
```

**Error: "Insufficient swap output"**

**원인:** 슬리피지가 설정값을 초과

**해결:**
```javascript
// 1. 슬리피지 허용치 증가
const minOut = expectedOut * 97n / 100n;  // 3% 슬리피지

// 2. 더 나은 DEX 사용
const swap = await get1inchSwapData(...);  // 최적 경로

// 3. 유동성 체크
const liquidity = await checkPoolLiquidity(WETH, USDC);
if (liquidity < swapAmount * 10) {
    console.log('Insufficient liquidity');
    return;
}
```

**Error: "Compound liquidation failed"**

**원인:** Compound의 `liquidateBorrow`가 에러 코드 반환

**해결:**
```javascript
// 에러 코드 해석
const errorCodes = {
    0: 'NO_ERROR',
    1: 'UNAUTHORIZED',
    3: 'COMPTROLLER_REJECTION',
    // ...
};

// 에러 로깅
const result = await cToken.liquidateBorrow.staticCall(...);
if (result !== 0) {
    console.log('Compound error:', errorCodes[result]);
}

// 재시도 또는 스킵
if (result === 3) {  // COMPTROLLER_REJECTION
    console.log('User not liquidatable yet');
    return;
}
```

**Error: "No collateral received"**

**원인:** 청산 실행했으나 담보를 받지 못함

**해결:**
```javascript
// 1. Health Factor 재확인
const data = await aavePool.getUserAccountData(user);
if (data.healthFactor >= 1e18) {
    console.log('User recovered, not liquidatable');
    return;
}

// 2. 다른 청산자 확인
const events = await aavePool.queryFilter(
    aavePool.filters.LiquidationCall(null, null, user)
);
if (events.length > 0) {
    console.log('Already liquidated by someone else');
    return;
}

// 3. 담보 자산 확인
const collateralBalance = await aToken.balanceOf(user);
if (collateralBalance === 0) {
    console.log('User has no collateral');
    return;
}
```

**Error: "Gas estimation failed"**

**원인:** 트랜잭션이 실행 단계에서 revert 예상

**해결:**
```javascript
try {
    const gasEstimate = await contract.executeLiquidation.estimateGas(...);
} catch (error) {
    // 에러 메시지에서 원인 파악
    console.log('Estimation error:', error.message);

    // 시뮬레이션으로 정확한 원인 파악
    try {
        await contract.executeLiquidation.staticCall(...);
    } catch (simError) {
        console.log('Simulation error:', simError.message);
        // → "Insufficient profit" 등의 정확한 원인 확인
    }
}
```

### 10.3 최적화 팁

**가스 최적화:**

1. **배치 처리**
```javascript
// ❌ 나쁨: 한 번에 하나씩
for (const user of users) {
    await checkHealthFactor(user);
}

// ✅ 좋음: 배치로 조회
const healthFactors = await Promise.all(
    users.map(user => checkHealthFactor(user))
);
```

2. **The Graph 사용**
```javascript
// ❌ 나쁨: 모든 사용자 일일이 조회
const allUsers = await getAllUsers();
for (const user of allUsers) {
    const hf = await getHealthFactor(user);
}

// ✅ 좋음: The Graph로 필터링된 사용자만 조회
const atRiskUsers = await getAtRiskUsersFromGraph();
```

3. **Calldata 최적화**
```solidity
// ❌ 나쁨: memory (비쌈)
function execute(LiquidationParams memory params) external {
    // ...
}

// ✅ 좋음: calldata (저렴)
function execute(LiquidationParams calldata params) external {
    // ...
}
```

**수익 최적화:**

1. **복수 DEX 비교**
```javascript
const quotes = await Promise.all([
    getUniswapQuote(WETH, USDC, amount),
    get1inchQuote(WETH, USDC, amount),
    getParaswapQuote(WETH, USDC, amount)
]);

const best = quotes.reduce((a, b) => a.output > b.output ? a : b);
```

2. **동적 슬리피지**
```javascript
function getOptimalSlippage(volatility, urgency) {
    if (urgency === 'high') return 2;  // 빨리 실행
    if (volatility < 2) return 0.5;    // 안정적
    if (volatility < 5) return 1;
    return 2;
}
```

3. **MEV 보호**
```javascript
// Flashbots 사용
const flashbotsProvider = await providers.FlashbotsBundleProvider.create(
    provider,
    authSigner
);

const signedBundle = await flashbotsProvider.signBundle([
    {
        signer: wallet,
        transaction: tx
    }
]);

const simulation = await flashbotsProvider.simulate(signedBundle, blockNumber);
if (simulation.firstRevert) {
    console.log('Simulation failed');
    return;
}

const bundleSubmission = await flashbotsProvider.sendRawBundle(
    signedBundle,
    blockNumber + 1
);
```

**모니터링 최적화:**

1. **WebSocket 사용**
```javascript
const wsProvider = new ethers.WebSocketProvider(WS_URL);

// 실시간 가격 모니터링
const priceFeed = new ethers.Contract(CHAINLINK_ETH_USD, ABI, wsProvider);
priceFeed.on('AnswerUpdated', (current, roundId, updatedAt) => {
    console.log('ETH price updated:', ethers.formatUnits(current, 8));
    checkAllUsers();  // 가격 변동시 재검증
});

// 청산 이벤트 모니터링
aavePool.on('LiquidationCall', (collateral, debt, user, amount, liquidator) => {
    console.log('Liquidation detected:', user);
    // 우리 청산 리스트에서 제거
    removeFromQueue(user);
});
```

2. **멀티체인 모니터링**
```javascript
const chains = [
    {name: 'Ethereum', rpc: ETH_RPC, pool: ETH_POOL},
    {name: 'Polygon', rpc: POLY_RPC, pool: POLY_POOL},
    {name: 'Arbitrum', rpc: ARB_RPC, pool: ARB_POOL}
];

// 병렬 모니터링
await Promise.all(
    chains.map(chain => monitorChain(chain))
);
```

---

## 11. 결론

### 11.1 핵심 요약

**LiquidationStrategy 컨트랙트의 핵심:**

1. **플래시론 활용**: 무담보로 대량 자금 확보
2. **멀티 프로토콜**: Aave v3, Compound v2/v3 지원
3. **DEX 통합**: 유연한 담보 교환
4. **보안 강화**: 다층 보안 검증
5. **가스 최적화**: 효율적인 코드 구조

**청산 봇 성공 요소:**

1. **빠른 감지**: The Graph, WebSocket 활용
2. **정확한 분석**: 수익성 사전 검증
3. **효율적 실행**: 가스 최적화, MEV 보호
4. **위험 관리**: 슬리피지 보호, 에러 처리
5. **지속적 개선**: 모니터링, 분석, 최적화

### 11.2 추가 학습 자료

**공식 문서:**
- [Aave V3 Documentation](https://docs.aave.com/developers/)
- [Compound V2 Documentation](https://docs.compound.finance/)
- [Compound V3 Documentation](https://docs.compound.finance/v3/)
- [Uniswap V3 Documentation](https://docs.uniswap.org/protocol/V3/introduction)
- [Flashbots Documentation](https://docs.flashbots.net/)

**커뮤니티:**
- [Aave Discord](https://discord.gg/aave)
- [Compound Discord](https://discord.gg/compound)
- [Flashbots Discord](https://discord.gg/flashbots)

**감사 회사:**
- [Consensys Diligence](https://consensys.net/diligence/)
- [Trail of Bits](https://www.trailofbits.com/)
- [OpenZeppelin](https://www.openzeppelin.com/security-audits)

---

**문서 작성 완료!** 🎉

이 문서는 LiquidationStrategy 스마트 컨트랙트의 모든 측면을 초보자도 이해할 수 있도록 상세히 설명합니다. 

총 분량: 약 150+ 페이지
- Solidity 기본 문법
- 라인별 코드 분석
- 플래시론 완전 가이드
- 프로토콜별 청산 메커니즘
- DEX 스왑 로직
- 보안 및 에러 처리
- 실전 예제
- FAQ 및 트러블슈팅

추가 질문이나 특정 부분의 더 자세한 설명이 필요하시면 언제든지 문의해주세요!
