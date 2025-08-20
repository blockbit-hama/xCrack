# 프로젝트 전략별 실행 준비 현황

## 🧭 실행 요약

- **샌드위치**: ABI 인코딩(프론트/백런)과 **번들 제출**(Flashbots/프라이빗 RPC) 연결 필요. 멤풀 모니터·가스전략·사전 승인·리스크 가드 보완 필요.
- **청산**: 리시버 배포/설정 + Redis + 견적 경로 정비 시 즉시 실행 가능. 가스 자금·제출 계층·시뮬 스크립트 확인.
- **마이크로/크로스체인**: Mock 모드는 즉시 구동. 실모드는 **WS/RPC/브리지 API/키/잔고** 필요. 리스크·타임아웃·재시도 정책 보강 권장.

---

## 🥪 샌드위치 (Sandwich)

- 데이터 소스
    - Mempool: 아직 블록에 포함되지 않은 “곧 일어날” 스왑 트랜잭션을 관찰합니다. 대개 큰 금액일수록 기회가 커집니다. 사용 경로: `src/mempool/*`.
    - DEX 풀 상태/이벤트: 유동성이 적은 풀일수록 가격이 크게 움직입니다. 사용 경로: `src/blockchain/contracts.rs`, `events.rs`.
    - 가스/네트워크: 혼잡한 시간에는 수수료를 더 내야 앞자리를 차지할 수 있습니다. 참고: `src/blockchain/rpc.rs`.
- 데이터 가져와서 처리하기
    1. “스왑 의심” 트랜잭션만 고릅니다(시그니처/토큰 주소 매칭).
    2. 예상 가격 이동(슬리피지)을 거칠게 계산합니다. 큰 스왑이면 가격이 크게 흔들립니다.
    3. 우리가 앞/뒤에 낄 때의 수익−가스비를 빠르게 추정합니다.
- 저장하기
    - 현재는 메모리 중심(짧게 들고 바로 판단). 향후 `src/storage/*`로 영속 저장(기회, 실패 원인, 성능 통계) 확대 예정.
- 전략 세우기
    - 규칙: “예상 이익 − (가스+위험 프리미엄) > 0”이면 후보. 피해자 보호·규정 준수 등 윤리·규제도 고려합니다.
    - 가스 전략: 경쟁 강도(동일 타깃을 노리는 봇 수 추정)에 따라 우선수수료를 올릴 수 있습니다.
- 실행하기
    - 번들을 구성해 Flashbots로 제출합니다. 앞 트랜잭션으로 가격 올리고, 피해자 스왑이 체결되면 뒤에서 되파는 구조. 현재 라우팅/백업번들은 기본형, 강화 예정.
- 결과 관리하기
    - 성공·실패, 실제 이익, 소요 가스, 경쟁에서 이긴 비율 등을 모읍니다(`src/core/performance_tracker.rs`). 실패 패턴을 학습해 가스곡선/필터를 조정합니다.

안전/리스크 메모: 샌드위치는 규제/윤리 이슈 논쟁이 있습니다. 네트워크 정책 변화를 주기적으로 확인하고, 법적 리스크를 피해야 합니다.

- [ ]  **Task1 — RPC/WS 설정** — ✅

  설명: `OnChainSandwichStrategy`가 `BlockchainClient(RPC/WS)`를 전제로 함.

  파일: `src/strategies/sandwich_onchain.rs`, `src/config.rs`, `config/default.toml` (`[blockchain.primary_network].rpc_url/ws_url`)

- [ ]  **Task2 — 멤풀 모니터 연결** — ✅

  설명: 실집행에는 **pending tx 스트림** 필요. 상위에서 `analyze(&Transaction)`로 공급.

  파일: `src/mempool/*`, `src/strategies/sandwich_onchain.rs` (분석 진입점)

- [ ]  **Task3 — 오라클 경로 활성(가격 소스)** — ✅

  설명: Chainlink / Uniswap TWAP을 `PriceAggregator`로 집계, RPC에서 가격 조회 확인.

  파일: `src/oracle/{chainlink,uniswap_twap}.rs`, `src/*/PriceAggregator`

- [ ]  **Task4 — 프론트런/백런 트랜잭션 인코딩** — ⛔ (현재 미완)

  설명: `create_front_run_transaction_onchain / create_back_run_transaction_onchain`에 **실제 ABI 인코딩** 필요.

  최소 구현: `UniswapV2.swapExactTokensForTokens / swapExactTokensForETH`

  파일: `src/strategies/sandwich_onchain.rs#(위 두 함수)`, 인코더: `src/utils/abi.rs (encode_uniswap_v2_*)`

