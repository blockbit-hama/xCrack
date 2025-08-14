# xCrack 환경변수 설정 가이드

## 📋 개요

xCrack은 다양한 환경변수를 통해 설정을 관리합니다. 이 문서는 모든 환경변수의 설정 방법과 용도를 상세히 설명합니다.

## 🔧 필수 환경변수

### API_MODE (핵심 설정)
xCrack의 실행 모드를 결정하는 가장 중요한 변수입니다.

```bash
# Mock 모드 (개발/테스트용)
API_MODE=mock

# 실제 모드 (운영용)
API_MODE=real
```

**영향받는 기능:**
- 네트워크 연결 (Mock vs Real)
- 거래 실행 (시뮬레이션 vs 실제)
- 데이터 소스 (Mock vs Live)
- 가스비 사용 (가상 vs 실제)

### 네트워크 설정

#### ETH_RPC_URL
이더리움 RPC 엔드포인트 URL입니다.

```bash
# Mainnet
ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
ETH_RPC_URL=https://ethereum.publicnode.com

# Goerli Testnet
ETH_RPC_URL=https://goerli.infura.io/v3/YOUR_PROJECT_ID

# Sepolia Testnet
ETH_RPC_URL=https://sepolia.infura.io/v3/YOUR_PROJECT_ID

# Local
ETH_RPC_URL=http://localhost:8545
```

#### ETH_WS_URL
WebSocket 연결 URL (실시간 데이터용)입니다.

```bash
# Mainnet
ETH_WS_URL=wss://mainnet.infura.io/ws/v3/YOUR_PROJECT_ID
ETH_WS_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY

# Testnet
ETH_WS_URL=wss://goerli.infura.io/ws/v3/YOUR_PROJECT_ID
ETH_WS_URL=wss://sepolia.infura.io/ws/v3/YOUR_PROJECT_ID
```

### 보안 설정

#### PRIVATE_KEY
트랜잭션 서명용 개인키입니다. **매우 중요한 보안 정보입니다.**

```bash
# 실제 private key (0x 접두사 포함)
PRIVATE_KEY=0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef

# Mock 모드용 테스트 키
PRIVATE_KEY=0x0000000000000000000000000000000000000000000000000000000000000001
```

**보안 주의사항:**
- 절대로 코드나 공개 저장소에 포함하지 마세요
- `.env` 파일을 `.gitignore`에 추가하세요
- 테스트와 운영 키를 분리하세요
- 하드웨어 지갑 사용을 권장합니다

#### FLASHBOTS_RELAY_URL
Flashbots 릴레이 서버 URL입니다.

```bash
# Mainnet
FLASHBOTS_RELAY_URL=https://relay.flashbots.net

# Goerli Testnet
FLASHBOTS_RELAY_URL=https://relay-goerli.flashbots.net

# Sepolia Testnet
FLASHBOTS_RELAY_URL=https://relay-sepolia.flashbots.net

# Mock 모드
FLASHBOTS_RELAY_URL=mock://flashbots
```

## 📊 모니터링 및 알림 설정

### Discord 알림

#### DISCORD_WEBHOOK_URL
Discord 채널로 알림을 받기 위한 웹훅 URL입니다.

```bash
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/123456789/abcdefghijklmnop
```

**설정 방법:**
1. Discord 서버에서 웹훅 생성
2. URL 복사
3. 환경변수에 설정

#### ENABLE_DISCORD_ALERTS
Discord 알림 활성화 여부입니다.

```bash
ENABLE_DISCORD_ALERTS=true   # 활성화
ENABLE_DISCORD_ALERTS=false  # 비활성화
```

### Telegram 알림

#### TELEGRAM_BOT_TOKEN
Telegram 봇 토큰입니다.

```bash
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz
```

#### TELEGRAM_CHAT_ID
알림을 받을 Telegram 채팅 ID입니다.

```bash
TELEGRAM_CHAT_ID=123456789
TELEGRAM_CHAT_ID=-123456789  # 그룹 채팅의 경우
```

#### ENABLE_TELEGRAM_ALERTS
Telegram 알림 활성화 여부입니다.

```bash
ENABLE_TELEGRAM_ALERTS=true   # 활성화
ENABLE_TELEGRAM_ALERTS=false  # 비활성화
```

## ⚙️ 전략별 설정

### 공통 전략 설정

#### MAX_GAS_PRICE
최대 가스 가격 (Gwei 단위)입니다.

```bash
# 보수적 설정
MAX_GAS_PRICE=50

# 적극적 설정
MAX_GAS_PRICE=100

# 매우 적극적 설정
MAX_GAS_PRICE=200
```

#### MIN_PROFIT_THRESHOLD
최소 수익 임계값 (ETH 단위)입니다.

```bash
# 보수적 설정
MIN_PROFIT_THRESHOLD=0.01   # 0.01 ETH 이상

# 균형 설정
MIN_PROFIT_THRESHOLD=0.005  # 0.005 ETH 이상

# 적극적 설정
MIN_PROFIT_THRESHOLD=0.001  # 0.001 ETH 이상
```

