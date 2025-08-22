## xCrack 프론트·백엔드 실행 튜토리얼

이 문서는 로컬에서 xCrack 백엔드(API/메트릭 서버)와 프론트엔드(Next.js 대시보드)를 실행하는 방법과 필요한 환경설정을 안내합니다. 최근 추가된 기능(번들 상세/리스트, 전략 파라미터 편집, 로그 SSE, 리포트/상태/전략 토글/통계 초기화 등) 기준으로 업데이트되었습니다.

### 0) 선행 준비물
- Rust toolchain (stable), Cargo
- Node.js 18+ (권장 20+), npm
- (옵션) Redis 서버: 청산 전략 영속 저장용
- (옵션) Foundry(Anvil/forge): 로컬 시뮬/컨트랙트 배포 테스트

---

### 1) 환경 변수(.env)와 설정(config)
백엔드 실행 전 다음 환경변수를 필요에 따라 설정합니다. 실제 모드가 아니라면 `API_MODE=mock`로 간단히 시작할 수 있습니다.

필수/권장 ENV (실모드 기준):
- PRIVATE_KEY: Flashbots 또는 트랜잭션 서명용 개인키(절대 노출 금지)
- ETH_RPC_URL: 메인넷 RPC URL
- ETH_WS_URL: 메인넷 WS URL
- FLASHBOTS_RELAY_URL: Flashbots 릴레이 URL (기본값 존재)
- ONEINCH_API_KEY: 1inch API 키 (청산 DEX 견적용)
- LIFI_API_KEY: LI.FI API 키 (크로스체인 브리지 견적용)
- REDIS_URL: Redis 연결 문자열(예: redis://127.0.0.1:6379)
- API_MODE: mock 또는 real (mock 권장으로 시작)
- XCRACK_CONFIG_PATH: 설정 저장 경로(기본: config/default.toml, 전략 파라미터 저장 시 사용)

설정 파일(config/default.toml) 주요 포트/주소:
- monitoring.metrics_port: 메트릭 서버 포트 (기본 9090)
- monitoring.api_port: 퍼블릭 API 포트 (기본 8080)
- blockchain.primary_network.flashloan_receiver: 플래시론 리시버 배포 주소(청산 전략 실전 시 필요)

간단 샘플(.env):
```bash
API_MODE=mock
ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/XXXX
ETH_WS_URL=wss://eth-mainnet.g.alchemy.com/v2/XXXX
PRIVATE_KEY=0x0000000000000000000000000000000000000000000000000000000000000001
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
# 선택: ONEINCH_API_KEY=...
# 선택: LIFI_API_KEY=...
# 선택: REDIS_URL=redis://127.0.0.1:6379
```

---

### 2) 백엔드 실행 (Rust, Axum API 포함)
프로젝트 루트에서:
```bash
# 1) 빌드 확인
cargo check

# 2) 실행 (기본 설정)
cargo run
```
확인 엔드포인트:
- API 헬스: `http://localhost:8080/api/health` → `{ "ok": true }`
- API 상태: `http://localhost:8080/api/status` → 실행/메트릭 요약
- 메트릭: `http://localhost:9090/metrics`
- 메트릭 상태: `http://localhost:9090/status`
- 번들 요약/리스트: `GET /api/bundles`
- 번들 상세: `GET /api/bundles/:id`
- 전략 상태: `GET /api/strategies` / `POST /api/strategies/toggle`
- 전략 상세 통계: `GET /api/strategies/stats`
- 성능 리포트: `GET /api/report`
- 로그 SSE 스트림: `GET /api/stream/logs`
- 설정/액션: `GET/POST /api/settings`
- 전략 파라미터: `GET/POST /api/strategies/params` (POST는 저장 후 재시작 필요)

전략 선택/모드 예시:
```bash
# 샌드위치+청산만 활성화, 시뮬레이션 모드
a
cargo run -- --strategies sandwich,liquidation --simulation

# 크로스체인 모의 단독 실행
cargo run -- --strategies cross_chain
```

주의:
- 실모드(real) 사용 시 RPC/WS/키가 유효해야 하며, 지갑에 가스가 필요합니다.
- 청산 전략 실전 시 `contracts/FlashLoanLiquidationReceiver.sol` 배포 주소를 `config/default.toml`에 설정해야 합니다.

---

### 3) 프론트엔드 실행 (crack_front, Next.js)
설치/개발 서버:
```bash
cd crack_front
npm install

# 개발 모드
npm run dev
# → http://localhost:3000 접속
```

프로덕션 빌드/실행:
```bash
npm run build
npm start
# → http://localhost:3000
```

백엔드 API 주소 설정:
- 환경변수 `NEXT_PUBLIC_BACKEND_URL`로 백엔드 베이스 URL 지정(기본 `http://localhost:8080`).
- 예) `.env.local`:
```bash
NEXT_PUBLIC_BACKEND_URL=http://localhost:8080
```

