# xCrack MEV 서쳐 아키텍처 문서

## 개요

xCrack은 Rust로 구축된 고성능 MEV(Maximum Extractable Value) 서쳐로, 실시간 멤풀 모니터링과 플래시론 기반 차익거래를 통해 최대한의 수익을 추출하는 시스템입니다.

## 전체 아키텍처

### 시스템 구성도

```
┌─────────────────────────────────────────────────────────────────┐
│                    xCrack MEV Searcher                        │
├─────────────────────────────────────────────────────────────────┤
│  Frontend (Next.js)                                           │
│  - 실시간 대시보드                                              │
│  - 전략별 모니터링                                              │
│  - 플래시론 관리                                               │
│  - 리스크 분석                                                 │
└─────────────────┬───────────────────────────────────────────────┘
                  │ HTTP API (Port 8080)
┌─────────────────▼───────────────────────────────────────────────┐
│                Backend (Rust)                                 │
├─────────────────────────────────────────────────────────────────┤
│  Core Components:                                              │
│  ┌───────────────┐ ┌──────────────┐ ┌─────────────────┐       │
│  │ SearcherCore  │ │ MempoolMon.  │ │ StrategyManager │       │
│  │ - 메인 루프   │ │ - 트랜잭션   │ │ - 4개 전략 관리  │       │
│  │ - 기회 검증   │ │   감시       │ │ - 동적 활성화    │       │
│  │ - 번들 생성   │ │ - 가스 분석  │ │ - 성과 추적     │       │
│  └───────────────┘ └──────────────┘ └─────────────────┘       │
│                                                               │
│  ┌───────────────┐ ┌──────────────┐ ┌─────────────────┐       │
│  │ BundleManager │ │ ExchangeMon. │ │ FlashbotClient  │       │
│  │ - 번들 최적화 │ │ - DEX 모니터 │ │ - 번들 제출     │       │
│  │ - 가스 경쟁   │ │ - 가격 피드  │ │ - MEV-Boost     │       │
│  │ - 시뮬레이션  │ │ - 지연 추적  │ │ - 수수료 최적화  │       │
│  └───────────────┘ └──────────────┘ └─────────────────┘       │
└─────────────────┬───────────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────────┐
│                External Systems                               │
├─────────────────────────────────────────────────────────────────┤
│  Blockchain Infrastructure:                                   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐              │
│  │ Ethereum    │ │ Flashbots   │ │ DEX/CEX     │              │
│  │ Mainnet     │ │ Relay       │ │ APIs        │              │
│  └─────────────┘ └─────────────┘ └─────────────┘              │
│                                                               │
│  Flashloan Providers:                                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐              │
│  │ Aave V3     │ │ Balancer    │ │ Uniswap V3  │              │
│  │ Pool        │ │ Vault       │ │ Flash       │              │
│  └─────────────┘ └─────────────┘ └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

## 핵심 컴포넌트

### 1. SearcherCore (메인 오케스트레이터)

**역할**: 전체 시스템의 중앙 제어 컴포넌트
```rust
pub struct SearcherCore {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    strategy_manager: StrategyManager,
    bundle_manager: BundleManager,
    mempool_monitor: CoreMempoolMonitor,
    micro_arbitrage_orchestrator: MicroArbitrageOrchestrator,
}
```

**주요 기능**:
- 5단계 파이프라인 관리: 감지→분석→생성→번들링→실행
- 전략별 기회 라우팅
- 성능 모니터링 및 최적화
- 리스크 관리

### 2. StrategyManager (전략 관리자)

**지원 전략**:
1. **Sandwich Attack** - DEX 거래 샌드위치
2. **Liquidation** - 레버리지 포지션 청산
3. **Micro Arbitrage** - 크로스 DEX 차익거래
4. **Cross Chain Arbitrage** - 체인간 차익거래

**동적 활성화**:
- 시장 조건에 따른 전략 on/off
- 수익성 기반 우선순위 조정
- 가스 가격 최적화

### 3. MempoolMonitor (멤풀 모니터)

**실시간 트랜잭션 분석**:
- 고가치 DEX 거래 감지
- 가스 가격 경쟁 분석
- MEV 기회 스코어링
- 블록 우선순위 예측

### 4. BundleManager (번들 관리자)

**번들 최적화**:
- 트랜잭션 순서 최적화
- 가스 한도 계산
- 수익성 시뮬레이션
- Flashbots 호환성

### 5. FlashLoan Integration (플래시론 통합)

**지원 프로토콜**:
- **Aave V3**: 최저 수수료 (0.05%)
- **Aave V2**: 안정성 우선 (0.09%)
- **Balancer**: 수수료 없음 (0.00%)
- **Uniswap V3**: 높은 유동성 (0.30%)

## 데이터 플로우

### 1. 기회 발견 플로우

```
멤풀 트랜잭션 → 필터링 → 분석 → 스코어링 → 전략 라우팅
       ↓           ↓        ↓        ↓           ↓
    Raw Tx     High Value  MEV Opp   Priority   Strategy
    Stream      Trades     Score      Queue     Selection
