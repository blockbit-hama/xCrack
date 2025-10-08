# xCrack 프로젝트 분리 계획

## 🎯 목표

현재 단일 모노리스 프로젝트를 **전략별 독립 프로젝트**로 분리:
- 청산 (Liquidation)
- 샌드위치 (Sandwich)
- 마이크로 아비트러지 (MicroArb)

각 전략은 **독립 백엔드 + 독립 프론트엔드**로 구성

---

## 📁 최종 디렉토리 구조

```
blockbit/
├── xCrack/                          # 현재 프로젝트 (레거시 참조용)
│
├── xCrack-Liquidation/              # 청산 백엔드
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── api.rs                   # REST API (port 8081)
│   │   ├── scanner/                 # 프로토콜 스캐너
│   │   ├── executor/                # 청산 실행
│   │   ├── thegraph.rs              # The Graph 통합
│   │   └── storage/                 # PostgreSQL
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
│
├── xCrack-Sandwich/                 # 샌드위치 백엔드
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── api.rs                   # REST API (port 8082)
│   │   ├── mempool/                 # 멤풀 모니터링
│   │   ├── target_analyzer.rs       # 타겟 분석
│   │   ├── bundle_builder.rs        # 번들 구성
│   │   └── executor.rs              # 샌드위치 실행
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
│
├── xCrack-MicroArb/                # 마이크로 아비트러지 백엔드
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── api.rs                   # REST API (port 8083)
│   │   ├── dex_monitor.rs           # DEX 모니터링
│   │   ├── cex_connector.rs         # CEX 연동
│   │   └── arbitrage.rs             # 아비트러지 실행
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
│
├── xCrack-Front/                    # 🎯 단일 통합 프론트엔드
│   ├── package.json
│   ├── next.config.js
│   ├── app/
│   │   ├── layout.tsx               # 공통 레이아웃
│   │   ├── page.tsx                 # 통합 대시보드
│   │   ├── liquidation/             # 청산 페이지들
│   │   │   ├── page.tsx
│   │   │   ├── positions/
│   │   │   ├── opportunities/
│   │   │   └── history/
│   │   ├── sandwich/                # 샌드위치 페이지들
│   │   │   ├── page.tsx
│   │   │   ├── mempool/
│   │   │   ├── targets/
│   │   │   └── bundles/
│   │   ├── microarb/                # 아비트러지 페이지들
│   │   │   ├── page.tsx
│   │   │   ├── opportunities/
│   │   │   └── markets/
│   │   └── settings/                # 통합 설정
│   ├── components/
│   │   ├── ui/                      # 공통 UI 컴포넌트
│   │   ├── layout/                  # 레이아웃 컴포넌트
│   │   └── strategy/                # 전략별 컴포넌트
│   ├── lib/
│   │   ├── api.ts                   # 3개 백엔드 API 통합
│   │   ├── utils.ts
│   │   └── types.ts
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
│
├── xCrack-Shared/                  # 공통 Rust 라이브러리
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs                 # 공통 타입 정의
│       ├── flashbots.rs             # Flashbots 클라이언트
│       ├── blockchain.rs            # RPC 유틸
│       ├── profitability.rs         # 수익성 계산
│       └── utils.rs                 # 공통 유틸
│
└── xCrack-Deploy/                  # 통합 배포 스크립트
    ├── docker-compose.all.yml       # 4개 서비스 (백엔드 3개 + 프론트 1개)
    ├── docker-compose.liquidation.yml
    ├── docker-compose.sandwich.yml
    ├── docker-compose.microarb.yml
    ├── k8s/                         # Kubernetes
    │   ├── liquidation/
    │   ├── sandwich/
    │   └── microarb/
    └── README.md
```

---

## 🔌 포트 할당

| 서비스 | 백엔드 포트 | 프론트엔드 포트 |
|--------|-------------|-----------------|
| Liquidation | 8081 | 3001 |
| Sandwich | 8082 | 3002 |
| MicroArb | 8083 | 3003 |
| PostgreSQL | 5432 | - |
| Redis | 6379 | - |

---

## 📋 마이그레이션 단계

### Phase 1: 공통 라이브러리 생성 ✅

**1-1. xCrack-Shared (Rust)**
```bash
cd blockbit/
cargo new --lib xCrack-Shared
cd xCrack-Shared
```