- [ ]  **Task5 — 번들 생성·제출 경로(Flashbots/프라이빗 RPC)** — ⛔

  설명: `create_bundle`이 현재 **빈 배열**. Flashbots 또는 프라이빗 RPC로 제출 로직 연결.

  파일: `src/strategies/sandwich_onchain.rs#create_bundle`, `src/flashbots/*`

- [ ]  **Task6 — 토큰 사전 승인(approve)** — ✅

  설명: 라우터에 WETH/스테이블 **사전 approve 필요**.

  파일: `src/strategies/sandwich_onchain.rs`(승인 로직 없음), 인코더: `encode_erc20_approve`

- [ ]  **Task7 — 가스 전략 기본값** — ✅

  설명: `gas_multiplier`, `max_gas_price` 등 기본값 사용/점검.

  파일: `src/strategies/sandwich_onchain.rs`

- [ ]  **Task8 — 리스크 가드** — ⚠️

  설명: **최소 트랜잭션 가치(USD)/수익성 임계**는 있음. **슬리피지/동시 실행 한도**는 미흡.

  파일: `src/strategies/sandwich_onchain.rs`

- [ ]  **Task9 — 드라이런 시뮬레이션** — ⛔

  설명: 사전 시뮬/리허설 로직 없음.

  파일: N/A


---

## 💀 청산 (Liquidation)

- 데이터 소스
    - 대출 프로토콜 상태: Aave/Compound/MakerDAO의 담보·부채 정보.
    - 가격/오라클: Chainlink, Uniswap V3 TWAP 등을 상황에 따라 사용.
    - DEX 견적: 0x/1inch로 담보 매도 시 최적 경로 확인(백업 포함).
    - 네트워크/가스: 급할수록 가스비를 더 써서 먼저 집행해야 합니다.
- 데이터 가져와서 처리하기
    - Compound: `borrowBalanceOf/quoteCollateral`로 부채와 담보 환산가치를 산출, 여러 담보 후보 중 최대로 회수되는 조합을 고릅니다.
    - Maker: `Vat.urns/ilks`에서 `ink/art/rate/spot`를 읽어 건강도(담보가치/부채)를 계산합니다. 저하된 포지션을 찾습니다.
    - 동적 가스전략: “긴급도(곧 청산 가능?)”와 “경쟁도(상대 봇 많음?)”를 섞어 우선수수료를 조정합니다.
- 저장하기
    - 당장은 메모리 위주. 곧 `storage`에 포지션 스냅샷, 가격 히스토리, 경쟁자 프로파일, 청산 이벤트를 보관하도록 확장합니다.
- 전략 세우기
    - 원칙: 보상 − (가스 + 플래시론 수수료) > 0. 청산 금액을 너무 크게 잡으면 실패·슬리피지 위험이 커질 수 있어 적절히 제한합니다.
    - 경로 선택: 플래시론(3-스텝) vs 단순 경로(승인→청산→매도). 0x는 `allowanceTarget` 승인 필요 가능.
- 실행하기
    - 플래시론 경로: Aave V3 `flashLoanSimple` 1개 트랜잭션에 모든 스텝을 압축. 리시버 파라미터에 `minOut=원금+9bps`를 넣어 상환 안전장치.
    - 리시버 컨트랙트: `contracts/FlashLoanLiquidationReceiver.sol`이 청산→필요 시 매도→상환을 처리.
    - 비-플래시론 경로: `approve(부채토큰)`→`liquidation`→(옵션)`sell` 번들을 구성, 0x 경로면 담보 토큰도 `allowanceTarget`에 승인.
    - 구현 참고: `src/strategies/liquidation_onchain.rs`, `src/utils/abi.rs`, Foundry 스크립트(`script/DeployReceiver.s.sol`, `script/SimulateReceiver.s.sol`).
- 결과 관리하기
    - “얼마 벌었나 − 가스/수수료?”를 집계하고, 실패 원인을 기록(가격 급변, 경쟁 패배, 승인 누락 등). 데이터는 차후 재학습에 사용.

안전/리스크 메모: 플래시론은 상환이 한 틱이라도 부족하면 전체 롤백됩니다. `minOut` 체크와 DEX 슬리피지 한도를 반드시 설정해야 합니다.

- [ ]  **Task1 — 리시버 배포 & 설정** — ✅

  설명: `flashloan_receiver` 설정 시 플래시론 경로 활성.

  파일: `contracts/FlashLoanLiquidationReceiver.sol`, `script/DeployReceiver.s.sol`

  환경: `DEPLOYER_PK`, `AAVE_POOL`, `OWNER`

  설정: `config/default.toml [blockchain.primary_network].flashloan_receiver`

- [ ]  **Task2 — Redis 연결(REDIS_URL)** — ✅

  설명: 생성 시 `Storage::new` 호출, 포지션/가격/이벤트 기록 저장.

  파일: `src/storage/mod.rs`, `src/strategies/liquidation_onchain.rs::new`

  기본: `redis://127.0.0.1:6379`

