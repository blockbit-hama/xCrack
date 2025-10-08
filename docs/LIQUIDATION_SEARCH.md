# 🔍 청산 대상자 탐지 메커니즘

## 핵심 질문

**"수백만 개의 이더리움 주소 중에서 어떻게 대출받은 사용자를 찾는가?"**

---

## 방법 1: 멤풀 모니터링 - 트랜잭션 디코딩

### 메커니즘

**경쟁 봇의 청산 트랜잭션을 가로채서 청산 대상자 주소를 추출**

### 1단계: Pending 트랜잭션 구독

```rust
// mempool_watcher.rs:149-151
let mut pending_tx_stream = self.provider.watch_pending_transactions().await?;

// 실시간으로 멤풀의 모든 pending 트랜잭션 수신
```

**동작:**
- WebSocket으로 이더리움 노드에 연결
- 멤풀에 들어오는 모든 트랜잭션을 실시간 스트림으로 받음
- 트랜잭션이 블록에 포함되기 **전**에 감지

### 2단계: 청산 트랜잭션 식별

```rust
// mempool_watcher.rs:188-194
if let Some(to) = tx.to {
    // 1. 대출 프로토콜 주소인가?
    if self.is_lending_protocol_address(&to) {
        // 2. 청산 함수 호출인가?
        if self.is_liquidation_call(&tx.input) {
            // 청산 트랜잭션 발견!
```

#### Step 2-1: 프로토콜 주소 확인

```rust
// mempool_watcher.rs:210-222
fn is_lending_protocol_address(&self, address: &Address) -> bool {
    let lending_protocols = vec![
        "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2", // Aave V3 Pool
        "0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B", // Compound V2 Comptroller
        "0xc3d688B66703497DAA19211EEdff47f25384cdc3", // Compound V3 Comet
        "0x9759A6Ac90977b93B58547b4A71c78317f391A28", // MakerDAO Cat
    ];

    lending_protocols.iter().any(|addr| {
        addr.parse::<Address>().map(|a| a == *address).unwrap_or(false)
    })
}
```

**동작:** 트랜잭션의 `to` 주소가 알려진 대출 프로토콜인지 확인

#### Step 2-2: Function Selector 확인

```rust
// mempool_watcher.rs:236-254
fn is_liquidation_call(&self, input: &ethers::types::Bytes) -> bool {
    let function_selector = &input[0..4];  // 첫 4바이트

    // 각 프로토콜의 청산 함수 서명
    let liquidation_selectors = vec![
        [0xe8, 0xef, 0xa4, 0x40], // Aave liquidationCall()
        [0xf5, 0xe3, 0xc4, 0x62], // Compound liquidateBorrow()
        [0x72, 0xc6, 0xc1, 0xe6], // MakerDAO bite()
    ];

    liquidation_selectors.iter().any(|selector| function_selector == selector)
}
```

**Function Selector란?**

이더리움 트랜잭션의 `input` 데이터 구조:
```
[4바이트 Function Selector] + [32바이트 Param1] + [32바이트 Param2] + ...
```

Function Selector는 함수 서명의 **Keccak256 해시 첫 4바이트**:

```
함수 서명: "liquidationCall(address,address,address,uint256,bool)"
Keccak256: 0xe8efa440dc753ae92db54fa1e3e87e0cc6a855f1f5a76b542fdfeb014594f986
Selector:   0xe8efa440  ← 첫 4바이트
```

### 3단계: 청산 대상자 주소 추출

**트랜잭션 Input 데이터 파싱**

```
실제 청산 트랜잭션 예시:

tx.to = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2  (Aave V3 Pool)
tx.input =
  0xe8efa440                                                          [Selector: liquidationCall]
  000000000000000000000000C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2  [Param 1: collateralAsset = WETH]
  000000000000000000000000A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48  [Param 2: debtAsset = USDC]
  0000000000000000000000001234567890123456789012345678901234567890  [Param 3: user ← 청산 대상자!]
  00000000000000000000000000000000000000000000000056bc75e2d63100000  [Param 4: debtToCover = 100 USDC]
  0000000000000000000000000000000000000000000000000000000000000000  [Param 5: receiveAToken = false]
```

**파싱 코드:**

```rust
// ABI 디코딩
let decoded = ethabi::decode(
    &[
        ParamType::Address,  // collateralAsset
        ParamType::Address,  // debtAsset
        ParamType::Address,  // user
        ParamType::Uint(256), // debtToCover
        ParamType::Bool,      // receiveAToken
    ],
    &tx.input[4..],  // Selector 제외
)?;

let user_address = decoded[2].clone().into_address().unwrap();
// → 0x1234567890123456789012345678901234567890
```

