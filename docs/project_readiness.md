# 프로젝트 전략별 실행 준비 현황

(레전드: ✅ 사실 / ⚠️ 부분 사실 / ⛔ 미구현)

---

## 🧭 실행 요약

- **샌드위치**: 프론트/백런 ABI 인코딩 및 번들 구성 완료. 멤풀 모니터·가스전략 적용·사전 승인 필요. 슬리피지/동시 실행 한도는 보강 중.
- **청산**: 리시버 배포/설정 + Redis + 견적 경로 준비 시 즉시 실행 가능. 가스 자금·제출 계층·시뮬 스크립트 확인.
- **마이크로/크로스체인**: Mock는 즉시 구동. 실모드는 **WS/RPC/브리지 API/키/잔고** 필요. 크로스체인은 견적 만료 재검증/백업 재시도까지 보강 완료.

---

## 🥪 샌드위치 (Sandwich)

- 코드 구현 (된 것/부분/미구현)
  - ✅ 프론트/백런 트랜잭션 인코딩: `encode_uniswap_v2_swap_exact_tokens` 적용. 파일: `src/strategies/sandwich_onchain.rs`
  - ✅ 번들 생성·제출 경로: 프론트런/백런/approve TX 배열 구성, 가스전략/타깃블록 반영. 제출은 공통 `BundleManager`→`FlashbotsClient` 사용
  - ✅ 토큰 사전 승인(approve): 번들 선두에 approve 삽입(간단 always-approve)
  - ✅ 가스 전략 기본값: `gas_multiplier`, `max_gas_price` 적용
  - ⚠️ 슬리피지/동시 실행 가드: amountOutMin 반영 완료, 동시 실행 상한은 스캐폴딩(제출 매니저 큐 연동 예정)
  - ⛔ 드라이런 시뮬: 사전 시뮬/리허설 로직 없음

- 환경/기타 (된 것/부분/미구현)
  - ✅ RPC/WS 설정: `config/default.toml` 기반
  - ✅ 멤풀 입력: 상위에서 `analyze(&Transaction)`로 공급
  - ✅ 오라클 경로: `PriceAggregator(Chainlink+Uniswap TWAP)`
  - ⚠️ 수신자 주소: 현재 placeholder 주소 사용 → 운영 지갑 주소 주입 필요
  - ⚠️ approve 최적화: allowance 검사 후 필요시에만 승인하도록 개선 권장
  - ⛔ 사전 리허설 파이프라인: 미구현

---

## 💀 청산 (Liquidation)

- [ ]  Task1 — 리시버 배포 & 설정 — ✅
  - 설명: `flashloan_receiver` 설정 시 플래시론 경로 활성.
  - 파일/환경: `contracts/FlashLoanLiquidationReceiver.sol`, `script/DeployReceiver.s.sol` / `DEPLOYER_PK`, `AAVE_POOL`, `OWNER`, `config/default.toml`

- [ ]  Task2 — Redis 연결(REDIS_URL) — ✅
  - 설명: 포지션/가격/이벤트 기록.
  - 파일: `src/storage/mod.rs`, `src/strategies/liquidation_onchain.rs::new`

- [ ]  Task3 — Aave V3 풀 주소 확인 — ✅
  - 설명: 기본 상수 또는 Aave 프로토콜 주소 사용.
  - 파일: `src/utils/abi.rs::contracts::AAVE_V3_POOL`, `src/strategies/liquidation_onchain.rs`

- [ ]  Task4 — 견적 경로(0x/1inch) 점검 — ✅
  - 설명: 1inch는 `ONEINCH_API_KEY` 헤더 자동 주입(없으면 폴백).
  - 파일: `src/strategies/liquidation_onchain.rs::try_get_1inch_quote`

- [ ]  Task5 — 토큰/프로토콜 주소 세트 — ✅
  - 설명: 설정 우선, 폴백 하드코드(Maker Vat/Dog 포함).

- [ ]  Task6 — 가스 자금 준비 — ✅

- [ ]  Task7 — 임계값/수수료(9bps) 반영 — ✅
  - 설명: `min_profit_eth`/청산량 상·하한/플래시론 9bps 비용.

- [ ]  Task8 — 제출 경로 선택(번들/직접) — ✅
  - 설명: 번들 생성 후 공통 제출 계층 사용.
  - 파일: `src/core/bundle_manager.rs`, `src/flashbots/client.rs`

- [ ]  Task9 — 리시버 파라미터 샌드박스 — ✅
  - 파일: `script/SimulateReceiver.s.sol`

메모: 플래시론 경로는 리시버 배포·설정이 필수. Redis 미연결 시 런타임 에러 가능.

---

## ⚡ 마이크로 아비트래지 (Micro Arbitrage)

- [ ]  Task1 — Mock 모드 구동(API_MODE=mock) — ✅
- [ ]  Task2 — 거래쌍/거래소 활성화 — ✅
- [ ]  Task3 — 모니터→피드→전략 파이프라인 — ✅
- [ ]  Task4 — 리스크 한도 — ✅
- [ ]  Task5 — 실모드 API/잔고/WS — ✅
  - 설명: 실행 전 `BINANCE_API_KEY/SECRET`, `COINBASE_API_KEY/SECRET/PASSPHRASE` 확인, 연결 워밍업 후 주문 실행.
  - 파일: `src/strategies/micro_arbitrage.rs`, `src/exchange/client.rs`
- [ ]  Task6 — 타임아웃/동시 실행/레이턴시 — ✅

메모: 실거래 전 잔고/심볼 표기(특히 Coinbase `-USD`) 확인 필요.

---

## 🌉 크로스체인 아비트래지 (Cross-chain Arbitrage)

- 코드 구현 (된 것/부분/미구현)
  - ✅ 브리지 견적/라우팅: `BridgeManager.get_best_quote/execute_bridge`
  - ✅ 실패/만료/재시도: 견적 만료 재검증(임박 시 1회 재조회), 1회 백업 경로 재시도, 표준화 실패 로그. 파일: `src/strategies/cross_chain_arbitrage.rs`
  - ✅ LI.FI 최소 실행: `execute_bridge`가 `route_data.transaction_request`를 표면화(RequiresAction)하여 상위 전송기로 연결. 파일: `src/bridges/lifi.rs`
  - ⚠️ 완전 자동 실행기: 체인별 signer로 `transaction_request` 서명/브로드캐스트 + 상태 폴링 루프는 후속 구현 대상
  - ⚠️ 최소 수익 임계/가스 추정: 고정 임계/단순 추정(설정 외부화·정교화 필요)

- 환경/기타 (된 것/부분/미구현)
  - ✅ Mock 모드 구동: `API_MODE=mock`
  - ✅ 토큰 레지스트리 기본(USDC/WETH)
  - ✅ 브리지 API 키: `LIFI_API_KEY` 지원
  - ✅ 출발/도착 가스비용: 실모드 전제(자동 리퓨얼은 후속)
  - ⚠️ 상태 폴링 운영 룰: 타임아웃/재시작 정책 외부화 필요

---

## 공통 제출/환경

- 제출: `src/core/bundle_manager.rs` → `src/flashbots/client.rs`(시뮬/실제 전송)
- 환경 변수:
  - Flashbots: `config.flashbots.{relay_url, private_key, simulation_mode}`
  - 1inch: `ONEINCH_API_KEY`
  - LI.FI: `LIFI_API_KEY`
  - CEX: `BINANCE_API_KEY/SECRET`, `COINBASE_API_KEY/SECRET/PASSPHRASE`
  - 모의모드: `API_MODE=mock`
