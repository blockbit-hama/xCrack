# 🚀 xCrack 실행 가이드 (v2.0.0)

## 📋 목차
1. [시스템 요구사항](#시스템-요구사항)
2. [사전 준비](#사전-준비)
3. [설치 과정](#설치-과정)
4. [환경 설정](#환경-설정)
5. [실행 방법](#실행-방법)
6. [운영 모드](#운영-모드)
7. [모니터링](#모니터링)
8. [문제 해결](#문제-해결)

---

## 💻 시스템 요구사항

### 하드웨어
| 구성 | 최소 사양 | 권장 사양 | 고성능 사양 |
|------|----------|-----------|------------|
| **CPU** | 4 코어 | 8 코어 | 16+ 코어 |
| **RAM** | 8 GB | 16 GB | 32+ GB |
| **저장소** | 50 GB SSD | 100 GB NVMe | 500+ GB NVMe |
| **네트워크** | 100 Mbps | 1 Gbps | 10 Gbps |
| **지연시간** | < 100ms | < 50ms | < 10ms |

### 소프트웨어
```bash
# 필수 요구사항
- OS: Ubuntu 20.04+ / macOS 12+ / Windows 10+ (WSL2)
- Rust: 1.75.0+
- Node.js: 18.0+ (옵션)
- Git: 2.30+
- Docker: 20.10+ (옵션)
```

---

## 🔧 사전 준비

### 1. 개발 도구 설치

#### Rust 설치
```bash
# Rust 설치 (rustup 사용)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 버전 확인
rustc --version
cargo --version

# 필요한 컴포넌트 추가
rustup component add rustfmt clippy
```

#### Foundry 설치 (스마트 컨트랙트용)
```bash
# Foundry 설치
curl -L https://foundry.paradigm.xyz | bash
foundryup

# 버전 확인
forge --version
anvil --version
cast --version
```

### 2. API 키 준비

#### 필수 API 키
| 서비스 | 용도 | 가입 URL | 무료 티어 |
|--------|------|----------|-----------|
| **Alchemy** | 이더리움 RPC | https://alchemy.com | 300M CU/월 |
| **Infura** | 백업 RPC | https://infura.io | 100K 요청/일 |
| **Etherscan** | 컨트랙트 검증 | https://etherscan.io/apis | 5 요청/초 |

#### 선택적 API 키
| 서비스 | 용도 | 필요 전략 |
|--------|------|-----------|
| **LI.FI** | 크로스체인 브리지 | 크로스체인 아비트라지 |
| **1inch** | DEX 어그리게이터 | 마이크로 아비트라지 |
| **Binance** | CEX 가격 | 마이크로 아비트라지 |
| **Discord** | 알림 | 모든 전략 |
| **Telegram** | 알림 | 모든 전략 |

### 3. 지갑 준비

#### 개발용 지갑 생성
```bash
# 새 지갑 생성 (cast 사용)
cast wallet new

# 출력 예시:
# Address: 0x1234...abcd
# Private Key: 0xabcd...1234

# ⚠️ 절대 메인넷에서 사용하지 마세요!
```

#### 테스트넷 ETH 받기
```bash
# Sepolia 테스트넷 Faucet
# 1. https://sepoliafaucet.com
# 2. https://faucet.quicknode.com/ethereum/sepolia
# 3. https://sepolia-faucet.pk910.de

# Goerli 테스트넷 (지원 종료 예정)
# https://goerlifaucet.com
```

---

## 📥 설치 과정

### 1. 코드 다운로드
```bash
# GitHub에서 클론
git clone https://github.com/blockbit-hama/xCrack.git
cd xCrack

# 브랜치 확인
git branch -a
git checkout main
```

### 2. 의존성 설치
```bash
# Rust 의존성 설치
cargo build --release

# 스마트 컨트랙트 의존성 (옵션)
forge install

# Git hooks 설치 (권장)
./scripts/install-hooks.sh
```

### 3. 설정 파일 준비
```bash
# 기본 설정 파일 복사
cp config/default.toml config/local.toml

# 환경변수 파일 생성
cp .env.example .env.local
```

---

## ⚙️ 환경 설정

### 1. 환경변수 파일 작성

#### `.env.local` (개발/테스트)
```bash
# ====================================
# 🎯 핵심 설정
# ====================================
# 실행 모드: mock(테스트) 또는 real(실제)
API_MODE=mock

# 로깅 레벨: error, warn, info, debug, trace
RUST_LOG=info

# ====================================
# 🌐 네트워크 설정
# ====================================
# 이더리움 RPC (필수)
ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
ETH_WS_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY

# 멀티체인 RPC (크로스체인 전략용)
POLYGON_RPC_URL=https://polygon-mainnet.g.alchemy.com/v2/YOUR_API_KEY
BSC_RPC_URL=https://bsc-dataseed.binance.org/
ARBITRUM_RPC_URL=https://arb-mainnet.g.alchemy.com/v2/YOUR_API_KEY
OPTIMISM_RPC_URL=https://opt-mainnet.g.alchemy.com/v2/YOUR_API_KEY

# ====================================
# 🔐 보안 설정
# ====================================
# 지갑 Private Key (⚠️ 매우 중요!)
PRIVATE_KEY=0x0000000000000000000000000000000000000000000000000000000000000001

# Flashbots 설정 (MEV 전략용)
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
FLASHBOTS_AUTH_KEY=0x...

# ====================================
# 💰 전략 설정
# ====================================
# 활성화할 전략 (쉼표로 구분)
ENABLED_STRATEGIES=micro_arbitrage,cross_chain

# 공통 설정
MAX_GAS_PRICE_GWEI=100
MIN_PROFIT_THRESHOLD_ETH=0.01
MAX_POSITION_SIZE_ETH=10.0

# 마이크로 아비트라지
MICRO_ARB_ENABLED=true
MICRO_ARB_MIN_PROFIT_USD=10.0
MICRO_ARB_MAX_CONCURRENT_TRADES=3
MICRO_ARB_USE_FLASHLOAN=false

# 크로스체인 아비트라지
CROSS_CHAIN_ENABLED=true
CROSS_CHAIN_MIN_PROFIT_USD=50.0
CROSS_CHAIN_BRIDGE_TIMEOUT_MINUTES=15
LIFI_API_KEY=your_lifi_api_key

# 샌드위치 공격 (고위험)
SANDWICH_ENABLED=false
SANDWICH_MIN_TARGET_VALUE_ETH=1.0
SANDWICH_MAX_SLIPPAGE=0.03

# 청산 (중위험)
LIQUIDATION_ENABLED=false
LIQUIDATION_MIN_PROFIT_ETH=0.05
LIQUIDATION_PROTOCOLS=aave,compound

# ====================================
# 📊 모니터링 설정
# ====================================
# Discord 알림
ENABLE_DISCORD_ALERTS=true
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...

# Telegram 알림
ENABLE_TELEGRAM_ALERTS=false
TELEGRAM_BOT_TOKEN=123456789:ABCdef...
TELEGRAM_CHAT_ID=-123456789

# 메트릭 서버
ENABLE_METRICS=true
METRICS_PORT=9090
HEALTH_CHECK_PORT=8080

# ====================================
# 🧪 Mock 모드 설정
# ====================================
# Mock 시장 설정
MOCK_MARKET_VOLATILITY=medium
MOCK_SUCCESS_RATE=95
MOCK_INITIAL_BALANCE_ETH=100
MOCK_INITIAL_BALANCE_USDC=200000
```

### 2. TOML 설정 파일 수정

#### `config/local.toml`
```toml
[network]
chain_id = 1
name = "ethereum-mainnet"

[rpc]
http_url = "${ETH_RPC_URL}"
ws_url = "${ETH_WS_URL}"
max_retries = 3
timeout_ms = 10000

[wallet]
private_key = "${PRIVATE_KEY}"
max_gas_price_gwei = 100

[strategies.micro_arbitrage]
enabled = true
min_profit_usd = 10.0
max_position_size_eth = 5.0
use_flashloan = false

[strategies.cross_chain_arbitrage]
enabled = true
min_profit_usd = 50.0
supported_chains = ["ethereum", "polygon", "bsc"]

[monitoring]
discord_webhook = "${DISCORD_WEBHOOK_URL}"
alert_threshold_eth = 0.1
```

### 3. 설정 검증 스크립트

```bash
#!/bin/bash
# scripts/verify_config.sh

echo "🔍 xCrack 설정 검증 시작..."
echo "================================"

# 1. 환경변수 확인
check_env() {
    if [ -z "${!1}" ]; then
        echo "❌ $1 누락"
        return 1
    else
        if [[ "$1" == *"KEY"* ]] || [[ "$1" == "PRIVATE_KEY" ]]; then
            echo "✅ $1: [HIDDEN]"
        else
            echo "✅ $1: ${!1}"
        fi
        return 0
    fi
}

# 필수 환경변수 체크
REQUIRED_VARS=(
    "API_MODE"
    "ETH_RPC_URL"
    "PRIVATE_KEY"
)

ERROR_COUNT=0
for var in "${REQUIRED_VARS[@]}"; do
    check_env "$var" || ((ERROR_COUNT++))
done

# 2. RPC 연결 테스트
echo ""
echo "🔌 RPC 연결 테스트..."
if [ "$API_MODE" = "real" ]; then
    curl -s -X POST -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        $ETH_RPC_URL > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        echo "✅ RPC 연결 성공"
    else
        echo "❌ RPC 연결 실패"
        ((ERROR_COUNT++))
    fi
fi

# 3. 설정 파일 확인
echo ""
echo "📁 설정 파일 확인..."
if [ -f "config/local.toml" ]; then
    echo "✅ config/local.toml 존재"
else
    echo "❌ config/local.toml 누락"
    ((ERROR_COUNT++))
fi

# 4. 결과
echo ""
echo "================================"
if [ $ERROR_COUNT -eq 0 ]; then
    echo "✅ 모든 검증 통과! 실행 준비 완료"
else
    echo "❌ $ERROR_COUNT개 문제 발견. 수정 필요"
    exit 1
fi
```

---

## 🎮 실행 방법

### 1. Mock 모드 (개발/테스트)

```bash
# 기본 Mock 모드 실행
API_MODE=mock cargo run

# 특정 전략만 테스트
API_MODE=mock cargo run -- --strategies micro_arbitrage

# 시뮬레이션 모드
API_MODE=mock cargo run -- --simulation

# 디버그 모드
API_MODE=mock RUST_LOG=debug cargo run
```

### 2. 테스트넷 모드

```bash
# Sepolia 테스트넷
export ETH_RPC_URL="https://sepolia.infura.io/v3/YOUR_KEY"
export FLASHBOTS_RELAY_URL="https://relay-sepolia.flashbots.net"
API_MODE=real cargo run -- --network sepolia

# Goerli 테스트넷
export ETH_RPC_URL="https://goerli.infura.io/v3/YOUR_KEY"
API_MODE=real cargo run -- --network goerli
```

### 3. 메인넷 모드 (프로덕션)

```bash
# ⚠️ 실제 자금이 사용됩니다! 신중하게 실행하세요

# 안전 모드 (dry-run)
API_MODE=real cargo run --release -- --dry-run

# 실제 실행 (단일 전략)
API_MODE=real cargo run --release -- --strategies micro_arbitrage

# 실제 실행 (모든 전략)
API_MODE=real cargo run --release -- --strategies all

# 백그라운드 실행
nohup cargo run --release > logs/xcrack.log 2>&1 &
```

### 4. Docker 실행 (권장)

```dockerfile
# Dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/xcrack /usr/local/bin/
COPY --from=builder /app/config /config
CMD ["xcrack"]
```

```bash
# Docker 빌드 및 실행
docker build -t xcrack:latest .
docker run -d \
    --name xcrack \
    --env-file .env.local \
    -v $(pwd)/config:/config \
    -v $(pwd)/logs:/logs \
    xcrack:latest
```

### 5. Systemd 서비스 (Linux)

```ini
# /etc/systemd/system/xcrack.service
[Unit]
Description=xCrack MEV Searcher Bot
After=network.target

[Service]
Type=simple
User=xcrack
WorkingDirectory=/home/xcrack/xCrack
EnvironmentFile=/home/xcrack/xCrack/.env.local
ExecStart=/home/xcrack/xCrack/target/release/xcrack
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
# 서비스 등록 및 실행
sudo systemctl daemon-reload
sudo systemctl enable xcrack
sudo systemctl start xcrack
sudo systemctl status xcrack
```

---

## 🔄 운영 모드

### 전략별 권장 설정

| 모드 | 리스크 | 자본 요구 | 예상 수익률 | 권장 설정 |
|------|--------|-----------|------------|----------|
| **초보자** | 낮음 | 0.1 ETH | 5-10% APY | Mock 모드 + 마이크로 아비트라지 |
| **중급자** | 중간 | 1 ETH | 20-50% APY | 테스트넷 → 메인넷 마이크로 |
| **고급자** | 높음 | 10 ETH | 50-200% APY | 모든 전략 활성화 |
| **전문가** | 매우 높음 | 100+ ETH | 200%+ APY | MEV + 플래시론 |

### 단계별 실행 가이드

#### 1단계: Mock 모드 학습 (1-2주)
```bash
# Mock 모드로 시스템 이해
API_MODE=mock cargo run -- --simulation

# 목표:
# - 시스템 이해
# - 전략 테스트
# - 설정 최적화
```

#### 2단계: 테스트넷 실전 (2-4주)
```bash
# Sepolia 테스트넷 실행
API_MODE=real cargo run -- --network sepolia --strategies micro_arbitrage

# 목표:
# - 실제 네트워크 경험
# - 가스 최적화
# - 버그 발견
```

#### 3단계: 메인넷 소액 (1-2개월)
```bash
# 소액으로 메인넷 시작
API_MODE=real cargo run --release -- \
    --strategies micro_arbitrage \
    --max-position 0.1

# 목표:
# - 실제 수익 창출
# - 리스크 관리
# - 성능 모니터링
```

#### 4단계: 점진적 확대
```bash
# 검증된 전략만 확대
API_MODE=real cargo run --release -- \
    --strategies micro_arbitrage,cross_chain \
    --max-position 1.0

# 목표:
# - 수익 극대화
# - 포트폴리오 다각화
# - 자동화 완성
```

---

## 📊 모니터링

### 1. 실시간 로그 모니터링

```bash
# 실시간 로그 확인
tail -f logs/xcrack.log

# 에러만 필터링
tail -f logs/xcrack.log | grep ERROR

# 수익 추적
tail -f logs/xcrack.log | grep PROFIT

# 컬러 출력 (권장)
tail -f logs/xcrack.log | ccze -A
```

### 2. 성능 대시보드

```bash
# Prometheus 메트릭 서버 (localhost:9090)
curl http://localhost:9090/metrics

# 주요 메트릭:
# - xcrack_profit_total: 총 수익
# - xcrack_trades_total: 총 거래 수
# - xcrack_success_rate: 성공률
# - xcrack_gas_spent_total: 총 가스 비용
```

### 3. 헬스체크

```bash
# 헬스체크 엔드포인트
curl http://localhost:8080/health

# 응답 예시:
{
  "status": "healthy",
  "uptime": 86400,
  "strategies": ["micro_arbitrage", "cross_chain"],
  "last_trade": "2025-01-27T12:34:56Z"
}
```

### 4. 알림 설정

#### Discord 알림
```javascript
// Discord 웹훅 메시지 형식
{
  "embeds": [{
    "title": "💰 수익 발생!",
    "description": "마이크로 아비트라지 성공",
    "fields": [
      {"name": "수익", "value": "0.05 ETH ($125)"},
      {"name": "가스 비용", "value": "0.01 ETH"},
      {"name": "순수익", "value": "0.04 ETH ($100)"}
    ],
    "color": 5832650
  }]
}
```

#### Telegram 알림
```bash
# Telegram 봇 설정
# 1. @BotFather로 봇 생성
# 2. 토큰 받기
# 3. 채팅방 ID 확인
curl https://api.telegram.org/bot${TOKEN}/getUpdates
```

---

## 🐛 문제 해결

### 일반적인 문제와 해결법

#### 1. 컴파일 오류
```bash
# 오류: could not compile `xcrack`
# 해결:
rustup update
cargo clean
cargo build --release

# Rust 버전 확인
rustc --version
```

#### 2. RPC 연결 실패
```bash
# 오류: Failed to connect to RPC endpoint
# 해결:
# 1. API 키 확인
echo $ETH_RPC_URL

# 2. 연결 테스트
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  $ETH_RPC_URL

# 3. 백업 RPC로 전환
export ETH_RPC_URL="https://ethereum.publicnode.com"
```

#### 3. 가스 가격 너무 높음
```bash
# 오류: Gas price too high
# 해결:
# 1. 가스 상한 조정
export MAX_GAS_PRICE_GWEI=50

# 2. 가스 가격 확인
cast gas-price

# 3. 낮은 시간대 실행
# 주말, 한국 시간 새벽 추천
```

#### 4. 메모리 부족
```bash
# 오류: Out of memory
# 해결:
# 1. 스왑 추가 (Linux)
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# 2. 동시 거래 수 제한
export MICRO_ARB_MAX_CONCURRENT_TRADES=1
```

#### 5. Private Key 오류
```bash
# 오류: Invalid private key format
# 해결:
# 1. 형식 확인 (0x로 시작, 64자)
echo $PRIVATE_KEY | grep -E "^0x[0-9a-fA-F]{64}$"

# 2. 새 키 생성
cast wallet new

# 3. 권한 확인
chmod 600 .env.local
```

### 긴급 상황 대응

#### 시스템 즉시 중단
```bash
# 프로세스 찾기
ps aux | grep xcrack

# 강제 종료
kill -9 <PID>

# Docker 중단
docker stop xcrack

# Systemd 중단
sudo systemctl stop xcrack
```

#### 자금 긴급 이동
```bash
# 잔액 확인
cast balance $ADDRESS

# 전체 잔액 이동
cast send --private-key $PRIVATE_KEY \
  $SAFE_ADDRESS \
  --value $(cast balance $ADDRESS)
```

---

## 📚 추가 리소스

### 관련 문서
- [🏗️ 시스템 아키텍처](./ARCHITECTURE.md)
- [⚙️ 환경 설정](./ENVIRONMENT_SETUP.md)
- [📖 API 레퍼런스](./API_REFERENCE.md)
- [🧪 Mock/Production 가이드](./MOCK_PRODUCTION_GUIDE.md)

### 유용한 도구
- [Etherscan](https://etherscan.io) - 트랜잭션 확인
- [Tenderly](https://tenderly.co) - 시뮬레이션
- [Blocknative](https://blocknative.com) - 멤풀 모니터링
- [DexScreener](https://dexscreener.com) - DEX 가격 추적

### 커뮤니티
- Discord: [참여 링크]
- Telegram: [@xcrack_community]
- GitHub Issues: [버그 리포트]

---

## ✅ 체크리스트

### 실행 전 체크리스트
- [ ] Rust 1.75+ 설치됨
- [ ] API 키 준비 완료
- [ ] Private Key 설정됨
- [ ] 환경변수 파일 생성됨
- [ ] 설정 검증 통과
- [ ] RPC 연결 테스트 성공
- [ ] Mock 모드 테스트 완료

### 메인넷 실행 전 필수 체크
- [ ] 테스트넷에서 2주 이상 운영
- [ ] 수익률 검증 완료
- [ ] 리스크 관리 정책 수립
- [ ] 긴급 대응 계획 준비
- [ ] 백업 지갑 준비
- [ ] 모니터링 시스템 구축
- [ ] 충분한 자금 확보 (가스비 포함)

---

**🎉 준비 완료! xCrack을 실행할 준비가 되었습니다.**

문제가 있으면 GitHub Issues나 Discord로 문의하세요.