**이동할 코드:**
- `src/types.rs` → 공통 타입 정의
- `src/common/abi.rs` → ABI 유틸
- `src/common/profitability.rs` → 수익성 계산
- `src/common/utils.rs` → 공통 유틸
- `src/flashbots/` → Flashbots 클라이언트
- `src/blockchain/rpc.rs` → RPC 유틸

**1-2. xCrack-UI-Shared (Next.js)**
```bash
cd blockbit/
npx create-next-app@latest xCrack-UI-Shared --typescript --tailwind --app
cd xCrack-UI-Shared
```

**이동할 코드:**
- `crack_front/components/ui/` → 공통 UI 컴포넌트
- `crack_front/lib/utils.ts` → 공통 유틸
- `crack_front/lib/api.ts` → API 클라이언트 베이스

---

### Phase 2: Liquidation 분리 (우선순위 1) 🎯

**2-1. 백엔드 (xCrack-Liquidation)**
```bash
cd blockbit/
cargo new xCrack-Liquidation
cd xCrack-Liquidation
```

**Cargo.toml 의존성:**
```toml
[dependencies]
xcrack-shared = { path = "../xCrack-Shared" }
tokio = { version = "1", features = ["full"] }
axum = "0.7"
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls"] }
ethers = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**이동할 코드:**
- `src/strategies/liquidation/` → `src/`
- `src/protocols/` → `src/protocols/`
- `src/storage/` → `src/storage/`
- `docs/LIQUIDATION_*.md` → `docs/`
- `abi/*.json` → `abi/`
- `migrations/` → `migrations/`

**새로 작성:**
- `src/main.rs` - 독립 실행 가능한 메인
- `src/api.rs` - REST API 엔드포인트
- `Dockerfile`
- `docker-compose.yml`

**2-2. 프론트엔드 (xCrack-Liquidation-Front)**
```bash
cd blockbit/
npx create-next-app@latest xCrack-Liquidation-Front --typescript --tailwind --app
cd xCrack-Liquidation-Front
```

**package.json 의존성:**
```json
{
  "dependencies": {
    "@xcrack/ui-shared": "file:../xCrack-UI-Shared",
    "next": "^14.0.0",
    "react": "^18.0.0",
    "recharts": "^2.10.0",
    "swr": "^2.2.0"
  }
}
```

**이동할 페이지:**
- `crack_front/app/liquidation/` → `app/dashboard/`
- `crack_front/app/page.tsx` 중 청산 부분 → `app/dashboard/page.tsx`
- `crack_front/lib/api.ts` 중 청산 API → `lib/api.ts`

**새로 작성:**
- `app/layout.tsx` - 청산 전용 레이아웃
- `app/dashboard/` - 청산 대시보드
- `app/positions/` - 포지션 목록
- `app/opportunities/` - 청산 기회
- `app/history/` - 청산 히스토리
- `Dockerfile`
- `docker-compose.yml`

---

### Phase 3: Sandwich 분리 (우선순위 2)

**3-1. 백엔드 (xCrack-Sandwich)**
```bash
cd blockbit/
cargo new xCrack-Sandwich
cd xCrack-Sandwich
```

**이동할 코드:**
- `src/strategies/sandwich/` → `src/`
- `src/mempool/` → `src/mempool/`
- `src/mev/` → `src/mev/`
- `docs/SANDWICH_*.md` → `docs/`
- `contracts/SandwichAttackStrategy.sol` → `contracts/`

**3-2. 프론트엔드 (xCrack-Sandwich-Front)**
```bash
cd blockbit/
npx create-next-app@latest xCrack-Sandwich-Front --typescript --tailwind --app
```

**이동할 페이지:**
- `crack_front/app/sandwich/` → `app/dashboard/`
- `crack_front/app/mempool/` → `app/mempool/`
- `crack_front/app/bundles/` → `app/bundles/`

---

### Phase 4: MicroArb 분리 (우선순위 3)

**4-1. 백엔드 (xCrack-MicroArb)**
```bash
cd blockbit/
cargo new xCrack-MicroArb
cd xCrack-MicroArb
```

**이동할 코드:**
- `src/strategies/micro_arbitrage/` → `src/`
- `src/strategies/cex_dex_arbitrage/` → `src/`
- `src/dex/` → `src/dex/`
- `src/exchange/` → `src/exchange/`
- `docs/MICRO_ARBITRAGE_*.md` → `docs/`
- `contracts/MicroArbitrageStrategy.sol` → `contracts/`

**4-2. 프론트엔드 (xCrack-MicroArb-Front)**
```bash
cd blockbit/
npx create-next-app@latest xCrack-MicroArb-Front --typescript --tailwind --app
```

**이동할 페이지:**
- `crack_front/app/micro-v2/` → `app/dashboard/`
- `crack_front/app/complex-arbitrage/` → `app/complex/`

---

### Phase 5: 통합 배포 (xCrack-Deploy)

**5-1. Docker Compose 파일 작성**

**docker-compose.liquidation.yml:**
```yaml
version: '3.8'

services:
  liquidation-backend:
    build: ../xCrack-Liquidation
    container_name: liquidation-backend
    ports:
      - "8081:8081"
    environment:
      - DATABASE_URL=postgresql://xcrack:password@postgres:5432/liquidation
      - REDIS_URL=redis://redis:6379
      - THEGRAPH_ENDPOINT=https://api.thegraph.com/...
      - RPC_URL=${RPC_URL}
    depends_on:
      - postgres
      - redis
    restart: unless-stopped

  liquidation-frontend:
    build: ../xCrack-Liquidation-Front
    container_name: liquidation-frontend
    ports:
      - "3001:3000"
    environment:
      - NEXT_PUBLIC_API_URL=http://localhost:8081
    depends_on:
      - liquidation-backend
    restart: unless-stopped

  postgres:
    image: postgres:16-alpine
    container_name: liquidation-postgres
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_USER=xcrack
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=liquidation
    volumes:
      - liquidation_postgres_data:/var/lib/postgresql/data
      - ../xCrack-Liquidation/migrations:/docker-entrypoint-initdb.d
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    container_name: liquidation-redis
    ports:
      - "6379:6379"
    volumes:
      - liquidation_redis_data:/data
    restart: unless-stopped

volumes:
  liquidation_postgres_data:
  liquidation_redis_data:
```

**docker-compose.all.yml:**
```yaml
version: '3.8'

services:
  # Liquidation
  liquidation-backend:
    build: ../xCrack-Liquidation
    ports:
      - "8081:8081"
    # ...

  liquidation-frontend:
    build: ../xCrack-Liquidation-Front
    ports:
      - "3001:3000"
    # ...

  # Sandwich
  sandwich-backend:
    build: ../xCrack-Sandwich
    ports:
      - "8082:8082"
    # ...

  sandwich-frontend:
    build: ../xCrack-Sandwich-Front
    ports:
      - "3002:3000"
    # ...

  # MicroArb
  microarb-backend:
    build: ../xCrack-MicroArb
    ports:
      - "8083:8083"
    # ...

  microarb-frontend:
    build: ../xCrack-MicroArb-Front
    ports:
      - "3003:3000"
    # ...

  # Shared services
  postgres:
    # ...

  redis:
    # ...
```

**5-2. Kubernetes 매니페스트 (선택사항)**

`k8s/liquidation/deployment.yaml`, `k8s/sandwich/deployment.yaml` 등

---

## 🔄 API 통신 구조

### Liquidation Frontend → Backend
```typescript
// xCrack-Liquidation-Front/lib/api.ts
const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8081';

export async function getPositions() {
  const res = await fetch(`${API_URL}/api/positions`);
  return res.json();
}

export async function getOpportunities() {
  const res = await fetch(`${API_URL}/api/opportunities`);
  return res.json();
}
```

### Liquidation Backend API
```rust
// xCrack-Liquidation/src/api.rs
use axum::{Router, routing::get};

pub fn api_routes() -> Router {
    Router::new()
        .route("/api/positions", get(get_positions))
        .route("/api/opportunities", get(get_opportunities))
        .route("/api/history", get(get_history))
        .route("/api/stats", get(get_stats))
}

async fn get_positions() -> Json<Vec<Position>> {
    // PostgreSQL에서 포지션 조회
}
```

---

## ✅ 체크리스트

### Phase 1: 공통 라이브러리
- [ ] xCrack-Shared 생성
- [ ] 공통 Rust 코드 이동
- [ ] xCrack-UI-Shared 생성
- [ ] 공통 React 컴포넌트 이동

### Phase 2: Liquidation
- [ ] xCrack-Liquidation 백엔드 생성
- [ ] 청산 관련 코드 이동
- [ ] REST API 구현
- [ ] Dockerfile 작성
- [ ] xCrack-Liquidation-Front 프론트엔드 생성
- [ ] 청산 페이지 이동
- [ ] API 연동
- [ ] docker-compose.liquidation.yml 작성
- [ ] 테스트 및 검증

### Phase 3: Sandwich
- [ ] xCrack-Sandwich 백엔드 생성
- [ ] 샌드위치 관련 코드 이동
- [ ] REST API 구현
- [ ] Dockerfile 작성
- [ ] xCrack-Sandwich-Front 프론트엔드 생성
- [ ] 샌드위치 페이지 이동
- [ ] API 연동
- [ ] docker-compose.sandwich.yml 작성
- [ ] 테스트 및 검증

### Phase 4: MicroArb
- [ ] xCrack-MicroArb 백엔드 생성
- [ ] 아비트러지 관련 코드 이동
- [ ] REST API 구현
- [ ] Dockerfile 작성
- [ ] xCrack-MicroArb-Front 프론트엔드 생성
- [ ] 아비트러지 페이지 이동
- [ ] API 연동
- [ ] docker-compose.microarb.yml 작성
- [ ] 테스트 및 검증

### Phase 5: 통합 배포
- [ ] xCrack-Deploy 디렉토리 생성
- [ ] docker-compose.all.yml 작성
- [ ] Kubernetes 매니페스트 작성 (선택)
- [ ] 통합 테스트
- [ ] 문서화 (각 프로젝트 README)

---

## 📊 마이그레이션 타임라인 (예상)

| Phase | 작업 | 예상 소요 시간 |
|-------|------|----------------|
| Phase 1 | 공통 라이브러리 | 1일 |
| Phase 2 | Liquidation 분리 | 2-3일 |
| Phase 3 | Sandwich 분리 | 2-3일 |
| Phase 4 | MicroArb 분리 | 2-3일 |
| Phase 5 | 통합 배포 | 1일 |
| **총계** | | **8-11일** |

---

## 🎯 마이그레이션 이후 장점

### 1. 독립 배포
```bash
# 청산 봇만 업데이트
cd xCrack-Deploy
docker-compose -f docker-compose.liquidation.yml up -d --build

# 샌드위치는 그대로 유지
```

### 2. 독립 스케일링
```bash
# 청산 봇만 스케일 아웃
docker-compose -f docker-compose.liquidation.yml up -d --scale liquidation-backend=3
```

### 3. 리스크 격리
- 샌드위치 봇 버그 → 청산 봇 안전
- 청산 봇 DB 장애 → 샌드위치 계속 동작

### 4. 전문화
- 각 팀이 각 전략에 집중
- 독립적인 개발 사이클
- 전략별 최적화

### 5. 간단한 CI/CD
```yaml
# .github/workflows/liquidation.yml
name: Liquidation CI/CD

on:
  push:
    paths:
      - 'xCrack-Liquidation/**'
      - 'xCrack-Liquidation-Front/**'

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Build Liquidation
        run: cd xCrack-Liquidation && cargo build --release
      - name: Deploy
        run: ./deploy-liquidation.sh
```

---

## 🚀 시작 명령어 요약

### 개별 전략 실행
```bash
# 청산만 실행
cd xCrack-Deploy
docker-compose -f docker-compose.liquidation.yml up -d

# 샌드위치만 실행
docker-compose -f docker-compose.sandwich.yml up -d

# 아비트러지만 실행
docker-compose -f docker-compose.microarb.yml up -d
```

### 모든 전략 실행
```bash
cd xCrack-Deploy
docker-compose -f docker-compose.all.yml up -d
```

### 프론트엔드 접속
- Liquidation: http://localhost:3001
- Sandwich: http://localhost:3002
- MicroArb: http://localhost:3003

---

## 📝 참고사항

1. **현재 xCrack 프로젝트는 삭제하지 않고 보관** (레거시 참조용)
2. **공통 라이브러리부터 시작** (xCrack-Shared, xCrack-UI-Shared)
3. **Liquidation부터 분리** (가장 완성도 높음)
4. **각 단계마다 테스트 및 검증**
5. **문서화 철저히** (각 프로젝트 README.md)