### 예측기반 자동매매 설정

#### AI_PREDICTOR_CONFIG_PATH
AI 예측 시스템 설정 파일 경로입니다.

```bash
AI_PREDICTOR_CONFIG_PATH=./ai_predictor/config/settings.yaml
```

#### MIN_CONFIDENCE_THRESHOLD
최소 신뢰도 임계값입니다.

```bash
MIN_CONFIDENCE_THRESHOLD=0.7   # 70% 이상 신뢰도
MIN_CONFIDENCE_THRESHOLD=0.8   # 80% 이상 신뢰도
MIN_CONFIDENCE_THRESHOLD=0.9   # 90% 이상 신뢰도
```

### 마이크로 아비트러지 설정

#### ARBITRAGE_MIN_SPREAD
최소 차익 스프레드 (퍼센트)입니다.

```bash
ARBITRAGE_MIN_SPREAD=0.1   # 0.1% 이상 차익
ARBITRAGE_MIN_SPREAD=0.3   # 0.3% 이상 차익
ARBITRAGE_MIN_SPREAD=0.5   # 0.5% 이상 차익
```

#### EXCHANGE_ENDPOINTS
지원하는 거래소 엔드포인트입니다.

```bash
# 중앙화 거래소
BINANCE_API_URL=https://api.binance.com
COINBASE_API_URL=https://api.exchange.coinbase.com

# 탈중앙화 거래소
UNISWAP_V2_URL=https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v2
UNISWAP_V3_URL=https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v3
```

### MEV 전략 설정

#### SANDWICH_MAX_SLIPPAGE
샌드위치 공격 최대 슬리피지입니다.

```bash
SANDWICH_MAX_SLIPPAGE=0.05   # 5% 최대 슬리피지
SANDWICH_MAX_SLIPPAGE=0.03   # 3% 최대 슬리피지
SANDWICH_MAX_SLIPPAGE=0.01   # 1% 최대 슬리피지
```

#### LIQUIDATION_MIN_PROFIT
청산 전략 최소 수익입니다.

```bash
LIQUIDATION_MIN_PROFIT=0.02   # 0.02 ETH 이상
LIQUIDATION_MIN_PROFIT=0.05   # 0.05 ETH 이상
LIQUIDATION_MIN_PROFIT=0.1    # 0.1 ETH 이상
```

## 🔄 Mock 모드 전용 설정

### Mock 시뮬레이션 설정

#### MOCK_MARKET_VOLATILITY
Mock 시장 변동성 레벨입니다.

```bash
MOCK_MARKET_VOLATILITY=low      # 낮은 변동성
MOCK_MARKET_VOLATILITY=medium   # 중간 변동성
MOCK_MARKET_VOLATILITY=high     # 높은 변동성
```

#### MOCK_SUCCESS_RATE
Mock 거래 성공률입니다.

```bash
MOCK_SUCCESS_RATE=95   # 95% 성공률
MOCK_SUCCESS_RATE=80   # 80% 성공률
MOCK_SUCCESS_RATE=60   # 60% 성공률
```

#### MOCK_INITIAL_BALANCE
Mock 모드 초기 잔액 (Wei 단위)입니다.

```bash
# 1 ETH
MOCK_INITIAL_BALANCE=1000000000000000000

# 10 ETH
MOCK_INITIAL_BALANCE=10000000000000000000

# 100 ETH
MOCK_INITIAL_BALANCE=100000000000000000000
```

## 📁 환경별 .env 파일 설정

### .env.mock (Mock 개발 환경)

```bash
# 기본 모드 설정
API_MODE=mock

# Mock 네트워크 설정
ETH_RPC_URL=mock://ethereum
ETH_WS_URL=mock://ethereum/ws
FLASHBOTS_RELAY_URL=mock://flashbots

# Mock 보안 설정
PRIVATE_KEY=0x0000000000000000000000000000000000000000000000000000000000000001

# Mock 시뮬레이션 설정
MOCK_MARKET_VOLATILITY=medium
MOCK_SUCCESS_RATE=80
MOCK_INITIAL_BALANCE=1000000000000000000

# 전략 설정
MAX_GAS_PRICE=50
MIN_PROFIT_THRESHOLD=0.005
MIN_CONFIDENCE_THRESHOLD=0.7

# 로깅
RUST_LOG=info
```

### .env.testnet (테스트넷 환경)

```bash
# 기본 모드 설정
API_MODE=real

# Goerli 테스트넷 설정
ETH_RPC_URL=https://goerli.infura.io/v3/YOUR_PROJECT_ID
ETH_WS_URL=wss://goerli.infura.io/ws/v3/YOUR_PROJECT_ID
FLASHBOTS_RELAY_URL=https://relay-goerli.flashbots.net

# 테스트넷 보안 설정
PRIVATE_KEY=0x... # 테스트넷 전용 키

# 전략 설정 (보수적)
MAX_GAS_PRICE=30
MIN_PROFIT_THRESHOLD=0.01
MIN_CONFIDENCE_THRESHOLD=0.8

# 알림 설정 (선택적)
ENABLE_DISCORD_ALERTS=true
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...

# 로깅
RUST_LOG=debug
```