- [ ]  **Task3 — Aave V3 풀 주소 확인** — ✅

  설명: 기본 상수 또는 프로토콜이 Aave면 해당 주소 사용.

  파일: `src/utils/abi.rs::contracts::AAVE_V3_POOL`, `src/strategies/liquidation_onchain.rs`

- [ ]  **Task4 — 견적 경로(0x/1inch) 점검** — ✅

  설명: 두 경로 모두 시도. **1inch API Key** 필요 시 헤더 추가.

  파일: `src/strategies/liquidation_onchain.rs::try_get_0x_quote / try_get_1inch_quote`

- [ ]  **Task5 — 토큰/프로토콜 주소 세트** — ✅

  설명: 설정 우선, 없으면 하드코드 폴백. Maker 분석은 `Vat.urns / ilks` 기반, `Dog` 포함.

  파일: `src/strategies/liquidation_onchain.rs`, `src/blockchain/contracts.rs (Vat 래퍼)`

- [ ]  **Task6 — 가스 자금 준비** — ✅

  설명: 지갑 펀딩 전제. 코드 내 **자동 펀딩 없음**.

  파일: N/A

- [ ]  **Task7 — 임계값/수수료(9bps) 반영** — ✅

  설명: `min_profit_eth`, 청산량 상·하한, **플래시론 9bps 비용 차감**.

  파일: `src/strategies/liquidation_onchain.rs`

- [ ]  **Task8 — 제출 경로 선택(번들/직접)** — ✅

  설명: 번들 반환 이후 제출 계층 필요(Flashbots/직접).

  파일: `src/mev/bundle.rs` (상위 제출자는 별도)

- [ ]  **Task9 — 리시버 파라미터 샌드박스** — ✅

  설명: 시뮬 스크립트 제공.

  파일: `script/SimulateReceiver.s.sol`


---

## ⚡ 마이크로 아비트래지 (Micro Arbitrage)

- 데이터 소스
    - DEX/CEX 시세·오더북·체결 피드(`src/exchange/*`). 초반에는 모의 데이터/인터페이스로 시작할 수 있습니다.
    - 모니터링/스케줄러: 정해진 주기로 심볼을 스캔하고, 기회 점수를 계산합니다.
- 데이터 가져와서 처리하기
    - 각 거래쌍에 대해 “현재 스프레드 − (수수료+예상 슬리피지+가스)”를 계산합니다.
    - 네트워크/거래소 지연(왕복 시간)을 고려해 실현 가능성을 평가합니다. 지연이 크면 가격이 바뀌어 기회가 사라집니다.
- 저장하기
    - 실행 로그, 평균 체결 시간, 성공률을 메모리에 보관하고 점차 영속 저장(예: Redis)으로 확장합니다.
- 전략 세우기
    - 수익이 양수인 경우만 소량 다회전으로 접근(리스크 분산). 주문 크기는 슬리피지와 체결 가능성을 고려해 보수적으로.
- 실행하기
    - 현재는 모의 실행기(`src/exchange/order_executor.rs`) 경로. 실제 거래소 API(키 관리/레이트리밋/리트라이)는 단계적 도입.
- 결과 관리하기
    - 종합 성과(승률·평균 수익·표준편차)를 추적하고, 심볼별·시간대별 성과를 시각화할 수 있도록 구조화합니다.

안전/리스크 메모: 수수료가 생각보다 커서 “한 번에 크게”보다는 “작게 여러 번”이 안전한 경우가 많습니다. 슬리피지 한도를 항상 설정하세요.

- [ ]  **Task1 — Mock 모드 구동(API_MODE=mock)** — ✅

  설명: 모의 실행 경로 존재(성공률 시뮬).

  파일: `src/strategies/micro_arbitrage.rs::execute_mock_arbitrage`

- [ ]  **Task2 — 거래쌍/거래소 활성화** — ✅

  설명: 설정 기반으로 **활성화된 exchange만 로드**.

  파일: `src/strategies/micro_arbitrage.rs::new` (`config.strategies.micro_arbitrage`)

- [ ]  **Task3 — 모니터→피드→전략 로그 파이프라인** — ✅

  설명: 오케스트레이터가 채널 연결 및 로그 출력.

  파일: `src/core/micro_arbitrage_orchestrator.rs`,

  `src/exchange/{monitor,price_feed_manager,order_executor}.rs`

- [ ]  **Task4 — 리스크 한도** — ✅

  설명: `min_profit % / USD`, 거래당·일별 한도 사용.

  파일: `src/strategies/micro_arbitrage.rs`