```

### 2. 실행 플로우

```
전략 선택 → 플래시론 확인 → 번들 생성 → 시뮬레이션 → 제출
    ↓           ↓           ↓         ↓         ↓
Strategy    Liquidity    Bundle    Profit    Flashbots
Selection    Check      Creation   Validation   Relay
```

### 3. 모니터링 플로우

```
실행 결과 → 성과 분석 → 전략 조정 → 리스크 평가 → 대시보드 업데이트
    ↓          ↓         ↓          ↓           ↓
Execution   Performance Strategy    Risk     Frontend
Results     Metrics    Adjustment  Analysis    Update
```

## 성능 최적화

### 1. 저지연 처리
- **WebSocket 연결**: 실시간 멤풀 스트림
- **병렬 처리**: 토큐오 기반 비동기 처리
- **메모리 풀**: 사전 할당된 객체 재사용
- **SIMD 최적화**: 수학적 계산 가속화

### 2. 가스 최적화
- **동적 가스 가격**: 네트워크 혼잡도 기반 조정
- **번들 압축**: 트랜잭션 배치 최적화
- **우선 가스**: 경쟁력 있는 가스 가격 책정

### 3. 메모리 관리
- **Zero-copy**: 불필요한 메모리 복사 방지
- **Arc/Mutex**: 스레드간 효율적인 데이터 공유
- **Connection Pool**: 재사용 가능한 연결 관리

## 보안 및 리스크 관리

### 1. 스마트 컨트랙트 보안
- **재진입 공격 방지**: ReentrancyGuard 적용
- **오버플로우 방지**: SafeMath 라이브러리 사용
- **권한 관리**: OnlyOwner 모디파이어
- **긴급 정지**: Circuit Breaker 패턴

### 2. 운영 리스크 관리
- **자금 한도**: 거래당 최대 금액 제한
- **손실 한계**: 일일 최대 손실 설정
- **건강 체크**: 시스템 상태 실시간 모니터링
- **자동 복구**: 장애 시 자동 재시작

### 3. 시장 리스크 관리
- **슬리피지 보호**: 최대 슬리피지 제한
- **가격 오라클**: 다중 소스 가격 검증
- **유동성 체크**: 충분한 유동성 확인
- **MEV 경쟁**: 다른 봇과의 경쟁 분석

## API 구조

### RESTful API 엔드포인트

```
GET  /api/dashboard           - 메인 대시보드 데이터
GET  /api/strategies          - 전략별 성과 데이터
GET  /api/bundles            - 번들 거래 이력
GET  /api/mempool/status     - 멤풀 상태 정보
GET  /api/mempool/txs        - 최근 트랜잭션
GET  /api/flashloan/dashboard - 플래시론 대시보드
GET  /api/performance        - 성능 메트릭
GET  /api/alerts             - 알림 정보
GET  /api/risk               - 리스크 분석
GET  /api/network            - 네트워크 상태
POST /api/strategies/toggle  - 전략 활성화/비활성화
POST /api/emergency/stop     - 긴급 정지
```

### WebSocket 스트림

```
ws://localhost:8080/ws/mempool    - 실시간 멤풀 데이터
ws://localhost:8080/ws/trades     - 실시간 거래 데이터  
ws://localhost:8080/ws/metrics    - 실시간 성능 메트릭
ws://localhost:8080/ws/alerts     - 실시간 알림
```

## 배포 및 확장성

### 1. 컨테이너화
```dockerfile
FROM rust:1.70-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/searcher /usr/local/bin/
EXPOSE 8080 9090
CMD ["searcher", "--strategies", "all"]
```

### 2. 수평 확장
- **멀티 인스턴스**: 전략별 전용 인스턴스
- **로드 밸런싱**: NGINX 기반 요청 분산
- **데이터 동기화**: Redis 클러스터
- **상태 공유**: PostgreSQL 클러스터

### 3. 모니터링
- **Prometheus**: 메트릭 수집
- **Grafana**: 시각화 대시보드
- **Jaeger**: 분산 트레이싱
- **ELK Stack**: 로그 분석

## 향후 개선사항

### 1. 기술적 개선
- **L2 지원**: Arbitrum, Optimism, Polygon 추가
- **크로스체인**: 체인간 차익거래 확장
- **ML 예측**: 머신러닝 기반 기회 예측
- **GPU 가속**: CUDA 기반 계산 최적화

### 2. 전략 확장
- **NFT MEV**: NFT 거래 차익거래
- **Options**: 옵션 거래 전략
- **Yield Farming**: 유동성 채굴 최적화
- **Governance**: 거버넌스 토큰 차익거래

### 3. 사용자 경험
- **모바일 앱**: 실시간 모니터링
- **알림 시스템**: Telegram, Discord 통합  
- **백테스팅**: 과거 데이터 기반 전략 검증
- **시뮬레이션**: 가상 환경에서 전략 테스트