### .env.mainnet (메인넷 환경)

```bash
# 기본 모드 설정
API_MODE=real

# 메인넷 설정
ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
ETH_WS_URL=wss://mainnet.infura.io/ws/v3/YOUR_PROJECT_ID
FLASHBOTS_RELAY_URL=https://relay.flashbots.net

# 실제 보안 설정
PRIVATE_KEY=0x... # 실제 private key (매우 주의!)

# 전략 설정
MAX_GAS_PRICE=100
MIN_PROFIT_THRESHOLD=0.005
MIN_CONFIDENCE_THRESHOLD=0.75

# 전체 알림 활성화
ENABLE_DISCORD_ALERTS=true
ENABLE_TELEGRAM_ALERTS=true
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...
TELEGRAM_BOT_TOKEN=...
TELEGRAM_CHAT_ID=...

# 수익/손실 알림 임계값
PROFIT_ALERT_THRESHOLD=0.1
LOSS_ALERT_THRESHOLD=0.05

# 로깅
RUST_LOG=info
```

## 🚀 실행 방법

### 환경변수 파일 사용

```bash
# Mock 환경으로 실행
cp .env.mock .env
cargo run --bin predictive_demo

# 테스트넷 환경으로 실행
cp .env.testnet .env
cargo run --bin searcher -- --strategies micro_arbitrage

# 메인넷 환경으로 실행
cp .env.mainnet .env
cargo run --release --bin searcher -- --strategies sandwich,liquidation
```

### 직접 환경변수 설정

```bash
# 임시 환경변수 설정
export API_MODE=mock
export RUST_LOG=debug
cargo run --bin predictive_demo

# 한 줄로 실행
API_MODE=mock RUST_LOG=debug cargo run --bin predictive_demo
```

### 환경변수 파일 로드

```bash
# 특정 환경변수 파일 로드
source .env.testnet
cargo run --bin searcher

# 또는 dotenv 사용
API_MODE=real cargo run --bin searcher -- --strategies predictive
```

## 🔍 환경변수 검증

### 설정 확인 스크립트

```bash
#!/bin/bash
# check_env.sh

echo "=== xCrack 환경변수 검증 ==="

# 필수 변수 확인
if [ -z "$API_MODE" ]; then
    echo "❌ API_MODE가 설정되지 않았습니다"
else
    echo "✅ API_MODE: $API_MODE"
fi

if [ "$API_MODE" = "real" ]; then
    if [ -z "$ETH_RPC_URL" ]; then
        echo "❌ ETH_RPC_URL이 설정되지 않았습니다"
    else
        echo "✅ ETH_RPC_URL: $ETH_RPC_URL"
    fi
    
    if [ -z "$PRIVATE_KEY" ]; then
        echo "❌ PRIVATE_KEY가 설정되지 않았습니다"
    else
        echo "✅ PRIVATE_KEY: [HIDDEN]"
    fi
fi

echo "================"
```

```bash
# 실행
chmod +x check_env.sh
./check_env.sh
```

### 런타임 검증

xCrack은 시작시 자동으로 환경변수를 검증합니다:

```rust
// 자동 검증 로그 예시
INFO - 🎭 Mock mode enabled - using mock APIs
INFO - 🔑 Private key loaded from environment
INFO - 🔌 RPC URL loaded from environment
INFO - ⚡ Flashbots relay URL loaded from environment
```

## 🐛 문제 해결

### 일반적인 오류

#### 1. API_MODE 미설정
```
오류: API_MODE 환경변수가 설정되지 않았습니다
해결: API_MODE=mock 또는 API_MODE=real 설정
```

#### 2. Private Key 형식 오류
```
오류: Invalid private key format
해결: 0x로 시작하는 64자 16진수 확인
```

#### 3. RPC URL 연결 실패
```
오류: Failed to connect to RPC endpoint
해결: 
- URL 형식 확인
- API 키 유효성 확인
- 네트워크 연결 확인
```

#### 4. Webhook URL 형식 오류
```
오류: Invalid Discord webhook URL
해결: https://discord.com/api/webhooks/... 형식 확인
```

### 디버깅 팁

```bash
# 환경변수 전체 출력
env | grep -E "^(API_MODE|ETH_|FLASHBOTS_|DISCORD_|TELEGRAM_)"

# 특정 변수 확인
echo "API_MODE: $API_MODE"
echo "ETH_RPC_URL: $ETH_RPC_URL"

# .env 파일 확인
cat .env | grep -v "^#" | grep -v "^$"
```

## 📚 참고 자료

- [Mock/Production 전환 가이드](./MOCK_PRODUCTION_GUIDE.md)
- [xCrack 아키텍처 문서](./ARCHITECTURE.md)
- [AI 예측 시스템 문서](./AI_PREDICTOR.md)

---

**⚠️ 보안 주의사항**

- Private key는 절대로 공개하지 마세요
- .env 파일을 git에 커밋하지 마세요
- 테스트와 운영 환경을 분리하세요
- 정기적으로 API 키를 교체하세요