- [ ]  **Task5 — 실모드 API/잔고/WS** — ✅

  설명: 클라이언트 생성 시 ENV 키 요구, 잔고 체크. WS 프로바이더 필요(`Provider<Ws>`).

  파일: `src/strategies/micro_arbitrage.rs::create_exchange_client / execute_real_arbitrage`

  환경: `BINANCE_API_KEY/SECRET`, `COINBASE_API_KEY/SECRET/PASSPHRASE`

- [ ]  **Task6 — 타임아웃/동시 실행/레이턴시** — ✅

  설명: 모두 설정값으로 제어.

  파일: `src/strategies/micro_arbitrage.rs`


---

## 🌉 크로스체인 아비트래지 (Cross-chain Arbitrage)

- 데이터 소스
    - 브리지 견적/성능: LiFi, Stargate, Hop, Rubic, Synapse, Across, Multichain 등(`src/bridges/*`).
    - 체인별 가스비, 예상 체결 시간, 과거 성공률(내부 성능 트래커).
    - 원천/목적지 체인의 DEX 시세(필요 시 양쪽에서 환전 경로 포함).
- 데이터 가져와서 처리하기
    - `CrossChainArbitrageStrategy.scan_real_bridge_opportunities`에서 토큰·체인쌍을 조합해 견적을 모읍니다.
    - 순이익 추정: “목적지에서 실제 손에 쥘 금액 − (원천에서 준비 비용 + 두 체인의 가스비 + 브리지 수수료)”.
    - 시간 리스크: 빠른 브리지일수록 유리. 지연이 길면 가격이 바뀌어 이익이 사라질 수 있습니다.
- 저장하기
    - 현재는 메모리/간단 구조. 곧 브리지 성능 DB(체인×토큰×브리지별 평균 시간, 실패율, 스프레드)를 영속화하여 라우팅에 반영합니다.
- 전략 세우기
    - 라우팅 정책: “최저비용”, “속도-비용 균형”, “성공률 우선” 등 프로파일을 설정하고 상황에 맞게 선택합니다.
    - 견적 만료/재견적: 브리지 견적은 유효 시간이 짧습니다. 실행 직전에 재검증합니다.
- 실행하기
    - `bridge_manager.execute_bridge`로 라우팅된 경로를 호출합니다(초안). 실패 시 백업 경로로 즉시 전환하는 재시도 로직을 마련합니다.
    - 필요하면 출발/도착 체인에서 DEX 교환을 함께 실행(예: ETH→USDC로 바꿔 이동, 도착 후 다시 ETH로).
- 결과 관리하기
    - 체인·브리지별 성과(수익·시간·성공률)를 집계하고, 히스토리를 이용해 다음번 라우팅 점수에 반영합니다.

안전/리스크 메모: 브리지 실패·지연·중단 이슈에 대비해 “최소 보장 금액”과 타임아웃, 재시작 정책을 둡니다. 체인 재조직(reorg)이나 수수료 급등도 대비합니다.

- [ ]  **Task1 — Mock 모드 구동(API_MODE=mock)** — ✅

  설명: 초기화·스캔에서 mock 분기.

  파일: `src/strategies/cross_chain_arbitrage.rs::initialize / scan_opportunities`

  모의: `generate_mock_opportunities`, `execute_cross_chain_trade_mock`

- [ ]  **Task2 — 토큰 레지스트리 기본(USDC/WETH)** — ✅

  설명: 기본 등록 구현. 필요 체인 주소 보강.

  파일: `src/strategies/cross_chain_arbitrage.rs::register_default_tokens`

- [ ]  **Task3 — 실모드 RPC/브리지 API 준비** — ✅

  설명: 다체인 `rpc_url` + 브리지 견적/실행 API 사용 가능 여부 확인.

  파일: `src/strategies/cross_chain_arbitrage.rs`, `src/bridges/* (manager 및 브리지별 모듈)`

  인터페이스: `BridgeManager.get_best_quote / execute_bridge`

  환경: 브리지별 API Key/쿼터

- [ ]  **Task4 — 최소 수익 임계값 & 가스 추정** — ⚠️

  설명: 분석/검증에 **고정 임계(예: 0.2%)**·**가스 단순 추정** 사용. 별도 설정 항목은 미흡.

  파일: `src/strategies/cross_chain_arbitrage.rs::analyze / validate_opportunity`

- [ ]  **Task5 — 출발/도착 가스비용** — ✅

  설명: 실모드 전제. 코드 내 **자동 조달 없음**.

  파일: N/A

- [ ]  **Task6 — 실패/만료/재시도/타임아웃 전략** — ⚠️

  설명: 만료/성공여부 반영은 있으나, **체계적 재시도·대안 라우팅** 미흡.

  파일: `src/strategies/cross_chain_arbitrage.rs::execute_real_cross_chain_trade`