**이제 이 주소로 무엇을 하는가?**

1. **선점 전략**: 같은 사용자를 더 높은 가스로 청산
2. **대체 전략**: 같은 사용자의 다른 담보/부채 쌍으로 청산
3. **모니터링**: 이 사용자를 DB에 저장하고 계속 추적

### 추가 신호: 오라클 가격 업데이트

```rust
// mempool_watcher.rs:198-200
if self.is_oracle_address(&to) {
    self.process_oracle_update(tx.clone()).await?;
}
```

**왜 중요한가?**

ETH 가격이 $3000 → $2500으로 하락하면:
- ETH를 담보로 한 모든 사용자의 건강도 하락
- 건강도 < 1.0이 되면 즉시 청산 가능

**오라클 주소 목록:**
```rust
let oracle_addresses = vec![
    "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419", // Chainlink ETH/USD
    "0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6", // Chainlink USDC/USD
];
```

**동작:**
1. 오라클 업데이트 트랜잭션 감지
2. 가격 변화 추출 (input 데이터 디코딩)
3. Position Scanner에 즉시 알림
4. 영향받는 사용자들 긴급 스캔

---

## 방법 2: 온체인 스캔 - 이벤트 로그 수집

### 메커니즘

**대출 프로토콜의 이벤트 로그를 읽어서 대출받은 사용자 목록 수집**

### 핵심 개념: 이벤트 로그

스마트 컨트랙트는 중요한 동작마다 **이벤트(Event)**를 발생시킵니다:

```solidity
// Aave V3 Pool 컨트랙트
event Borrow(
    address indexed reserve,      // 빌린 자산 (USDC, DAI 등)
    address user,                  // 사용자 주소 ← 여기!
    address indexed onBehalfOf,    // 대신 빌려준 주소
    uint256 amount,                // 대출 금액
    uint8 interestRateMode,        // 이자율 모드
    uint256 borrowRate,            // 이자율
    uint16 indexed referralCode    // 레퍼럴 코드
);

event Supply(
    address indexed reserve,   // 예금한 자산
    address user,              // 사용자 주소 ← 여기!
    address indexed onBehalfOf,
    uint256 amount,            // 예금 금액
    uint16 indexed referralCode
);
```

**이벤트는 블록체인에 영구 저장되며, 누구나 읽을 수 있습니다!**

### 방법 2-1: 이벤트 로그 직접 스캔

```rust
use ethers::prelude::*;

// 1. Aave V3 Pool 컨트랙트 연결
let pool = AaveV3Pool::new(
    "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".parse()?,
    provider.clone()
);

// 2. 최근 1000블록의 Borrow 이벤트 조회
let current_block = provider.get_block_number().await?;

let borrow_events = pool
    .event::<BorrowFilter>()
    .from_block(current_block - 1000)
    .to_block(current_block)
    .query()
    .await?;

// 3. 사용자 주소 수집
let mut user_addresses = HashSet::new();
for event in borrow_events {
    user_addresses.insert(event.user);
}

// 4. 각 사용자의 건강도 확인
for user in user_addresses {
    let user_data = pool.get_user_account_data(user).call().await?;

    if user_data.health_factor < U256::from(1e18 as u64) {
        // health_factor < 1.0 → 청산 가능!
        liquidation_candidates.push(user);
    }
}
```

**동작 흐름:**
```
최근 1000블록 이벤트 로그 읽기
    ↓
Borrow 이벤트에서 user 주소 추출
    ↓
중복 제거 (HashSet)
    ↓
각 사용자에 대해 get_user_account_data() 호출
    ↓
health_factor < 1.0인 사용자만 필터링
    ↓
청산 대상자 목록 완성
```

### 방법 2-2: The Graph 서브그래프 API

**The Graph란?**
- 블록체인 데이터를 미리 인덱싱해둔 서비스
- GraphQL로 복잡한 쿼리를 빠르게 실행
- Aave, Compound 등 주요 프로토콜 지원

**실제 사용 예시:**

```rust
use reqwest;
use serde_json::json;

// 1. GraphQL 쿼리 작성
let query = json!({
    "query": r#"
        query {
            users(
                where: {
                    healthFactor_lt: "1.5"
                }
                orderBy: healthFactor
                orderDirection: asc
                first: 100
            ) {
                id
                healthFactor
                totalCollateralETH
                totalDebtETH
                borrowedReservesCount
                collateralReservesCount
            }
        }
    "#
});

// 2. The Graph API 호출
let response = reqwest::Client::new()
    .post("https://api.thegraph.com/subgraphs/name/aave/protocol-v3")
    .json(&query)
    .send()
    .await?;

let data: SubgraphResponse = response.json().await?;

// 3. 결과 파싱
for user in data.users {
    if user.health_factor < 1.0 {
        println!("청산 대상: {}", user.id);
        println!("  건강도: {}", user.health_factor);
        println!("  담보: {} ETH", user.total_collateral_eth);
        println!("  부채: {} ETH", user.total_debt_eth);
    }
}
```

