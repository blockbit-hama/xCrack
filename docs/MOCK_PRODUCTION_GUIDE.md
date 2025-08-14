# xCrack Mock/실제 환경 전환 가이드

## 📋 개요

xCrack은 안전한 개발과 테스트를 위해 Mock 모드와 실제 운영 모드를 지원합니다. 이 문서는 두 환경 간의 전환 방법과 각 환경의 특징을 설명합니다.

## 🎭 Mock 모드 (개발/테스트)

### 특징
- **안전한 테스트 환경**: 실제 자금이나 네트워크 상호작용 없음
- **시뮬레이션 데이터**: Mock 시장 데이터와 거래 시뮬레이션
- **빠른 개발**: 실제 블록체인 연결 없이 빠른 테스트 가능
- **무료 사용**: 가스비나 거래 수수료 없음

### 현재 Mock 구현 상태

#### ✅ 완전 구현된 Mock 기능
1. **예측기반 자동매매** (`predictive_demo`)
   - Mock AI 예측 모델
   - 시뮬레이션 거래 실행
   - 랜덤 시장 데이터 생성
   - 성과 추적

2. **마이크로 아비트러지** (`micro_arbitrage`)
   - Mock 거래소 연결
   - 시뮬레이션 차익거래 기회 탐지
   - 가상 주문 실행

3. **MEV 전략** (`sandwich`, `liquidation`)
   - Mock 멤풀 모니터링
   - 시뮬레이션 샌드위치 공격
   - 가상 청산 기회 탐지

### Mock 모드 실행 방법

#### 1. 예측기반 자동매매 Demo
```bash
# 예측기반 전략 단독 실행 (추천)
cargo run --bin predictive_demo

# 또는 메인 바이너리에서 예측기반 전략만 실행
API_MODE=mock cargo run --bin searcher -- --strategies predictive
```

#### 2. 마이크로 아비트러지 Mock
```bash
# Mock 모드로 마이크로 아비트러지 실행
API_MODE=mock cargo run --bin searcher -- --strategies micro_arbitrage
```

#### 3. MEV 전략 Mock
```bash
# Mock 모드로 샌드위치 + 청산 전략 실행
API_MODE=mock cargo run --bin searcher -- --strategies sandwich,liquidation
```

#### 4. 전체 Mock 시스템
```bash
# 모든 전략을 Mock 모드로 실행
API_MODE=mock cargo run --bin searcher -- --strategies sandwich,liquidation,micro_arbitrage
```

### Mock 모드 설정

`.env` 파일 예시:
```bash
# Mock 모드 활성화
API_MODE=mock

# Mock 환경 설정
MOCK_MARKET_VOLATILITY=medium
MOCK_SUCCESS_RATE=80
MOCK_INITIAL_BALANCE=1000000000000000000  # 1 ETH in wei
```

## 🌐 실제 환경 (Production)

### 특징
- **실제 블록체인 연결**: Ethereum Mainnet/Testnet
- **실제 자금 사용**: 진짜 ETH, 토큰, 가스비
- **실시간 데이터**: 라이브 멤풀, 실제 시장 데이터
- **높은 성능 요구**: 레이턴시와 정확성이 중요

### ⚠️ 주의사항
- **실제 자금 손실 위험**: 잘못된 설정이나 전략으로 인한 손실 가능
- **가스비 소모**: 모든 거래에 실제 가스비 지불
- **보안 중요**: Private key와 API 키 보안 관리 필수
- **네트워크 의존성**: 인터넷 연결과 RPC 서비스 품질에 의존

### 실제 환경 설정

#### 1. 환경 변수 설정
```bash
# 실제 모드 활성화
API_MODE=real

# 네트워크 설정
ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
ETH_WS_URL=wss://mainnet.infura.io/ws/v3/YOUR_PROJECT_ID

# Flashbots 설정
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
PRIVATE_KEY=0x... # 실제 private key (매우 주의!)

# 알림 설정
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...
TELEGRAM_BOT_TOKEN=...
```

#### 2. 네트워크별 설정

##### Ethereum Mainnet (Production)
```bash
ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
CHAIN_ID=1
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
```

##### Goerli Testnet (테스트)
```bash
ETH_RPC_URL=https://goerli.infura.io/v3/YOUR_PROJECT_ID
CHAIN_ID=5
FLASHBOTS_RELAY_URL=https://relay-goerli.flashbots.net
```

##### Sepolia Testnet (테스트)
```bash
ETH_RPC_URL=https://sepolia.infura.io/v3/YOUR_PROJECT_ID
CHAIN_ID=11155111
FLASHBOTS_RELAY_URL=https://relay-sepolia.flashbots.net
```

## 🔄 환경 전환 가이드

### Mock → 실제 환경 전환

#### 1단계: 테스트넷에서 검증
```bash
# 1. 환경 변수 설정
cp .env.example .env.testnet
nano .env.testnet

# 2. 테스트넷 설정
API_MODE=real
ETH_RPC_URL=https://goerli.infura.io/v3/YOUR_PROJECT_ID
PRIVATE_KEY=0x... # 테스트넷용 private key
CHAIN_ID=5

# 3. 테스트넷에서 실행
source .env.testnet
cargo run --bin searcher -- --strategies micro_arbitrage
```

