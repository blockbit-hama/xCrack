# 🦀 xCrack Rust MEV 서쳐

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

최고의 속도와 효율성을 위해 전체를 Rust로 구축한 고성능 MEV(Maximal Extractable Value) 서쳐 봇입니다.

## 🚀 특징

- **🔥 순수 Rust 성능**: 오버헤드 제로, 최고의 속도
- **📡 실시간 멤풀 모니터링**: WebSocket 기반 트랜잭션 감지
- **🎯 다중 전략**: 차익 거래, 샌드위치 공격, 청산
- **⚡ Flashbots 통합**: 네이티브 번들 제출 및 모니터링
- **🛡️ 안전 우선**: 내장된 리스크 관리 및 긴급 정지 기능
- **📊 포괄적인 모니터링**: 메트릭, 알림 및 성능 추적
- **🔧 높은 설정 유연성**: TOML 기반 설정 시스템

## 🏗️ 아키텍처

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   멤풀          │    │   전략          │    │   Flashbots     │
│   감시자        │───▶│   엔진          │───▶│   클라이언트    │
│                 │    │                 │    │                 │
│ • WebSocket     │    │ • 차익 거래     │    │ • 번들 제출     │
│ • 필터링        │    │ • 샌드위치      │    │ • 상태 확인     │
│ • 실시간        │    │ • 청산          │    │ • 모니터링      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 📦 설치

### 사전 요구 사항

