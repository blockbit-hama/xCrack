# 프로젝트 전략별 실행 준비 현황

(레전드: ✅ 사실 / ⚠️ 부분 사실 / ⛔ 미구현)

---

## 🧭 실행 요약

- **샌드위치**: 프론트/백런 ABI 인코딩 및 번들 구성 완료. 멤풀 모니터·가스전략 적용·사전 승인 필요. 슬리피지/동시 실행 한도는 보강 중.
- **청산**: 리시버 배포/설정 + Redis + 견적 경로 준비 시 즉시 실행 가능. 가스 자금·제출 계층·시뮬 스크립트 확인.
- **마이크로/크로스체인**: Mock는 즉시 구동. 실모드는 **WS/RPC/브리지 API/키/잔고** 필요. 크로스체인은 견적 만료 재검증/백업 재시도까지 보강 완료.

---

## 🥪 샌드위치 (Sandwich)

- 데이터 소스/처리/저장/전략/실행/결과 관리 개요는 기존 문서와 동일

- [ ]  Task1 — RPC/WS 설정 — ✅
  - 설명: `OnChainSandwichStrategy`가 `BlockchainClient(RPC/WS)` 전제.
  - 파일: `src/strategies/sandwich_onchain.rs`, `src/config.rs`, `config/default.toml`

- [ ]  Task2 — 멤풀 모니터 연결 — ✅
  - 설명: 상위에서 pending tx를 `analyze(&Transaction)`로 공급.
  - 파일: `src/mempool/*`, `src/strategies/sandwich_onchain.rs`

- [ ]  Task3 — 오라클 경로 활성(가격 소스) — ✅
  - 설명: `PriceAggregator(Chainlink+Uniswap TWAP)` 적용.
  - 파일: `src/oracle/*`

- [ ]  Task4 — 프론트런/백런 트랜잭션 인코딩 — ✅
  - 설명: `encode_uniswap_v2_swap_exact_tokens`로 실제 ABI 인코딩 적용.
  - 파일: `src/strategies/sandwich_onchain.rs`

- [ ]  Task5 — 번들 생성·제출 경로(Flashbots/프라이빗 RPC) — ✅
  - 설명: 프론트런/백런 TX 배열 구성, 가스전략/타깃블록 반영. 제출은 공통 `BundleManager`→`FlashbotsClient` 경로 사용.
  - 파일: `src/strategies/sandwich_onchain.rs`, `src/core/bundle_manager.rs`, `src/flashbots/*`

- [ ]  Task6 — 토큰 사전 승인(approve) — ✅
  - 설명: 라우터에 WETH/스테이블 사전 승인 필요(인코더 준비됨).
  - 파일: `src/utils/abi.rs`

- [ ]  Task7 — 가스 전략 기본값 — ✅
  - 설명: `gas_multiplier`, `max_gas_price` 적용.
  - 파일: `src/strategies/sandwich_onchain.rs`

- [ ]  Task8 — 리스크 가드 — ⚠️
  - 설명: 최소 가치/수익 임계 검증 있음. 슬리피지(amountOutMin)·동시 실행 상한은 스캐폴딩 상태(다음 단계에서 amountOutMin 주입/큐 제한 연동 예정).

- [ ]  Task9 — 드라이런 시뮬레이션 — ⛔
  - 설명: 사전 시뮬/리허설 로직 없음.

메모: `to_recipient`(수신자 주소)와 amountOutMin 실제 주입, 라우터 승인 트랜잭션 삽입은 운영 전 점검 필요.

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

- [ ]  Task1 — Mock 모드 구동(API_MODE=mock) — ✅
- [ ]  Task2 — 토큰 레지스트리 기본(USDC/WETH) — ✅
- [ ]  Task3 — 실모드 RPC/브리지 API 준비 — ✅
  - 설명: `BridgeManager.get_best_quote/execute_bridge` 경로. LI.FI는 `LIFI_API_KEY` 지원.
- [ ]  Task4 — 최소 수익 임계값 & 가스 추정 — ⚠️
  - 설명: 고정 임계/단순 가스 추정 사용(설정 외부화·정교화 대상).
- [ ]  Task5 — 출발/도착 가스비용 — ✅
- [ ]  Task6 — 실패/만료/재시도/타임아웃 — ✅
  - 설명: 견적 만료 재검증(임박 시 1회 재조회), 1회 백업 경로 재시도, 실패 로그 표준화.
  - 파일: `src/strategies/cross_chain_arbitrage.rs`

메모: LI.FI `execute_bridge`는 최소선으로 `transaction_request`를 표면화하여 상위 실행기에서 실제 브로드캐스트 후 `get_execution_status`로 추적하는 흐름(RequiresAction)으로 구성.

---

## 공통 제출/환경

- 제출: `src/core/bundle_manager.rs` → `src/flashbots/client.rs`(시뮬/실제 전송)
- 환경 변수:
  - Flashbots: `config.flashbots.{relay_url, private_key, simulation_mode}`
  - 1inch: `ONEINCH_API_KEY`
  - LI.FI: `LIFI_API_KEY`
  - CEX: `BINANCE_API_KEY/SECRET`, `COINBASE_API_KEY/SECRET/PASSPHRASE`
  - 모의모드: `API_MODE=mock`
