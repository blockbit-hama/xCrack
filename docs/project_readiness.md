## **샌드위치 (Sandwich)**

- 코드 구현 (된 것/부분/미구현)
  - ✅ 프론트/백런 트랜잭션 인코딩: `encode_uniswap_v2_swap_exact_tokens` 적용. 파일: `src/strategies/sandwich_onchain.rs`, `src/utils/abi.rs`
  - ✅ 번들 생성·제출 경로: 프론트런/백런/approve TX 배열 구성, 가스전략/타깃블록 반영. 제출은 공통 `BundleManager`→`FlashbotsClient` 사용. 파일: `src/strategies/sandwich_onchain.rs`, `src/mev/bundle.rs`, `src/flashbots/*`
  - ✅ 토큰 사전 승인(approve): 번들 선두에 approve 삽입(간단 always-approve). 파일: `src/strategies/sandwich_onchain.rs`, `src/utils/abi.rs`
  - ✅ 가스 전략 기본값: `gas_multiplier`, `max_gas_price` 적용. 파일: `src/strategies/sandwich_onchain.rs`
  - ⚠️ 슬리피지/동시 실행 가드: amountOutMin 반영 완료, 동시 실행 상한은 스캐폴딩(제출 매니저 큐 연동 예정). 파일: `src/strategies/sandwich_onchain.rs`
  - ⛔ 드라이런 시뮬: 사전 시뮬/리허설 로직 없음
- 환경/기타 (된 것/부분/미구현)
  - ✅ RPC/WS 설정: `config/default.toml` 기반
  - ✅ 멤풀 입력: 상위에서 `analyze(&Transaction)`로 공급. 파일: `src/mempool/*`
  - ✅ 오라클 경로: `PriceAggregator(Chainlink+Uniswap TWAP)` 사용 가능. 파일: `src/oracle/*`
  - ⚠️ 수신자 주소: 현재 placeholder 주소 사용 → 운영 지갑 주소 주입 필요. 파일: `src/strategies/sandwich_onchain.rs`, `config/default.toml`
  - ⚠️ approve 최적화: allowance 검사 후 필요시에만 승인하도록 개선 권장
  - ⛔ 사전 리허설 파이프라인: 미구현

---

## **청산 (Liquidation)**

- 코드 구현 (된 것/부분/미구현)
  - ✅ 포지션 스캐닝/건강도 계산: Compound V3(Comet), MakerDAO(Vat/Dog) 온체인 질의로 health factor/파라미터 산출. 파일: `src/blockchain/contracts.rs`, `src/strategies/liquidation_onchain.rs`
  - ✅ 청산 콜 인코딩/실행 경로: `encode_compound_liquidation`, `encode_maker_bark` 등 ABI 인코딩과 호출 구성. 파일: `src/utils/abi.rs`, `src/strategies/liquidation_onchain.rs`
  - ✅ 플래시론 3-스텝 번들: Aave V3 `flashLoanSimple` + 파라미터(청산 calldata/DEX sell/minOut/spender) → `FlashLoanLiquidationReceiver` 콜백에서 실행. 파일: `src/utils/abi.rs`, `src/strategies/liquidation_onchain.rs`, `contracts/FlashLoanLiquidationReceiver.sol`
  - ✅ DEX 백업/견적: 0x/1inch 병행, 1inch는 API 키 헤더 주입(ENV). 파일: `src/strategies/liquidation_onchain.rs`
  - ✅ 가스 전략: 긴급도/경쟁도 반영한 동적 max_fee/max_priority 적용. 파일: `src/strategies/liquidation_onchain.rs`
  - ✅ 번들 제출 헬퍼: `submit_bundle_for_opportunity` → `FlashbotsClient` 제출. 파일: `src/strategies/liquidation_onchain.rs`, `src/flashbots/*`
  - ⚠️ 영속 저장: Redis 기반 기록 경로 존재(포지션/가격/이벤트), 스키마/메트릭은 최소. 파일: `src/storage/*`, `src/strategies/liquidation_onchain.rs`
  - ⛔ 사전 시뮬/리허설 자동화: Foundry/사설 시뮬 연동 미구현