동작 설명:
- 홈(/): `/api/status`, `/api/bundles`, `/api/report`를 호출해 상태/요약/리포트 카드와 최근 번들을 표시합니다.
- 전략(/strategies): `/api/strategies`, `/api/strategies/stats`를 주기적으로 폴링해 ON/OFF 토글 및 메트릭 노출.
- 전략 상세(/strategies/[key]): 최근 번들 스파크라인 및 상세 메트릭.
- 번들(/bundles): 요약과 최근 목록. 각 행은 상세로 링크.
- 번들 상세(/bundles/[id]): 번들 메타/세부 JSON 표시.
- 로그(/logs): `/api/stream/logs` SSE 구독, 레벨/검색/일시정지/초기화.
- 설정(/settings): 포트, 전략 상태, 액션 버튼(통계 초기화/알림 확인), 전략 파라미터 간단 편집(Sandwich/Liquidation/Micro 일부 키 저장).

---

### 4) Redis 및 선택적 컴포넌트
- Redis(선택): 청산 포지션/이벤트/가격 기록에 사용. 로컬 실행:
```bash
# 예시 (Docker)
docker run -p 6379:6379 --name xr-redis -d redis:7
```
- Foundry(선택): 플래시론 리시버 배포/시뮬.
```bash
forge install
forge build
# 배포/시뮬 스크립트는 script/ 폴더 참조
```

---

### 5) 트러블슈팅
- 프론트 빌드 시 `fetch failed`:
  - 백엔드가 꺼져 있어도 기본값으로 렌더되도록 처리되어 있으나, 
    `NEXT_PUBLIC_BACKEND_URL` 확인 또는 개발 모드(`npm run dev`)를 권장합니다.
- 백엔드 설정 검증 실패:
  - `cargo run` 시 Config validate 에러가 나면 `config/default.toml`과 `.env`를 재확인하세요.
- 포트 충돌:
  - `monitoring.metrics_port`(9090), `monitoring.api_port`(8080), Next.js(3000) 충돌 시 포트 변경 후 재실행.
- 파라미터 저장 반영 안됨:
  - `/api/strategies/params` POST는 설정 파일에 저장만 합니다. 적용에는 프로세스 재시작이 필요합니다. 배포 환경에서는 Docker 재시작 훅 또는 관리자 전용 재시작 엔드포인트를 별도로 구성하세요.

---

### 6) 보안/운영 권장
- UI는 기본 읽기 전용으로 시작하고, 민감 제어 액션은 서버 서명·RBAC·IP 제한 도입 후 개방하세요.
- 개인키/시크릿은 프론트에 절대 노출하지 말고 서버/시크릿 매니저로 관리하세요.
- 실운영은 역프록시(Nginx/Caddy) 뒤, 사설 네트워크 또는 VPN 환경에서 운용을 권장합니다.
- CORS는 개발 중 Any 허용이나, 운영에서는 도메인 화이트리스트로 제한하세요.
- API 인증 토큰(헤더) 기반 보호 도입을 권장합니다.