- Rust 1.75+ ([Rust 설치](https://rustup.rs/))
- Git

### 소스에서 빌드

```bash
# 레포지토리 클론
git clone <repository-url>
cd xCrackRust
cd xCrack

# 릴리즈 모드로 빌드
cargo build --release

# 바이너리는 target/release/searcher 에서 찾을 수 있습니다
```

## ⚙️ 구성

1. **기본 설정 복사:**
```bash
cp config/default.toml config/production.toml
```

2. **설정 파일 수정:**
```toml
[network]
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
ws_url = "wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"

[flashbots]
private_key = "your_private_key_here"

[monitoring]
enable_discord_alerts = true
discord_webhook_url = "https://discord.com/api/webhooks/..."
```

3. **전략 구성:**
```toml
[strategies.arbitrage]
enabled = true
min_profit_threshold = "0.01"  # 최소 수익 0.01 ETH

[strategies.sandwich]
enabled = true
min_target_value = "0.5"       # 최소 타겟 가치 0.5 ETH
```

## 🚀 사용법

### 기본 사용법

```bash
# 기본 설정으로 실행
./target/release/searcher

# 사용자 설정으로 실행
./target/release/searcher --config config/production.toml

# 시뮬레이션 모드로 실행 (실제 번들 제출 안 함)
./target/release/searcher --simulation

# 디버그 로깅 활성화
./target/release/searcher --log-level debug
```

### 커맨드 라인 옵션

```bash
xcrack-rust-searcher [OPTIONS]

OPTIONS:
    -c, --config <FILE>     설정 파일 경로 [기본값: config/default.toml]
    -l, --log-level <LEVEL> 로그 레벨 [기본값: info]
        --simulation        시뮬레이션 모드 (실제 번들을 제출하지 않음)
        --dev               개발 모드 활성화
    -h, --help              도움말 정보 출력
    -V, --version           버전 정보 출력
```

## 🎯 전략

### 1. 차익 거래 전략

DEX 간의 가격 차이를 감지하고 수익성 있는 거래를 실행합니다.

**구성:**
```toml
[strategies.arbitrage]
enabled = true
min_profit_threshold = "0.01"     # ETH 단위 최소 수익
max_trade_size = "10.0"           # ETH 단위 최대 거래 규모
max_price_impact = 0.05           # 최대 5% 가격 영향
supported_dexes = ["uniswap_v2", "sushiswap", "uniswap_v3"]
```

**작동 방식:**
1. 토큰 가격에 영향을 미치는 트랜잭션 모니터링
2. 여러 DEX에 걸쳐 가격 계산
3. 차익 거래 기회 식별
4. 수익성이 있을 때 거래 실행

### 2. 샌드위치 전략

MEV를 추출하기 위해 대규모 트랜잭션을 프론트런 및 백런합니다.

**구성:**
```toml
[strategies.sandwich]
enabled = true
min_target_value = "0.5"          # 최소 피해 트랜잭션 가치
max_slippage = 0.03               # 유발할 최대 슬리피지 3%
max_frontrun_size = "5.0"         # 최대 프론트런 규모
```

**작동 방식:**
1. 대기 중인 대규모 스왑 감지
2. 최적의 프론트런/백런 양 계산
3. Flashbots에 샌드위치 번들 제출

### 3. 청산 전략

청산 기회를 위해 DeFi 프로토콜을 모니터링합니다.

**구성:**
```toml
[strategies.liquidation]
enabled = true
protocols = ["aave", "compound", "makerdao"]
min_health_factor = 1.05          # 이 임계값 이하에서 청산
max_liquidation_amount = "50.0"   # 최대 청산 규모
```

## 🛡️ 안전 기능

### 리스크 관리
- **일일 가스 한도**: 과도한 가스 소비 방지
- **포지션 크기 한도**: 최대 거래 규모 제어
- **긴급 정지**: 큰 손실 발생 시 자동 종료
- **상태 모니터링**: 지속적인 시스템 상태 확인

### 구성 예시
```toml
[safety]
max_concurrent_bundles = 5        # 최대 활성 번들 수
max_daily_gas_spend = "1.0"       # 일일 가스 한도 1 ETH
emergency_stop_loss = "0.1"       # 0.1 ETH 손실 시 정지
max_position_size = "10.0"        # 최대 포지션 크기 10 ETH
enable_emergency_stop = true
```

## 📊 모니터링

### 메트릭
- 초당 처리된 트랜잭션
- 발견된 기회 및 전환율
- 번들 제출 및 포함률
- 손익 추적
- 가스 효율성 메트릭

### 알림
- **Discord**: 웹훅을 통한 실시간 알림
- **Telegram**: 모바일 알림
- **수익 보고서**: 일일/주간 수익 요약

### 알림 구성 예시
```toml
[monitoring]
enable_discord_alerts = true
discord_webhook_url = "https://discord.com/api/webhooks/..."
enable_telegram_alerts = true
telegram_bot_token = "your_bot_token"
telegram_chat_id = "your_chat_id"
profit_report_interval = "0 8 * * *"  # 매일 오전 8시
```

## 🔧 개발

### 개발용 빌드

```bash
# 디버그 정보와 함께 빌드
cargo build

# 테스트 실행
cargo test

# 로깅과 함께 실행
RUST_LOG=debug cargo run

# 코드 포맷팅
cargo fmt

# 코드 린트
cargo clippy
```

### 프로젝트 구조

```
src/
├── main.rs                 # 애플리케이션 진입점
├── config.rs              # 설정 관리
├── types.rs               # 핵심 데이터 타입
├── core/                  # 핵심 서쳐 엔진
├── mempool/               # 멤풀 모니터링
├── strategies/            # 거래 전략
├── flashbots/             # Flashbots 통합
├── monitoring/            # 메트릭 및 알림
└── utils/                 # 유틸리티 함수
```

## 📈 성능

### 벤치마크
- **트랜잭션 분석**: 평균 <10ms
- **번들 생성**: 평균 <50ms
- **메모리 사용량**: 일반적 <100MB
- **CPU 사용량**: 최신 하드웨어에서 <5%

### 최적화 팁
1. **동시 분석 증가**: `max_concurrent_analysis = 20`
2. **필터 최적화**: `mempool_filter_min_value` 감소
3. **메트릭 활성화**: 성능 병목 현상 모니터링
4. **SSD 스토리지 사용**: 더 나은 I/O 성능을 위해

## 🚨 문제 해결

### 일반적인 문제

**연결 문제:**
```bash
# RPC 연결 확인
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  YOUR_RPC_URL
```

**낮은 성능:**
- `max_concurrent_analysis` 증가
- `mempool_filter_min_value` 낮추기
- 네트워크 지연 시간 확인

**번들 실패:**
- 가스 가격이 경쟁력 있는지 확인
- 지갑 잔액 확인
- Flashbots 상태 모니터링

### 로그

```bash
# 실시간 로그 보기
tail -f logs/searcher.log

# 오류 검색
grep ERROR logs/searcher.log

# 전략별 필터링
grep "Arbitrage" logs/searcher.log
```

## 🔐 보안

### 모범 사례
1. **개인 키 보안**: 환경 변수 또는 보안 키 관리 사용
2. **네트워크 보안**: HTTPS/WSS 엔드포인트만 사용
3. **접근 제어**: API 접근 및 모니터링 엔드포인트 제한
4. **정기적인 업데이트**: 의존성 최신 상태 유지

### 환경 변수
```bash
# 안전한 개인 키 관리
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="your_rpc_url_here"

# 환경 변수로 실행
./target/release/searcher
```

## 📊 메트릭 대시보드

### Prometheus 메트릭
서쳐는 9090 포트(설정 가능)에서 메트릭을 노출합니다:

```
# HELP mev_transactions_processed_total 처리된 총 트랜잭션
# TYPE mev_transactions_processed_total counter
mev_transactions_processed_total 1234

# HELP mev_opportunities_found_total 발견된 총 기회
# TYPE mev_opportunities_found_total counter
mev_opportunities_found_total 56

# HELP mev_bundles_submitted_total 제출된 총 번들
# TYPE mev_bundles_submitted_total counter
mev_bundles_submitted_total 23

# HELP mev_profit_eth_total ETH 단위 총 수익
# TYPE mev_profit_eth_total gauge
mev_profit_eth_total 1.23
```

### Grafana 대시보드

시각화를 위해 제공된 Grafana 대시보드(`grafana/dashboard.json`)를 가져옵니다.

## 💰 수익성

### 예상 수익
- **차익 거래**: 기회당 0.5-2%
- **샌드위치**: 피해 트랜잭션당 0.1-1%
- **청산**: 5-15% 청산 보너스

### 비용 분석
- **가스 비용**: 번들당 약 0.001-0.01 ETH
- **인프라**: 월 약 $50-200
- **기회 비용**: 지갑에 묶인 자본

### 손익분기점 분석
```
일일 예상 수익: 0.1-1 ETH
일일 가스 비용: 0.01-0.1 ETH
순수익: 0.09-0.9 ETH/일
월간 ROI: 10-50%
```

## 🤝 기여하기

기여를 환영합니다! 가이드라인은 [CONTRIBUTING.md](CONTRIBUTING.md)를 참조하세요.

### 개발 설정
1. 레포지토리 포크
2. 기능 브랜치 생성
3. 새로운 기능에 대한 테스트 작성
4. 모든 테스트 통과 확인
5. 풀 리퀘스트 제출

### 코드 스타일
- 포맷팅에는 `cargo fmt` 사용
- Rust 명명 규칙 준수
- 포괄적인 문서 추가
- 새로운 함수에 대한 단위 테스트 작성

## 📄 라이선스

이 프로젝트는 MIT 라이선스에 따라 라이선스가 부여됩니다. 자세한 내용은 [LICENSE](LICENSE) 파일을 참조하세요.

## ⚠️ 면책 조항

이 소프트웨어는 교육 및 연구 목적으로 제작되었습니다. MEV 추출에는 금융적 위험이 따릅니다. 사용자는 다음에 대한 책임이 있습니다:

- 관련된 위험 이해
- 관련 법률 및 규정 준수
- 적절한 키 관리 및 보안
- 메인넷 배포 전 철저한 테스트

## 🔗 관련 자료

- [Flashbots 문서](https://docs.flashbots.net/)
- [MEV 리서치](https://github.com/flashbots/mev-research)
- [Rust 문서](https://doc.rust-lang.org/)
- [Ethers-rs 문서](https://docs.rs/ethers/)

## 📞 지원

- **이슈**: [GitHub 이슈](https://github.com/your-repo/issues)
- **토론**: [GitHub 토론](https://github.com/your-repo/discussions)
- **Discord**: [MEV 커뮤니티](https://discord.gg/flashbots)

---

**⚡ 최고의 성능을 위해 Rust로 제작되었습니다! ⚡**