- 환경/기타 (된 것/부분/미구현)
  - ⚠️ FlashLoan 리시버 배포/설정: `contracts/FlashLoanLiquidationReceiver.sol` 배포 후 주소를 `config/default.toml`의 `flashloan_receiver`에 설정 필요. 파일: `script/DeployReceiver.s.sol`, `script/SimulateReceiver.s.sol`, `config/default.toml`
  - ⚠️ Redis 연결: `REDIS_URL` 설정 및 인스턴스 실행 필요
  - ✅ Aave V3 풀 주소: 기본 상수 제공. 파일: `src/utils/abi.rs`
  - ⚠️ 1inch API 키: `ONEINCH_API_KEY` 환경 변수 주입 필요(코드에서 헤더 사용)
  - ⚠️ 주소/토큰/프로토콜 레지스트리 점검: WETH/USDC/DAI, Maker Vat/Dog, Compound Comet 주소 확인 필요
  - ⚠️ 지갑 가스 자금: 메인넷 ETH 펀딩 필요
  - ⛔ 사전 리허설/시뮬 파이프라인: 운영 전 점검용 자동 플로우 미구현

---

## **마이크로 아비트래지 (Micro Arbitrage)**

- 코드 구현 (된 것/부분/미구현)
  - ✅ Mock 실행 경로: 시뮬레이션 전용 경로로 전략·로깅 동작. 파일: `src/strategies/micro_arbitrage.rs`
  - ✅ 실집행 가드/웜업: API 키 검증, 가격 조회 워밍업, 연결 상태 확인. 파일: `src/strategies/micro_arbitrage.rs`
  - ✅ CEX 클라이언트 최소 실행: 주문/취소/상태/시세/잔고 구현(Binance/Coinbase). 파일: `src/exchange/client.rs`
  - ✅ 리스크/타임아웃/동시성 훅: 최소 이익/금액 한도·타임아웃·동시 실행 수 반영. 파일: `src/strategies/micro_arbitrage.rs`
  - ⚠️ 베스트 실행 고도화: 심화 주문 라우팅/슬리피지 방어 일부만 구현
  - ⛔ 고급 헤지/마켓메이킹: 미구현
- 환경/기타 (된 것/부분/미구현)
  - ✅ 최소 구동: `API_MODE=mock`로 즉시 실행 가능. 파일: `docs/MOCK_PRODUCTION_GUIDE.md`
  - ✅ 기본 거래쌍/거래소 설정: 설정값 로딩. 파일: `src/strategies/micro_arbitrage.rs`, `config/default.toml`
  - ⚠️ 실모드 전환: CEX API 키/시크릿/패스프레이즈 및 잔고 필요 (`BINANCE_API_KEY/SECRET`, `COINBASE_API_KEY/SECRET/PASSPHRASE`)
  - ⚠️ 레이턴시/동시성/타임아웃 튜닝: 운영 환경 값으로 보정 필요
  - ⛔ 운영 모니터링/알람: 대시보드/경보 미설정

---

## **크로스체인 아비트래지 (Cross-Chain Arbitrage)**

- 코드 구현 (된 것/부분/미구현)
  - ✅ 견적·만료 재검증: 만료 임박 시 재쿼리. 파일: `src/strategies/cross_chain_arbitrage.rs`
  - ✅ 백업 경로 1회 재시도: 모든 프로토콜 견적 취합 후 수익/비용 기준 재시도. 파일: `src/strategies/cross_chain_arbitrage.rs`
  - ✅ LI.FI 어댑터: `get_lifi_quote`(키 헤더/재시도), `execute_bridge`는 `transaction_request` 노출(RequiresAction). 파일: `src/bridges/lifi.rs`
  - ⚠️ 체인별 서명/브로드캐스트: `transaction_request`를 실제로 서명/전송하는 모듈 미구현
  - ⚠️ 상태 폴링/타임아웃 루프: `get_execution_status` 주기 폴링·타임아웃 처리 미구현
  - ⛔ 도착 체인 DEX 베스트 실행(자동 매도) 연계: 미구현
- 환경/기타 (된 것/부분/미구현)
  - ✅ 기본 토큰 레지스트리: WETH/USDC 등록. 파일: `src/strategies/cross_chain_arbitrage.rs`
  - ⚠️ 체인별 RPC_URL 준비: 소스/목적지 체인 모두 설정 필요
  - ⚠️ 브리지 API 키: `LIFI_API_KEY` 필요(요청 빈도/권한에 따라)
  - ⚠️ 가스 준비: 소스/목적지 체인 가스비용 확보 필요(브리지+도착 DEX 실행용)
  - ⚠️ 폴링 타임아웃/재시도 정책: 외부 설정값으로 분리 필요
  - ⛔ 다중 브리지 키/권한 관리 표준: 미수립
