# xCrack MEV Bot 실전 운영 가이드

## 📋 목차

1. [전략 우선순위 및 선정 이유](#전략-우선순위-및-선정-이유)
2. [1단계: Liquidation 전략](#1단계-liquidation-전략)
   - [0단계: 사전 준비 (필수!)](#0단계-사전-준비-필수)
   - [Phase 1: 스마트 컨트랙트 배포](#phase-1-스마트-컨트랙트-배포)
   - [Phase 2: 환경 설정](#phase-2-환경-설정)
   - [Phase 3: Testnet 테스트](#phase-3-testnet-테스트)
   - [Phase 4: Mainnet 운영](#phase-4-mainnet-운영)
3. [2단계: Micro-Arbitrage 전략](#2단계-micro-arbitrage-전략)
4. [3단계: Sandwich 전략](#3단계-sandwich-전략)
5. [4단계: Cross-Chain Arbitrage 전략](#4단계-cross-chain-arbitrage-전략)
6. [자본금 및 리스크 관리](#자본금-및-리스크-관리)

---

## 전략 우선순위 및 선정 이유

### 🎯 **추천 학습 및 운영 순서**

```
1. Liquidation (1~2개월)
   ↓
2. Micro-Arbitrage (1~2개월)
   ↓
3. Sandwich (2~3개월) - 선택사항
   ↓
4. Cross-Chain (2~3개월) - 고급
```

### 📊 **전략 비교 분석**

| 전략 | 구현도 | 리스크 | 수익성 | 경쟁도 | 자본금 | 난이도 | 우선순위 |
|-----|-------|-------|-------|-------|-------|-------|---------|
| **Liquidation** | 95% | ⭐ 낮음 | ⭐⭐⭐ 중상 | ⭐ 낮음 | 0.05 ETH | ⭐⭐ 중 | **1위** |
| **Micro-Arbitrage** | 80% | ⭐⭐ 중간 | ⭐⭐⭐⭐ 높음 | ⭐⭐ 중간 | 5~10 ETH | ⭐⭐⭐ 중상 | **2위** |
| **Sandwich** | 95% | ⭐⭐⭐⭐ 높음 | ⭐⭐⭐⭐⭐ 최고 | ⭐⭐⭐⭐⭐ 최고 | 10+ ETH | ⭐⭐⭐⭐⭐ 최고 | **3위** |
| **Cross-Chain** | 60% | ⭐⭐⭐⭐ 높음 | ⭐⭐ 중간 | ⭐⭐ 중간 | 20+ ETH | ⭐⭐⭐⭐ 높음 | **4위** |

### 🤔 **왜 이 순서인가?**

#### **1위: Liquidation을 먼저 하는 이유**

✅ **리스크가 가장 낮음**
- 경쟁자가 적음 (Sandwich는 수십 개 봇이 경쟁)
- 실패해도 가스비만 손실 (원금 안전)
- Flashloan으로 **초기 자본금 0.05 ETH만 필요** (가스비용)

✅ **안정적인 수익 구조**
- 프로토콜 청산 보상 자동 지급 (3~15%)
- 예측 가능한 수익 모델
- 타이밍 압박 적음

✅ **기술적 진입장벽 낮음**
- 온체인 데이터만 모니터링
- 외부 API 최소 (1inch만 필수)
- Flashbots 필수 아님

✅ **MEV 봇 운영 학습에 최적**
- 전체 플로우 이해 (모니터링 → 분석 → 실행)
- 가스 전략 학습
- 번들 제출 경험

---

## 1단계: Liquidation 전략

### 📌 **기본 정보**

- **기간:** 1~2개월
- **목표:** MEV 봇 운영 기초 마스터 + 안정적 수익
- **리스크:** ⭐ 낮음
- **구현 상태:** 95% 완료
- **필요 자본금:** **0.05 ETH** (가스비용) + **0.02 ETH** (컨트랙트 배포)
- **예상 수익:** 월 3~10 ETH (시장 상황에 따라)

### 🎯 **전략 개요**

DeFi 프로토콜(Aave, Compound, MakerDAO)에서 담보 부족 계정을 감지하고 청산하여 보상을 획득합니다.

**청산 프로세스:**
1. 프로토콜별 사용자 계정 스캔
2. Health Factor < 1.0 계정 탐지
3. **Flashloan으로 청산 자금 조달** (초기 자본 불필요!)
4. 청산 실행 (담보 획득 + 보상)
5. 담보 매각 → Flashloan 상환
6. 순수익 = 청산 보상 - 가스비 - Flashloan 수수료 (0.09%)

---

## 0단계: 사전 준비 (필수!)

### ⚠️ **중요: Flashloan 사용을 위한 필수 조건**

Liquidation 전략을 **Flashloan 모드**로 운영하려면 (초기 자본 0), **스마트 컨트랙트 배포가 필수**입니다!

| 실행 방식 | 컨트랙트 필요 | Flashloan | 트랜잭션 수 | 가스비 | 초기 자본 | 권장도 |
|----------|-------------|-----------|-----------|--------|----------|--------|
| **1. Liquidation Contract** | ✅ 필수 | ✅ 가능 | 1개 | 100% | 0.05 ETH | ⭐⭐⭐⭐⭐ |
| **2. FlashLoan Receiver** | ✅ 필수 | ✅ 가능 | 1개 | 120% | 0.05 ETH | ⭐⭐⭐⭐ |
| **3. EOA 직접 실행** | ❌ 불필요 | ❌ 불가 | 3-5개 | 250% | **10+ ETH** | ⭐ |

**결론**: 컨트랙트 없으면 **10 ETH 이상 필요**, 컨트랙트 있으면 **0.05 ETH만 필요**!

---

### 🛠️ **필요한 도구 설치**

#### 1️⃣ Foundry 설치 (스마트 컨트랙트 배포 도구)

```bash
# Foundry 설치
curl -L https://foundry.paradigm.xyz | bash
foundryup

# 설치 확인
forge --version
# forge 0.2.0 (...)
cast --version
# cast 0.2.0 (...)
```

#### 2️⃣ API 키 발급 (무료, 각 5분)

| 서비스 | 용도 | 발급 URL | 필수 여부 | 비용 |
|--------|------|---------|----------|------|
| **Alchemy** | 블록체인 연결 | https://www.alchemy.com | ✅ 필수 | 무료 |
| **Etherscan** | 컨트랙트 검증 | https://etherscan.io/myapikey | ✅ 필수 | 무료 |
| **1inch** | DEX 스왑 | https://portal.1inch.dev | ✅ 필수 | 무료 |
| **0x** | DEX 스왑 (백업) | https://0x.org/docs | ⭕ 선택 | 무료 |

**API 키 발급 순서**:

```bash
# 1. Alchemy
# → https://www.alchemy.com 가입
# → "Create App" → Ethereum Mainnet 선택
# → API Key 복사 (예: abc123def456)

# 2. Etherscan
# → https://etherscan.io/register 가입
# → My Profile → API Keys → Add
# → API Key 복사

# 3. 1inch
# → https://portal.1inch.dev 가입
# → API Keys → Create
# → API Key 복사
```

#### 3️⃣ 지갑 준비

```bash
# 옵션 1: MetaMask에서 Private Key 추출
# MetaMask → 계정 세부 정보 → 개인키 내보내기

# 옵션 2: 청산 봇 전용 새 지갑 생성 (권장)
cast wallet new

# 출력:
# Successfully created new keypair.
# Address:     0x742d35Cc6634C0532925a3b844Bc9e7595f0bEe
# Private key: 0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
```

**⚠️ 보안 주의**:
- Private Key는 **절대 GitHub에 업로드 금지**
- 청산 봇 전용 **새 지갑 생성 권장**
- `.env.local` 파일은 `.gitignore`에 추가

---

## Phase 1: 스마트 컨트랙트 배포

### 🔥 **왜 컨트랙트가 필요한가?**

**Flashloan 없이 (EOA 직접 실행)**:
```
👤 당신의 지갑
  ↓ 10 ETH 필요 (청산 자금)
  ↓
1️⃣ approve(debtAsset) → 가스비 $5
2️⃣ liquidationCall() → 가스비 $80
3️⃣ approve(collateralAsset) → 가스비 $5
4️⃣ swap() → 가스비 $40
  ↓
💰 순수익 = 청산 보상 - $130 가스비
⚠️ 초기 자본: 10 ETH 필요
⚠️ MEV 취약 (여러 트랜잭션 사이 끼어들기 가능)
```

**Flashloan 컨트랙트 사용 시**:
```
👤 당신의 지갑 (0.05 ETH만 필요!)
  ↓
📜 LiquidationStrategy 컨트랙트
  ↓
🏦 Aave Flash Loan (10 ETH 빌림)
  ↓
1️⃣ 청산 실행 + 담보 받음 + 판매 + 상환 → 가스비 $50
  ↓
💰 순수익 = 청산 보상 - $50 - Flashloan 수수료 (0.09%)
✅ 초기 자본: 0.05 ETH (가스비만!)
✅ 원자적 실행 (MEV 방어)
✅ 가스비 60% 절감
```

---

### 📝 **Step 1-1: 컨트랙트 코드 작성**

```bash
# 프로젝트 루트에서
mkdir -p contracts
```

**파일: `contracts/LiquidationStrategy.sol`**

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IPoolAddressesProvider} from "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import {IPool} from "@aave/core-v3/contracts/interfaces/IPool.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IFlashLoanSimpleReceiver} from "@aave/core-v3/contracts/flashloan/interfaces/IFlashLoanSimpleReceiver.sol";

/**
 * @title LiquidationStrategy
 * @notice Aave Flash Loan 기반 청산 전략 컨트랙트
 * @dev 1개 트랜잭션으로 flashLoan → liquidation → swap → repay 실행
 */
contract LiquidationStrategy is IFlashLoanSimpleReceiver {
    IPoolAddressesProvider public immutable ADDRESSES_PROVIDER;
    IPool public immutable POOL;
    address public immutable owner;

    struct LiquidationParams {
        address protocolPool;      // Aave/Compound 청산 대상 프로토콜
        address user;              // 청산 대상 사용자
        address collateralAsset;   // 받을 담보 자산
        address debtAsset;         // 상환할 부채 자산
        uint256 debtAmount;        // 청산 금액
        address swapTarget;        // 0x/1inch 스왑 라우터
        bytes swapCalldata;        // 스왑 트랜잭션 데이터
    }

    constructor(address _addressProvider) {
        ADDRESSES_PROVIDER = IPoolAddressesProvider(_addressProvider);
        POOL = IPool(ADDRESSES_PROVIDER.getPool());
        owner = msg.sender;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    /**
     * @notice 청산 실행 (외부 호출)
     * @param asset 부채 자산 (Flashloan으로 빌릴 토큰)
     * @param amount 청산 금액
     * @param params 청산 파라미터 (ABI 인코딩)
     */
    function executeLiquidation(
        address asset,
        uint256 amount,
        bytes calldata params
    ) external onlyOwner {
        // Aave V3 Flash Loan 시작
        POOL.flashLoanSimple(
            address(this),  // receiver
            asset,          // 빌릴 자산
            amount,         // 빌릴 금액
            params,         // executeOperation에 전달될 데이터
            0               // referralCode
        );
    }

    /**
     * @notice Flashloan 콜백 (Aave가 자동 호출)
     * @dev 1. 청산 실행 → 2. 담보 판매 → 3. Flashloan 상환
     */
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        require(msg.sender == address(POOL), "Caller must be Pool");
        require(initiator == address(this), "Initiator must be this");

        // 파라미터 디코딩
        LiquidationParams memory liqParams = abi.decode(params, (LiquidationParams));

        // ===========================
        // 1️⃣ 청산 실행 (Aave liquidationCall)
        // ===========================
        IERC20(asset).approve(liqParams.protocolPool, amount);
        IPool(liqParams.protocolPool).liquidationCall(
            liqParams.collateralAsset,
            liqParams.debtAsset,
            liqParams.user,
            liqParams.debtAmount,
            false  // receiveAToken = false (담보를 직접 받음)
        );

        // ===========================
        // 2️⃣ 받은 담보 판매 (0x/1inch)
        // ===========================
        uint256 collateralBalance = IERC20(liqParams.collateralAsset).balanceOf(address(this));
        require(collateralBalance > 0, "No collateral received");

        IERC20(liqParams.collateralAsset).approve(liqParams.swapTarget, collateralBalance);
        (bool success, ) = liqParams.swapTarget.call(liqParams.swapCalldata);
        require(success, "Swap failed");

        // ===========================
        // 3️⃣ Flash Loan 상환
        // ===========================
        uint256 amountOwed = amount + premium;
        IERC20(asset).approve(address(POOL), amountOwed);

        // 4️⃣ 남은 수익은 owner에게 전송
        uint256 profit = IERC20(asset).balanceOf(address(this));
        if (profit > 0) {
            IERC20(asset).transfer(owner, profit);
        }

        return true;
    }

    /**
     * @notice 긴급 출금 (컨트랙트에 남은 토큰 회수)
     */
    function emergencyWithdraw(address token) external onlyOwner {
        uint256 balance = IERC20(token).balanceOf(address(this));
        if (balance > 0) {
            IERC20(token).transfer(owner, balance);
        }
    }

    // Aave 인터페이스 요구사항
    function ADDRESSES_PROVIDER() external view returns (IPoolAddressesProvider) {
        return ADDRESSES_PROVIDER;
    }

    function POOL() external view returns (IPool) {
        return POOL;
    }
}
```

---

### 🚀 **Step 1-2: 컨트랙트 배포**

#### **Testnet 배포 (Sepolia)**

```bash
# 1. 환경 변수 설정
cat > .env << EOF
PRIVATE_KEY=0xYOUR_PRIVATE_KEY
RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
ETHERSCAN_API_KEY=YOUR_ETHERSCAN_KEY
EOF

# 2. Aave V3 Pool Address Provider (Sepolia)
POOL_PROVIDER=0x012bAC54348C0E635dCAc9D5FB99f06F24136C9A

# 3. 컨트랙트 배포
forge create \
  --rpc-url $RPC_URL \
  --private-key $PRIVATE_KEY \
  --constructor-args $POOL_PROVIDER \
  --verify \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  contracts/LiquidationStrategy.sol:LiquidationStrategy

# 출력 예시:
# Deployer: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEe
# Deployed to: 0x1234567890abcdef1234567890abcdef12345678
# Transaction hash: 0xabcdef...
# 가스 사용: 0.015 ETH
```

#### **Mainnet 배포** (Testnet 검증 후)

```bash
# 1. Mainnet 환경 변수
cat > .env << EOF
PRIVATE_KEY=0xYOUR_MAINNET_PRIVATE_KEY
RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
ETHERSCAN_API_KEY=YOUR_ETHERSCAN_KEY
EOF

# 2. Aave V3 Pool Address Provider (Mainnet)
POOL_PROVIDER=0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e

# 3. Mainnet 배포
forge create \
  --rpc-url $RPC_URL \
  --private-key $PRIVATE_KEY \
  --constructor-args $POOL_PROVIDER \
  --verify \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  contracts/LiquidationStrategy.sol:LiquidationStrategy

# 출력:
# Deployed to: 0xYOUR_MAINNET_CONTRACT_ADDRESS
# 가스 사용: ~0.02 ETH (~$56)
```

**✅ 배포 성공 체크리스트**:
- [ ] Etherscan에서 컨트랙트 검증 완료 (녹색 체크)
- [ ] `owner` 주소가 당신의 지갑 주소와 일치
- [ ] `POOL()` 함수가 Aave Pool 주소 반환
- [ ] 컨트랙트 주소 복사 (다음 단계에서 사용)

---

## Phase 2: 환경 설정

### 📝 **Step 2-1: `.env.local` 파일 생성**

```bash
# 프로젝트 루트에서
cat > .env.local << 'EOF'
# ===========================
# 필수 설정 (반드시 채워야 함!)
# ===========================

# Alchemy API 키
WS_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
HTTP_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY

# 지갑 Private Key
WALLET_PRIVATE_KEY=0xYOUR_PRIVATE_KEY

# 배포한 청산 컨트랙트 주소 (Phase 1에서 복사)
LIQUIDATION_CONTRACT=0xYOUR_DEPLOYED_CONTRACT_ADDRESS

# 1inch API 키
ONEINCH_API_KEY=YOUR_1INCH_API_KEY

# ===========================
# 선택 설정 (기본값 사용 가능)
# ===========================

# 실행 모드
LIQUIDATION_MODE=scan  # scan | auto | analyze | test

# 자금 조달 모드
FUNDING_MODE=flashloan  # flashloan (권장) | auto | wallet

# Redis (선택사항, 없으면 메모리 모드)
# REDIS_URL=redis://localhost:6379

# Flashbots (선택사항)
# FLASHBOTS_RELAY_URL=https://relay.flashbots.net
# FLASHBOTS_SIGNER_KEY=0xYOUR_PRIVATE_KEY
EOF

# .gitignore에 추가 (보안!)
echo ".env.local" >> .gitignore
```

---

### 🔧 **Step 2-2: `config/liquidation.toml` 생성**

```bash
# config 디렉토리 생성
mkdir -p config

cat > config/liquidation.toml << 'EOF'
[network]
chain_id = 1
http_url = "${HTTP_URL}"
ws_url = "${WS_URL}"

[blockchain.primary_network]
# ✅ 배포한 컨트랙트 주소 (환경 변수에서 읽음)
liquidation_contract = "${LIQUIDATION_CONTRACT}"

[liquidation]
scan_interval_seconds = 30
min_profit_eth = "50000000000000000"  # 0.05 ETH
min_liquidation_amount = "1000000000000000000"  # 1 ETH
max_concurrent_liquidations = 3
health_factor_threshold = 1.0
gas_multiplier = 1.5
max_gas_price = "200000000000"  # 200 Gwei

[liquidation.funding]
mode = "flashloan"  # ✅ Flashloan 활성화!
flashloan_fee_bps = 9  # Aave v3: 0.09%

[protocols.aave_v3]
name = "Aave V3"
lending_pool_address = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
price_oracle_address = "0x54586bE62E3c3580375aE3723C145253060Ca0C2"
liquidation_fee = 500  # 5%
min_health_factor = 1.0
supported_assets = [
    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",  # WETH
    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",  # USDC
    "0xdAC17F958D2ee523a2206206994597C13D831ec7",  # USDT
    "0x6B175474E89094C44Da98b954EedeAC495271d0F",  # DAI
]

[protocols.compound_v3]
name = "Compound V3"
lending_pool_address = "0xc3d688B66703497DAA19211EEdff47fB25365b65"
liquidation_fee = 750  # 7.5%
min_health_factor = 1.0

[protocols.maker_dao]
name = "MakerDAO"
lending_pool_address = "0x135954d155898D42C90D2a57824C690e0c7BEf1B"  # Dog
price_oracle_address = "0x35D1b3F3D7966A1DFe207aa4514C12a259A0492B"  # Vat
liquidation_fee = 1300  # 13%

[dex.oneinch]
api_url = "https://api.1inch.dev"
api_key = "${ONEINCH_API_KEY}"

[dex.zerox]
api_url = "https://api.0x.org"
# api_key = "${ZEROX_API_KEY}"  # 선택사항
EOF
```

---

## Phase 3: Testnet 테스트

### 🧪 **Step 3-1: Mock 모드 테스트 (API 키 없이)**

```bash
# 시스템 검증
API_MODE=mock LIQUIDATION_MODE=test cargo run --bin liquidation_bot

# 예상 출력:
# 🧪 Running liquidation system test...
# 1. Testing system connectivity... ✅
# 2. Testing protocol scanners... ✅
# 3. Testing strategy engine... ✅
# 4. Testing execution engine (dry run)... ✅
# 5. Testing configuration... ✅
#
# 🎉 All tests passed! System is ready for operation.
```

---

### 🌐 **Step 3-2: Sepolia Testnet 테스트**

```bash
# 1. Sepolia 환경 변수 설정
cat > .env.local << EOF
WS_URL=wss://eth-sepolia.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
HTTP_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
WALLET_PRIVATE_KEY=0xYOUR_TESTNET_PRIVATE_KEY
LIQUIDATION_CONTRACT=0xYOUR_SEPOLIA_CONTRACT_ADDRESS
ONEINCH_API_KEY=YOUR_1INCH_KEY
LIQUIDATION_MODE=scan
FUNDING_MODE=flashloan
EOF

# 2. Sepolia ETH 받기
# https://sepoliafaucet.com 에서 0.5 ETH 받기

# 3. 스캔 모드 실행 (청산 기회만 탐색)
export $(cat .env.local | xargs)
LIQUIDATION_MODE=scan cargo run --bin liquidation_bot

# 예상 출력:
# 🔍 청산 기회 발견: 3 개
# 💡 Top 5 Opportunities:
#   1. User: 0x742d35...001 | Profit: $120.00 | HF: 0.9235
#   2. User: 0x742d35...002 | Profit: $85.00 | HF: 0.9512

# 4. 분석 모드 (상세 통계)
LIQUIDATION_MODE=analyze cargo run --bin liquidation_bot

# 5. 자동 실행 (실제 청산 테스트!)
LIQUIDATION_MODE=auto cargo run --bin liquidation_bot
```

**Testnet 체크리스트**:
- [ ] Mock 모드 테스트 통과
- [ ] Sepolia 스캔 모드 정상 작동
- [ ] 최소 1회 청산 성공 (analyze로 확인)
- [ ] Etherscan에서 트랜잭션 확인
- [ ] 가스비 모니터링 (0.01 ETH 이하)

---

## Phase 4: Mainnet 운영

### 🚀 **Step 4-1: Mainnet 설정**

```bash
# Mainnet .env.local
cat > .env.local << 'EOF'
# Mainnet RPC
WS_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
HTTP_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY

# Mainnet 지갑 (0.05 ETH 가스비 필요)
WALLET_PRIVATE_KEY=0xYOUR_MAINNET_PRIVATE_KEY

# Mainnet 컨트랙트 (Phase 1에서 배포)
LIQUIDATION_CONTRACT=0xYOUR_MAINNET_CONTRACT_ADDRESS

# API 키
ONEINCH_API_KEY=YOUR_1INCH_KEY

# 모드
LIQUIDATION_MODE=auto
FUNDING_MODE=flashloan
EOF
```

---

### 📊 **Step 4-2: 단계별 Mainnet 진입**

#### **Week 1: 관찰 모드**

```bash
# 스캔 모드로 시장 상황 파악
LIQUIDATION_MODE=scan cargo run --bin liquidation_bot

# 매일 로그 확인
# - 청산 기회 빈도 확인
# - 평균 수익 분석
# - 경쟁 상황 파악
```

#### **Week 2: 고수익 선별**

```bash
# config/liquidation.toml 수정
min_profit_eth = "100000000000000000"  # 0.1 ETH로 상향

# 자동 실행 (하루 최대 3~5회)
LIQUIDATION_MODE=auto cargo run --bin liquidation_bot
```

#### **Week 3-4: 본격 운영**

```bash
# 최소 수익 하향
min_profit_eth = "50000000000000000"  # 0.05 ETH

# 24/7 자동 실행
nohup cargo run --release --bin liquidation_bot > logs/liq.log 2>&1 &
```

---

### 📈 **Step 4-3: 모니터링**

```bash
# 1. 실시간 로그
tail -f logs/liq.log

# 2. 통계 조회
LIQUIDATION_MODE=analyze cargo run --bin liquidation_bot

# 출력:
# 📈 LIQUIDATION ANALYSIS REPORT
# ===============================
#
# 🎯 Strategy Performance:
#   Total Opportunities: 156
#   Total Profit Realized: $3,450.00
#   Success Rate: 88.89%
#   Average Execution Time: 234.5ms

# 3. 에러 확인
grep "ERROR" logs/liq.log

# 4. 수익 확인
grep "successful" logs/liq.log | tail -20
```

---

## 💰 **예상 수익 시나리오**

### **보수적 시나리오** (성공률 70%)
```
하루 평균 기회: 5개
평균 수익/건: $80
월 수익: 5 × $80 × 0.7 × 30 = $8,400
가스비: ~$1,200
Flashloan 수수료: ~$200

순수익: ~$7,000 (약 2.5 ETH)
ROI: 5,000% (초기 자본 0.05 ETH 기준)
```

### **일반적 시나리오** (성공률 85%)
```
하루 평균 기회: 8개
평균 수익/건: $120
월 수익: 8 × $120 × 0.85 × 30 = $24,480
가스비: ~$2,400
Flashloan 수수료: ~$600

순수익: ~$21,480 (약 7.5 ETH)
ROI: 15,000% (초기 자본 0.05 ETH 기준)
```

---

## 🎓 **학습 체크리스트**

### **Phase 1: 컨트랙트 배포 (1주)**
- [ ] Foundry 설치 완료
- [ ] API 키 발급 (Alchemy, Etherscan, 1inch)
- [ ] Sepolia Testnet 컨트랙트 배포 성공
- [ ] Mainnet 컨트랙트 배포 성공
- [ ] Etherscan 검증 완료

### **Phase 2: 환경 설정 (1주)**
- [ ] `.env.local` 파일 생성
- [ ] `config/liquidation.toml` 설정
- [ ] Mock 모드 테스트 통과
- [ ] 지갑에 0.05 ETH 충전

### **Phase 3: Testnet 테스트 (2주)**
- [ ] Sepolia 스캔 모드 성공
- [ ] 최소 5회 청산 성공
- [ ] 가스비 모니터링 정상
- [ ] Flashloan 트랜잭션 성공

### **Phase 4: Mainnet 운영 (4주)**
- [ ] Week 1: 관찰 모드 (스캔만)
- [ ] Week 2: 고수익 선별 실행
- [ ] Week 3-4: 본격 24/7 운영
- [ ] 성공률 70% 이상 달성

---

## 🐛 **트러블슈팅**

### **문제 1: "No liquidation opportunities found"**

**원인**:
- Health Factor > 1.0 (청산 불가)
- 최소 수익성 너무 높음

**해결**:
```bash
# 최소 수익 하향
min_profit_eth = "10000000000000000"  # 0.01 ETH

# scan 모드로 시장 확인
LIQUIDATION_MODE=scan cargo run --bin liquidation_bot
```

---

### **문제 2: "Flashloan execution failed"**

**원인**:
- 컨트랙트 주소 미설정
- 지갑과 컨트랙트 owner 불일치

**해결**:
```bash
# 1. 컨트랙트 주소 확인
echo $LIQUIDATION_CONTRACT
# 0x1234... (반드시 0x로 시작)

# 2. Owner 확인
cast call $LIQUIDATION_CONTRACT "owner()(address)" --rpc-url $HTTP_URL
# 출력이 당신의 지갑 주소와 일치해야 함

# 3. config 확인
grep "liquidation_contract" config/liquidation.toml
```

---

### **문제 3: "DEX aggregator error"**

**원인**:
- 1inch API 키 누락
- API Rate Limit

**해결**:
```bash
# API 키 확인
echo $ONEINCH_API_KEY

# API 테스트
curl -X GET "https://api.1inch.dev/swap/v5.2/1/quote?src=0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2&dst=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48&amount=1000000000000000000" \
  -H "Authorization: Bearer $ONEINCH_API_KEY"
```

---

## 🎯 **1단계 완료 기준**

다음 조건을 모두 만족하면 2단계(Micro-Arbitrage)로 진행:

- ✅ Mainnet에서 **최소 30회 청산 성공**
- ✅ 성공률 **70% 이상**
- ✅ 월 순수익 **5 ETH 이상**
- ✅ 24/7 자동 운영 **1개월 이상**
- ✅ 가스비 최적화 완료 (건당 $50 이하)

**축하합니다! 이제 2단계로 진행하세요!** 🚀

---

## 2단계: Micro-Arbitrage 전략

*(내용 생략, 기존 문서와 동일)*

---

## 자본금 및 리스크 관리

### 💰 **단계별 자본금 요구사항**

| 단계 | 전략 | 최소 자본금 | 권장 자본금 | 용도 |
|-----|------|-----------|-----------|------|
| **1단계** | Liquidation | **0.05 ETH** | 0.1 ETH | 가스비 (Flashloan 사용) |
| **2단계** | Micro-Arbitrage | 5 ETH | 10 ETH | 거래 자금 |
| **3단계** | Sandwich | 10 ETH | 20 ETH | 가스 경매 + 포지션 |
| **4단계** | Cross-Chain | 20 ETH | 40 ETH | 멀티체인 분산 |

### 📊 **누적 수익 시뮬레이션 (Flashloan 모드)**

```
1~2개월 (Liquidation):
  - 초기 자본: 0.05 ETH (가스비)
  - 컨트랙트 배포: 0.02 ETH
  - 월 수익: 7.5 ETH (일반적 시나리오)
  - 누적: 0.07 + 7.5×2 = 15.07 ETH

3~4개월 (Micro-Arbitrage):
  - 초기 자본: 15 ETH
  - 월 수익: 10 ETH
  - 누적: 15 + 10×2 = 35 ETH

5~7개월 (Sandwich):
  - 초기 자본: 35 ETH
  - 월 수익: 15 ETH
  - 누적: 35 + 15×3 = 80 ETH

총 수익: 80 - 0.07 (초기) = 79.93 ETH
ROI: 114,186% (7개월)
```

---

## 🎯 **최종 요약**

### **Liquidation 전략 핵심**

1. ✅ **스마트 컨트랙트 배포 필수** (Flashloan 사용)
2. ✅ **초기 자본 0.05 ETH만 필요** (가스비)
3. ✅ **컨트랙트 배포 비용 0.02 ETH** (1회만)
4. ✅ **Testnet에서 충분히 테스트** (최소 5회 성공)
5. ✅ **Mainnet은 관찰 모드부터 시작** (안전 우선)

### **성공을 위한 체크리스트**

- [ ] Foundry 설치 및 컨트랙트 배포 완료
- [ ] API 키 발급 (Alchemy, Etherscan, 1inch)
- [ ] `.env.local` 및 `config/liquidation.toml` 설정
- [ ] Testnet 테스트 5회 이상 성공
- [ ] Mainnet 소액 테스트 성공
- [ ] 24/7 모니터링 시스템 구축

---

**⚠️ 면책 조항**

본 문서는 교육 목적으로 작성되었습니다. 스마트 컨트랙트 배포 및 MEV 봇 운영은 고위험 활동이며, 실제 자금 투입 전 충분한 테스트와 이해가 필요합니다. 저자는 본 문서 사용으로 인한 어떠한 손실에도 책임을 지지 않습니다.

**🚀 행운을 빕니다!**