#### 2단계: 최소 자금으로 메인넷 테스트
```bash
# 1. 메인넷 환경 설정
cp .env.testnet .env.mainnet
nano .env.mainnet

# 2. 메인넷 설정 (최소 자금)
API_MODE=real
ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
PRIVATE_KEY=0x... # 최소 자금이 있는 지갑
CHAIN_ID=1

# 3. 보수적 설정으로 실행
MAX_GAS_PRICE=50  # 50 gwei 최대
MIN_PROFIT_THRESHOLD=0.01  # 최소 0.01 ETH 수익
```

#### 3단계: 풀 운영 배포
```bash
# 1. 최종 운영 환경 설정
cp .env.mainnet .env.production
nano .env.production

# 2. 운영 최적화 설정
MAX_GAS_PRICE=100
MIN_PROFIT_THRESHOLD=0.005
ENABLE_DISCORD_ALERTS=true
ENABLE_TELEGRAM_ALERTS=true

# 3. 운영 모드 실행
source .env.production
cargo run --release --bin searcher -- --strategies sandwich,liquidation,micro_arbitrage
```

### 실제 환경 → Mock 전환 (문제 해결)

```bash
# 1. 즉시 Mock 모드로 전환
API_MODE=mock cargo run --bin searcher -- --strategies predictive

# 2. 문제 재현 및 디버깅
cargo run --bin predictive_demo

# 3. 수정 후 다시 테스트넷에서 검증
API_MODE=real cargo run --bin searcher -- --strategies micro_arbitrage
```

## 🛡️ 보안 가이드

### Private Key 보안
```bash
# 1. 환경 변수로만 사용 (파일에 저장 금지)
export PRIVATE_KEY=0x...

# 2. 하드웨어 지갑 사용 (추천)
LEDGER_ACCOUNT=0
TREZOR_ACCOUNT=0

# 3. 권한 분리
# - 테스트: 최소 자금 지갑
# - 운영: 필요한 만큼만 자금 보유
```

### API 키 보안
```bash
# 1. 제한된 권한으로 API 키 생성
# - Read-only 권한으로 시작
# - 필요시에만 추가 권한 부여

# 2. IP 제한 설정
# - Infura, Alchemy 등에서 허용 IP 제한

# 3. 주기적 교체
# - API 키 정기적 교체 (월 1회 권장)
```

## 📊 모니터링 및 알림

### Mock 모드 모니터링
```bash
# 로그 레벨 설정
RUST_LOG=debug cargo run --bin predictive_demo

# 상세 모니터링
cargo run --bin searcher -- --log-level debug --strategies predictive
```

### 실제 환경 모니터링
```bash
# Discord 알림 설정
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...
ENABLE_DISCORD_ALERTS=true

# Telegram 알림 설정
TELEGRAM_BOT_TOKEN=...
TELEGRAM_CHAT_ID=...
ENABLE_TELEGRAM_ALERTS=true

# 수익/손실 임계값 알림
PROFIT_ALERT_THRESHOLD=0.1  # 0.1 ETH 이상 수익시 알림
LOSS_ALERT_THRESHOLD=0.05   # 0.05 ETH 이상 손실시 알림
```

## 🔧 문제 해결

### 일반적인 문제

#### 1. 컴파일 에러
```bash
# 의존성 업데이트
cargo update

# 클린 빌드
cargo clean && cargo build
```

#### 2. 네트워크 연결 실패
```bash
# RPC 엔드포인트 확인
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  $ETH_RPC_URL
```

#### 3. Mock 모드가 실행되지 않음
```bash
# 환경 변수 확인
echo $API_MODE

# Mock 모드 강제 설정
API_MODE=mock cargo run --bin predictive_demo
```

#### 4. 실제 환경에서 거래 실패
```bash
# 가스 가격 확인
# 잔액 확인
# 네트워크 상태 확인
```

### 로그 분석

#### Mock 모드 로그 패턴
```
INFO - 🧠 예측기반 자동매매 전략 시작 (Mock 모드)
INFO - 📊 Mock AI 예측 모델 로딩 중...
INFO - ✅ Mock AI 모델 로딩 완료
INFO - 🎯 예측 #1: 방향=-0.26, 신뢰도=37.73%, 기대수익=6.91%
```

#### 실제 환경 로그 패턴
```
INFO - 🌐 Real API mode enabled
INFO - 🔌 WebSocket 연결 중: wss://mainnet.infura.io/ws
INFO - ⚡ 실제 거래 실행: 매수 1 ETH
INFO - ✅ 거래 성공! TX: 0x...
```

## 📚 추가 자료

### 관련 문서
- [환경변수 설정 가이드](./ENVIRONMENT_SETUP.md)
- [아키텍처 문서](./ARCHITECTURE.md)
- [AI 예측 시스템 문서](./AI_PREDICTOR.md)

### 외부 리소스
- [Flashbots 문서](https://docs.flashbots.net/)
- [Infura API 가이드](https://docs.infura.io/)
- [Ethereum 개발자 가이드](https://ethereum.org/developers/)

---

## ⚠️ 면책 조항

이 소프트웨어는 교육 및 연구 목적으로 제공됩니다. 실제 환경에서 사용시 발생하는 모든 손실에 대해 개발자는 책임을 지지 않습니다. 항상 충분한 테스트 후 최소 자금으로 시작하시기 바랍니다.