**반환 데이터 예시:**
```json
{
  "data": {
    "users": [
      {
        "id": "0x1234567890123456789012345678901234567890",
        "healthFactor": "0.95",
        "totalCollateralETH": "100.5",
        "totalDebtETH": "95.2",
        "borrowedReservesCount": 2,
        "collateralReservesCount": 3
      },
      {
        "id": "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
        "healthFactor": "0.88",
        "totalCollateralETH": "250.0",
        "totalDebtETH": "240.0",
        "borrowedReservesCount": 1,
        "collateralReservesCount": 2
      }
    ]
  }
}
```

**장점:**
- ✅ 한 번의 API 호출로 정렬된 결과
- ✅ 미리 계산된 건강도
- ✅ RPC 호출 비용 절감
- ✅ 실시간 업데이트 (5-10초 지연)

**단점:**
- ❌ 외부 서비스 의존
- ❌ 약간의 지연 (블록 인덱싱 시간)
- ❌ API 요청 제한

### 방법 2-3: 자체 DB 모니터링

**가장 빠른 방법: 알려진 고위험 주소 추적**

```rust
// 자체 PostgreSQL DB 사용
let users = sqlx::query!(
    r#"
    SELECT
        address,
        protocol,
        last_health_factor,
        total_collateral_usd,
        total_debt_usd
    FROM monitored_users
    WHERE
        protocol = 'Aave'
        AND last_health_factor < 1.2
        AND total_debt_usd > 10000
    ORDER BY last_health_factor ASC
    LIMIT 100
    "#
)
.fetch_all(&db_pool)
.await?;

// 각 사용자의 최신 상태 확인
for user in users {
    let current_data = pool.get_user_account_data(user.address).call().await?;

    // DB 업데이트
    update_user_in_db(&user.address, current_data).await?;

    // 청산 가능 체크
    if current_data.health_factor < 1.0 {
        execute_liquidation(&user).await?;
    }
}
```

**동작:**
1. 백그라운드로 모든 대출자를 주기적으로 스캔
2. 건강도 < 1.5인 고위험 사용자를 DB에 저장
3. 이 사용자들만 5초마다 재확인
4. 건강도 < 1.0이 되면 즉시 청산

**장점:**
- ✅ 가장 빠름 (추적 중인 주소만 체크)
- ✅ 자체 제어 가능
- ✅ 외부 의존성 없음

**단점:**
- ❌ 초기 DB 구축 필요
- ❌ 새로운 대출자 발견이 느림
- ❌ 인프라 비용

---

## 방법 3: 하이브리드 전략 (실전 사용)

### 최적 조합

```rust
pub struct LiquidationFinder {
    // 방법 1: 멤풀 모니터링
    mempool_watcher: Arc<LiquidationMempoolWatcher>,

    // 방법 2-1: 이벤트 로그 스캔
    event_scanner: Arc<EventLogScanner>,

    // 방법 2-2: The Graph API
    subgraph_client: Arc<SubgraphClient>,

    // 방법 2-3: 자체 DB
    database: Arc<Database>,
}

impl LiquidationFinder {
    pub async fn run(&self) -> Result<()> {
        // 스레드 1: 멤풀 실시간 모니터링 (가장 빠름)
        tokio::spawn(async move {
            self.mempool_watcher.watch_pending_txs().await;
        });

        // 스레드 2: The Graph 주기 조회 (30초마다)
        tokio::spawn(async move {
            loop {
                let users = self.subgraph_client.get_high_risk_users().await?;
                self.database.update_monitored_users(users).await?;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });

        // 스레드 3: DB 고위험 사용자 실시간 추적 (5초마다)
        tokio::spawn(async move {
            loop {
                let high_risk = self.database.get_high_risk_users().await?;
                for user in high_risk {
                    self.check_and_liquidate(&user).await?;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });

        // 스레드 4: 이벤트 로그 백그라운드 스캔 (1분마다)
        tokio::spawn(async move {
            loop {
                let new_users = self.event_scanner.scan_recent_borrows().await?;
                self.database.add_new_users(new_users).await?;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        Ok(())
    }
}
```

### 각 방법의 역할

