# 🎓 청산 대상자 탐지 - 초보자 가이드

## 목차
1. [개념 이해하기](#개념-이해하기)
2. [이벤트 로그란?](#이벤트-로그란)
3. [실전 예제: 코드와 함께](#실전-예제-코드와-함께)
4. [자주 묻는 질문](#자주-묻는-질문)

---

## 개념 이해하기

### 핵심 질문

**Q: 이더리움에 수백만 개의 주소가 있는데, 누가 대출받았는지 어떻게 알아?**

**A: 스마트 컨트랙트가 "일기장"에 기록을 남기기 때문입니다!**

### 비유로 이해하기

```
실생활 은행:
1. 김철수가 은행에서 대출
2. 은행 직원이 장부에 기록: "김철수, 2025-01-08, 1억원 대출"
3. 나중에 장부를 보면 김철수가 대출받았음을 알 수 있음

이더리움:
1. 사용자가 Aave에서 대출
2. Aave 컨트랙트가 "이벤트 로그"에 기록: "0x1234..., 2025-01-08, 200,000 USDC 대출"
3. 나중에 이벤트 로그를 읽으면 누가 대출받았는지 알 수 있음!
```

---

## 이벤트 로그란?

### 스마트 컨트랙트는 "일기"를 씁니다

스마트 컨트랙트는 중요한 일이 생길 때마다 블록체인에 기록을 남깁니다.

**Solidity 코드 (Aave V3 Pool)**:

```solidity
contract Pool {
    // 이벤트 정의 (일기 양식)
    event Borrow(
        address indexed reserve,      // 어떤 자산을 빌렸나? (USDC, DAI...)
        address user,                  // 누가 빌렸나? ← 이게 우리가 찾는 것!
        address indexed onBehalfOf,
        uint256 amount,                // 얼마나 빌렸나?
        uint8 interestRateMode,        // 이자율 모드
        uint256 borrowRate,            // 이자율
        uint16 indexed referralCode
    );

    // 대출 함수
    function borrow(
        address asset,
        uint256 amount,
        uint256 interestRateMode,
        uint16 referralCode,
        address onBehalfOf
    ) external {
        // 1. 대출 로직 실행
        // ...코드 생략...

        // 2. 이벤트 발생! (블록체인에 영구 기록)
        emit Borrow(
            asset,
            msg.sender,      // 함수를 호출한 사용자 주소
            onBehalfOf,
            amount,
            interestRateMode,
            borrowRate,
            referralCode
        );
    }
}
```

**emit Borrow(...)**를 호출하면:
- 블록체인에 영구 저장됨 ✅
- 누구나 읽을 수 있음 ✅
- 삭제 불가능 ✅

### 이벤트 로그 실제 모습

블록체인에 저장된 실제 이벤트 로그:

```json
{
  "blockNumber": 21234567,
  "transactionHash": "0xabcd1234...",
  "address": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",  // Aave V3 Pool
  "topics": [
    "0x13ed6866d4e1ee6da46f845c46d7e6aa3a3f7b92e5a6a8b7a2b8b0a7a0a7a0a7",  // Borrow 이벤트 시그니처
    "0x000000000000000000000000A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",  // reserve (USDC)
    "0x0000000000000000000000001234567890123456789012345678901234567890"   // onBehalfOf
  ],
  "data": "0x000000000000000000000000abcdefabcdefabcdefabcdefabcdefabcdefabcd..."  // user, amount, 등
}
```

**우리가 해야 할 일**:
1. `topics[0]`이 Borrow 이벤트 시그니처인지 확인
2. `data` 필드에서 `user` 주소 추출
3. 그 사용자가 청산 가능한지 확인

---

## 실전 예제: 코드와 함께

### Step 1: 이벤트 로그 조회하기

**우리 코드**: `src/protocols/aave.rs:89-130`

```rust
use ethers::{
    providers::{Provider, Middleware},
    types::{Filter, Log, BlockNumber, H256, H160},
};

async fn get_active_users(&self) -> Result<Vec<H160>> {
    // 1️⃣ 현재 블록 번호 (실제로는 provider.get_block_number() 사용)
    let current_block = 21234567u64;

    // 2️⃣ 스캔할 블록 범위 결정
    // 최근 1000블록만 스캔 (약 3.3시간)
    let from_block = if current_block > 1000 {
        current_block - 1000  // 21233567
    } else {
        0
    };

    println!("📊 Scanning blocks {} to {}", from_block, current_block);

    // 3️⃣ 중복 제거를 위한 HashSet
    let mut users = std::collections::HashSet::new();
```

### Step 2: Borrow 이벤트 필터 생성

```rust
    // 4️⃣ Borrow 이벤트 필터 생성
    let borrow_filter = Filter::new()
        .address(self.pool_address)  // Aave V3 Pool 주소
        .topic0(H256::from_slice(
            &hex::decode("13ed6866d4e1ee6da46f845c46d7e6aa3a3f7b92e5a6a8b7a2b8b0a7a0a7a0a7")
                .unwrap()
        ))  // Borrow 이벤트 시그니처
        .from_block(from_block)
        .to_block(BlockNumber::Latest);

    println!("🔍 Querying Borrow events...");
```

**무슨 뜻인가요?**
- `.address(...)`: Aave V3 Pool 컨트랙트에서 발생한 이벤트만
- `.topic0(...)`: Borrow 이벤트만 (다른 이벤트는 무시)
- `.from_block(...).to_block(...)`: 블록 21233567 ~ 21234567

### Step 3: 이벤트 로그 받기

```rust
    // 5️⃣ 이더리움 노드에 요청
    let borrow_logs: Vec<Log> = self.provider
        .get_logs(&borrow_filter)
        .await?;

    println!("✅ Found {} Borrow events", borrow_logs.len());
```

**실제 반환 예시**:

```rust
borrow_logs = vec![
    Log {
        block_number: 21233570,
        topics: [
            H256::from_slice(...),  // Borrow 시그니처
            H256::from_slice(...),  // reserve
            H256::from_slice(...),  // onBehalfOf
        ],
        data: Bytes::from(...),  // user, amount, 등
    },
    Log {
        block_number: 21233890,
        topics: [...],
        data: Bytes::from(...),
    },
    // ... 총 157개의 이벤트
]
```

### Step 4: 사용자 주소 추출

```rust
    // 6️⃣ 각 로그에서 사용자 주소 추출
    for log in borrow_logs {
        // topics[1]에서 user 주소 가져오기
        if let Some(user_topic) = log.topics.get(1) {
            // H256 (32바이트)의 마지막 20바이트가 주소
            let user = H160::from_slice(&user_topic.0[12..]);
            users.insert(user);  // HashSet에 추가 (자동 중복 제거)

            println!("👤 Found user: {:?}", user);
        }
    }
```

**왜 `[12..]`인가요?**

```
H256은 32바이트:
[00 00 00 00 00 00 00 00 00 00 00 00 12 34 56 78 90 12 34 56 78 90 12 34 56 78 90 12 34 56 78 90]
 ← 앞 12바이트 (패딩) →  ←─────────── 뒤 20바이트 (실제 주소) ───────────→

주소는 20바이트이므로 [12..]로 잘라냄
```

### Step 5: Deposit 이벤트도 같은 방법으로

```rust
    // 7️⃣ Deposit 이벤트도 조회
    let deposit_filter = Filter::new()
        .address(self.pool_address)
        .topic0(H256::from_slice(
            &hex::decode("2b627736bca15cd5381dcf80b85eaae9c6d54c5fc5d0b6b3e6b39e6c3c00ea7")
                .unwrap()
        ))  // Deposit 이벤트 시그니처
        .from_block(from_block)
        .to_block(BlockNumber::Latest);

    let deposit_logs: Vec<Log> = self.provider
        .get_logs(&deposit_filter)
        .await?;

    println!("✅ Found {} Deposit events", deposit_logs.len());

    for log in deposit_logs {
        if let Some(user_topic) = log.topics.get(2) {  // Deposit은 topics[2]
            let user = H160::from_slice(&user_topic.0[12..]);
            users.insert(user);
        }
    }
```

### Step 6: 결과 반환

```rust
    // 8️⃣ HashSet을 Vec로 변환
    let user_list: Vec<H160> = users.into_iter().collect();

    println!("🎯 Total unique users found: {}", user_list.len());
    println!("Users: {:?}", user_list);

    Ok(user_list)
}
```

**예상 출력**:

```
📊 Scanning blocks 21233567 to 21234567
🔍 Querying Borrow events...
✅ Found 157 Borrow events
👤 Found user: 0x1234567890123456789012345678901234567890
👤 Found user: 0xabcdefabcdefabcdefabcdefabcdefabcdefabcd
...
✅ Found 243 Deposit events
🎯 Total unique users found: 342
```

---

## 건강도 체크: 누가 청산 가능한가?

### getUserAccountData() 호출

**우리 코드**: `src/protocols/aave.rs:132-160`

```rust
async fn get_user_account_data_detailed(&self, user: H160) -> Result<Option<LiquidatableUser>> {
    println!("🔍 Checking user: {:?}", user);

    // 1️⃣ Aave Pool 컨트랙트 호출
    let account_data: (U256, U256, U256, U256, U256, U256) = self.pool_contract
        .method::<_, (U256, U256, U256, U256, U256, U256)>(
            "getUserAccountData",  // 함수 이름
            user                    // 파라미터
        )?
        .call()
        .await
        .map_err(|e| anyhow!("Failed to get user account data for {}: {}", user, e))?;

    // 2️⃣ 반환값 분해
    let (
        total_collateral_base,           // 전체 담보 (USD, 8 decimals)
        total_debt_base,                  // 전체 부채 (USD, 8 decimals)
        available_borrows_base,           // 추가 대출 가능 금액
        current_liquidation_threshold,    // 청산 임계값 (%)
        ltv,                              // Loan-To-Value 비율
        health_factor                     // 건강도 (18 decimals)
    ) = account_data;

    println!("📊 Account Data:");
    println!("  Collateral: ${:.2}", total_collateral_base.as_u128() as f64 / 1e8);
    println!("  Debt: ${:.2}", total_debt_base.as_u128() as f64 / 1e8);
    println!("  Health Factor: {:.4}", health_factor.as_u128() as f64 / 1e18);
```

### 건강도 판단

```rust
    // 3️⃣ health_factor를 사람이 읽을 수 있는 숫자로 변환
    let health_factor_f64 = health_factor.as_u128() as f64 / 1e18;

    // 4️⃣ 청산 가능 여부 확인
    if total_debt_base.is_zero() {
        println!("⚠️ User has no debt, skipping");
        return Ok(None);
    }

    if health_factor_f64 >= 1.0 {
        println!("✅ User is healthy (HF = {:.4}), skipping", health_factor_f64);
        return Ok(None);
    }

    println!("🚨 LIQUIDATABLE! Health Factor = {:.4}", health_factor_f64);
```

**예상 출력**:

```
🔍 Checking user: 0x1234567890123456789012345678901234567890
📊 Account Data:
  Collateral: $250000.00
  Debt: $200000.00
  Health Factor: 1.0000
✅ User is healthy (HF = 1.0000), skipping

🔍 Checking user: 0xabcdefabcdefabcdefabcdefabcdefabcdefabcd
📊 Account Data:
  Collateral: $200000.00
  Debt: $200000.00
  Health Factor: 0.8000
🚨 LIQUIDATABLE! Health Factor = 0.8000
```

### 담보/부채 상세 조회

```rust
    // 5️⃣ 각 자산별 담보/부채 조회
    let mut collateral_positions = Vec::new();
    let mut debt_positions = Vec::new();

    // 지원되는 모든 자산 확인 (WETH, USDC, DAI, ...)
    for &asset in &self.supported_assets {
        println!("  Checking asset: {:?}", asset);

        // 사용자의 이 자산에 대한 정보 조회
        let reserve_data = self.data_provider_contract
            .method::<_, (U256, U256, U256, ...)>(
                "getUserReserveData",
                (asset, user)
            )?
            .call()
            .await
            .unwrap_or_default();

        let (
            current_atoken_balance,      // aToken 잔액 (담보)
            current_stable_debt,          // 고정 금리 부채
            current_variable_debt,        // 변동 금리 부채
            ...
        ) = reserve_data;

        // 담보가 있으면 추가
        if !current_atoken_balance.is_zero() {
            println!("    💰 Collateral: {} tokens", current_atoken_balance.as_u128() as f64 / 1e18);

            collateral_positions.push(CollateralPosition {
                asset,
                amount: current_atoken_balance,
                usd_value: ...,
                ...
            });
        }

        // 부채가 있으면 추가
        let total_debt = current_stable_debt + current_variable_debt;
        if !total_debt.is_zero() {
            println!("    💸 Debt: {} tokens", total_debt.as_u128() as f64 / 1e18);

            debt_positions.push(DebtPosition {
                asset,
                amount: total_debt,
                usd_value: ...,
                ...
            });
        }
    }
```

**예상 출력**:

```
  Checking asset: 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2  (WETH)
    💰 Collateral: 100.0 tokens
  Checking asset: 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48  (USDC)
    💸 Debt: 200000.0 tokens
  Checking asset: 0x6B175474E89094C44Da98b954EedeAC495271d0F  (DAI)
    (no position)
```

### 우선순위 점수 계산

```rust
    // 6️⃣ 우선순위 점수 계산
    let total_collateral_usd = total_collateral_base.as_u128() as f64 / 1e8;
    let total_debt_usd = total_debt_base.as_u128() as f64 / 1e8;

    // 공식: 부채 × (1 - 건강도) / 건강도
    // 건강도가 낮을수록 점수가 높음
    let priority_score = if health_factor_f64 > 0.0 {
        total_debt_usd * (1.0 - health_factor_f64) / health_factor_f64
    } else {
        total_debt_usd * 1000.0  // 건강도 = 0이면 최고 우선순위
    };

    println!("🎯 Priority Score: {:.2}", priority_score);

    Ok(Some(LiquidatableUser {
        address: user,
        protocol: ProtocolType::Aave,
        account_data: UserAccountData {
            total_collateral_usd,
            total_debt_usd,
            health_factor: health_factor_f64,
            ...
        },
        collateral_positions,
        debt_positions,
        priority_score,
        ...
    }))
}
```

**우선순위 점수 예시**:

```
사용자 A:
  담보: $100,000
  부채: $100,000
  건강도: 0.95
  점수 = 100,000 × (1 - 0.95) / 0.95 = 5,263

사용자 B:
  담보: $200,000
  부채: $200,000
  건강도: 0.80
  점수 = 200,000 × (1 - 0.80) / 0.80 = 50,000

→ 사용자 B가 더 위험하므로 우선 청산!
```

---

## 전체 흐름 요약

### 30초마다 실행되는 프로세스

```rust
// state_indexer.rs
async fn indexing_loop(&self) -> Result<()> {
    loop {
        println!("⏰ Starting scan cycle...");

        // 1️⃣ 이벤트 로그에서 활성 사용자 찾기
        let active_users = get_active_users().await?;
        println!("👥 Found {} active users", active_users.len());

        // 2️⃣ 각 사용자의 건강도 확인
        let mut liquidatable_users = Vec::new();
        for user in active_users {
            if let Some(liquidatable) = get_user_account_data_detailed(user).await? {
                liquidatable_users.push(liquidatable);
            }
        }
        println!("🚨 Found {} liquidatable users", liquidatable_users.len());

        // 3️⃣ 우선순위 순으로 정렬
        liquidatable_users.sort_by(|a, b| {
            b.priority_score.partial_cmp(&a.priority_score).unwrap()
        });

        // 4️⃣ 청산 실행 (최고 우선순위 사용자)
        if let Some(top_candidate) = liquidatable_users.first() {
            println!("🎯 Liquidating top candidate: {:?}", top_candidate.address);
            execute_liquidation(top_candidate).await?;
        }

        // 5️⃣ 30초 대기
        println!("💤 Sleeping for 30 seconds...\n");
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
```

**실행 로그 예시**:

```
⏰ Starting scan cycle...
📊 Scanning blocks 21233567 to 21234567
🔍 Querying Borrow events...
✅ Found 157 Borrow events
✅ Found 243 Deposit events
👥 Found 342 active users

🔍 Checking user: 0x1234...
✅ User is healthy (HF = 1.2000), skipping
🔍 Checking user: 0xabcd...
✅ User is healthy (HF = 1.0500), skipping
🔍 Checking user: 0x5678...
🚨 LIQUIDATABLE! Health Factor = 0.8000
🎯 Priority Score: 50000.00
...

🚨 Found 3 liquidatable users
🎯 Liquidating top candidate: 0x5678...
✅ Liquidation successful!

💤 Sleeping for 30 seconds...
```

---

## 자주 묻는 질문

### Q1: 왜 1000블록만 스캔하나요?

**A**: 효율성과 속도의 균형입니다.

```
전체 블록 스캔 (15,000,000블록):
- 장점: 모든 사용자 발견
- 단점: 엄청 느림 (수 시간), RPC 비용 높음

최근 1000블록 스캔:
- 장점: 빠름 (수 초), 비용 저렴
- 단점: 오래된 사용자는 놓칠 수 있음
- 현실: 30초마다 반복하므로 충분함
```

**이유**:
- 1000블록 = 약 3.3시간
- 건강도가 1.0 미만으로 떨어진 사용자는 보통 최근에 활동했을 가능성이 높음
- 30초마다 반복하므로 놓쳐도 다음 주기에 발견됨

### Q2: 이벤트 시그니처는 어떻게 구하나요?

**A**: Keccak256 해시를 사용합니다.

```javascript
// JavaScript 예제
const { keccak256 } = require("@ethersproject/keccak256");
const { toUtf8Bytes } = require("@ethersproject/strings");

// 이벤트 시그니처
const signature = "Borrow(address,address,address,uint256,uint8,uint256,uint16)";

// Keccak256 해시
const hash = keccak256(toUtf8Bytes(signature));
console.log(hash);
// → 0x13ed6866d4e1ee6da46f845c46d7e6aa3a3f7b92e5a6a8b7a2b8b0a7a0a7a0a7
```

**더 쉬운 방법**: Etherscan에서 컨트랙트 ABI를 보면 이벤트 시그니처가 나와 있습니다!

### Q3: health_factor는 어떻게 계산되나요?

**A**: Aave의 공식:

```
health_factor = (담보 가치 × 청산 임계값) / 부채 가치

예시 1: 안전한 사용자
담보: 100 ETH × $2,500 = $250,000
청산 임계값: 0.8 (80%)
부채: $200,000
HF = ($250,000 × 0.8) / $200,000 = 1.0 ✅

예시 2: 위험한 사용자 (ETH 가격 하락)
담보: 100 ETH × $2,000 = $200,000
청산 임계값: 0.8 (80%)
부채: $200,000
HF = ($200,000 × 0.8) / $200,000 = 0.8 🚨
```

**규칙**:
- `HF >= 1.0`: 안전
- `HF < 1.0`: 청산 가능

### Q4: 왜 배치로 나누어 처리하나요?

**A**: RPC 레이트 리밋 때문입니다.

```rust
const BATCH_SIZE: usize = 10;
for chunk in active_users.chunks(BATCH_SIZE) {
    // 10명씩 동시 처리
    let results = join_all(batch_futures).await;

    // 100ms 딜레이
    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

**이유**:
- 한 번에 너무 많은 요청을 보내면 RPC 노드가 차단할 수 있음
- 10명씩 나누어 처리하고 100ms 쉬면 안전함
- 총 342명 → 342/10 = 34개 배치 → 34 × 100ms = 3.4초

### Q5: The Graph API를 사용하지 않는 이유는?

**A**: 현재는 구현되지 않았지만, 추가하면 더 좋습니다!

**장점**:
- ✅ 복잡한 쿼리 가능 (건강도 < 1.5인 사용자 정렬)
- ✅ 한 번의 API 호출로 여러 정보
- ✅ RPC 호출 비용 절감

**단점**:
- ❌ 외부 서비스 의존
- ❌ 5-10초 지연
- ❌ API 요청 제한

**결론**: 이벤트 로그 스캔을 메인으로 하고, The Graph API를 보조로 사용하는 것이 최선!

---

## 다음 단계

현재 우리 코드는 **이벤트 로그 스캔**만 사용합니다.

**추가하면 좋을 것**:
1. ✅ The Graph API 통합
2. ✅ PostgreSQL DB로 사용자 히스토리 추적
3. ✅ 실시간 Chainlink 가격 피드
4. ✅ 멀티 프로토콜 지원 (Compound, MakerDAO)

다음 문서에서 The Graph API 통합 방법을 다루겠습니다!