| 방법 | 주기 | 목적 | 속도 |
|------|------|------|------|
| 멤풀 모니터링 | 실시간 | 경쟁 선점 | ⚡⚡⚡ |
| The Graph | 30초 | 신규 고위험자 발견 | ⚡⚡ |
| DB 추적 | 5초 | 알려진 고위험자 감시 | ⚡⚡⚡ |
| 이벤트 로그 | 1분 | 신규 대출자 수집 | ⚡ |

---

## 실제 예시: 청산 대상자 발견 과정

### 시나리오: 신규 사용자가 대출받음

#### T+0초: 사용자가 Aave에서 대출

```
사용자 트랜잭션:
- 담보 예금: 100 ETH ($300,000 @ $3,000)
- 대출: 200,000 USDC
- 건강도: 1.2 (안전)

발생 이벤트:
- Supply(WETH, 0xUser123, 100 ETH)
- Borrow(USDC, 0xUser123, 200,000 USDC)
```

#### T+60초: 이벤트 로그 스캐너가 발견

```rust
// 백그라운드 스캐너
let events = pool.event::<BorrowFilter>()
    .from_block(current - 5)
    .query()
    .await?;

// 새 사용자 발견
for event in events {
    if !database.user_exists(event.user) {
        database.add_user(event.user).await?;
    }
}
```

**결과:**
- 0xUser123 → DB에 추가
- 건강도: 1.2 (아직 안전)

#### T+1시간: ETH 가격 하락 → 멤풀에서 감지

```
Chainlink Oracle 트랜잭션 (멤풀):
- ETH/USD 가격 업데이트: $3,000 → $2,500

멤풀 모니터:
- 오라클 주소 확인: 0x5f4eC3... (Chainlink ETH/USD)
- 가격 변화 추출: -16.7%
- 긴급 신호 발생! 🚨
```

#### T+1시간 5초: DB 스캐너가 즉시 재확인

```rust
// 오라클 업데이트 신호를 받은 DB 스캐너
let eth_holders = database.get_users_with_collateral("WETH").await?;

for user in eth_holders {
    let data = pool.get_user_account_data(user).await?;

    // 0xUser123의 새 건강도 계산
    // 담보 가치: 100 ETH × $2,500 = $250,000
    // 부채: $200,000
    // 건강도: ($250,000 × 0.8) / $200,000 = 1.0

    if data.health_factor < 1.0 {
        // 청산 가능! ✅
        liquidation_opportunities.push(user);
    }
}
```

#### T+1시간 6초: 경쟁 봇이 먼저 발견

```
멤풀에 새 트랜잭션 등장:
{
  from: 0xCompetitor...,
  to: 0x87870Bca... (Aave Pool),
  input: 0xe8efa440  // liquidationCall
         ...WETH...
         ...USDC...
         ...0xUser123...  ← 청산 대상자!
         ...
}

멤풀 모니터:
1. liquidationCall 함수 감지
2. Input 데이터 디코딩
3. 청산 대상자 추출: 0xUser123
4. 즉시 선점 시도! (더 높은 가스)
```

**최종 결과:**
- 멤풀 모니터가 가장 빠르게 대응
- 경쟁 봇보다 2 gwei 높은 가스로 선점 성공

---

## 핵심 요약

### 청산 대상자를 찾는 3가지 핵심 메커니즘

#### 1. 멤풀 트랜잭션 디코딩
```
경쟁 봇의 청산 트랜잭션 → Input 데이터 파싱 → 사용자 주소 추출
```

**구현:**
- Function Selector 확인 (첫 4바이트)
- ABI 디코딩으로 파라미터 추출
- 청산 대상자 주소 획득

#### 2. 이벤트 로그 스캔
```
Borrow/Supply 이벤트 → 사용자 주소 수집 → 건강도 확인
```

**구현:**
- `pool.event::<BorrowFilter>().query()`
- 최근 N블록의 이벤트 읽기
- 각 주소에 대해 `get_user_account_data()` 호출

#### 3. The Graph 서브그래프
```
GraphQL 쿼리 → 미리 계산된 건강도 → 정렬된 결과
```

**구현:**
- `users(where: { healthFactor_lt: "1.5" })`
- 한 번의 API 호출로 완성된 목록
- 5-10초 지연 있음

### 실전 전략

**하이브리드 조합:**
1. 멤풀 모니터링 (실시간) → 경쟁 선점
2. The Graph (30초 주기) → 신규 고위험자 발견
3. DB 추적 (5초 주기) → 알려진 고위험자 감시
4. 이벤트 스캔 (1분 주기) → 신규 대출자 수집

**결과:**
- 빠른 대응 (멤풀 실시간)
- 넓은 커버리지 (The Graph + 이벤트)
- 안정적 추적 (DB 모니터링)
