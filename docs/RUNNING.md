# 🚀 xCrack MEV Searcher 실행 가이드

xCrack MEV Searcher의 설치, 구성, 실행에 대한 완전한 가이드입니다.

## 📋 목차

1. [시스템 요구사항](#시스템-요구사항)
2. [설치](#설치)
3. [설정](#설정)
4. [실행 모드](#실행-모드)
5. [모니터링](#모니터링)
6. [문제 해결](#문제-해결)
7. [운영 가이드](#운영-가이드)

---

## 시스템 요구사항

### 최소 요구사항
- **OS**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows (WSL2 권장)
- **RAM**: 4GB 이상
- **CPU**: 2 코어 이상 
- **디스크**: 20GB 이상 여유 공간
- **네트워크**: 안정적인 인터넷 연결 (지연시간 < 100ms)

### 권장 요구사항
- **OS**: Linux (Ubuntu 22.04)
- **RAM**: 16GB 이상
- **CPU**: 8 코어 이상 (고성능 멀티코어)
- **디스크**: SSD 100GB 이상
- **네트워크**: 고속 인터넷 (지연시간 < 50ms)

### 필요 소프트웨어
```bash
# Rust 툴체인 (1.70+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 기타 의존성 (Ubuntu)
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev
```

---

## 설치

### 1. 저장소 클론
```bash
git clone https://github.com/your-org/xCrack.git
cd xCrack
```

### 2. 의존성 설치 및 빌드
```bash
# 의존성 설치
cargo fetch

# 빌드 (릴리즈 모드)
cargo build --release

# 빌드 확인
cargo test
```

### 3. 실행 파일 확인
```bash
# 빌드된 실행 파일 위치
ls -la target/release/xcrack

# 버전 확인
./target/release/xcrack --version
```

---

## 설정

### 1. 기본 설정 파일 생성
```bash
# 기본 설정 파일 복사
cp config/default.toml config/local.toml

# 설정 파일 편집
nano config/local.toml
```

### 2. 필수 설정 항목

#### 네트워크 설정
```toml
[network]
chain_id = 1
name = "mainnet"
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY_HERE"
ws_url = "wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY_HERE"
```

> ⚠️ **중요**: `YOUR_API_KEY_HERE`를 실제 Alchemy API 키로 교체하세요.

#### Flashbots 설정
```toml
[flashbots]
relay_url = "https://relay.flashbots.net"
private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
max_priority_fee_per_gas = "2"  # gwei
max_fee_per_gas = "50"         # gwei
```

> 🔐 **보안**: 실제 프라이빗 키는 환경 변수로 관리하세요.

#### 마이크로 아비트래지 설정
```toml
[strategies.micro_arbitrage]
enabled = true
min_profit_percentage = 0.001      # 0.1% 최소 수익률
min_profit_usd = "10.0"           # 최소 $10 수익
max_position_size = "5.0"         # 최대 5 ETH 포지션
execution_timeout_ms = 5000       # 5초 타임아웃
```

### 3. 환경 변수 설정
```bash
# .env 파일 생성
cat > .env << EOF
# API 키
ALCHEMY_API_KEY=your_alchemy_api_key_here
FLASHBOTS_PRIVATE_KEY=0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef

# 실행 모드
API_MODE=mock  # 또는 real

# 로그 레벨
RUST_LOG=info

# 모니터링
DISCORD_WEBHOOK_URL=your_discord_webhook_url_here
EOF

# 환경 변수 로드
source .env
```

---

## 실행 모드

### 1. Mock 모드 (테스트/개발)
```bash
# Mock 모드로 실행 (안전)
API_MODE=mock cargo run --release

# 또는 환경 변수 설정 후
export API_MODE=mock
./target/release/xcrack
```

**Mock 모드 특징:**
- ✅ 실제 네트워크 연결 없음
- ✅ 가상 거래로 안전한 테스트
- ✅ 현실적인 시장 조건 시뮬레이션
- ✅ 실제 손실 위험 없음

### 2. Production 모드 (실제 거래)
```bash
# Production 모드로 실행 (실제 거래)
API_MODE=real cargo run --release

# 백그라운드 실행
nohup API_MODE=real ./target/release/xcrack > xcrack.log 2>&1 &
```

> ⚠️ **경고**: Production 모드는 실제 자금을 사용합니다. 충분한 테스트 후 사용하세요.

### 3. 특정 전략만 실행
```bash
# 마이크로 아비트래지만 실행
cargo run --release -- --strategies micro_arbitrage

# 여러 전략 동시 실행
cargo run --release -- --strategies sandwich,liquidation
```

### 4. 설정 파일 지정
```bash
# 커스텀 설정 파일 사용
cargo run --release -- --config config/production.toml

# 다양한 설정 조합
cargo run --release -- --config config/local.toml --strategies micro_arbitrage
```

---

## 실행 예시

### 개발/테스트 환경에서 시작하기

```bash
# 1. Mock 모드로 빠른 테스트
API_MODE=mock cargo run --quiet

# 예상 출력:
# 🚀 xCrack MEV Searcher v1.2.0 시작
# 📡 Mock 모드로 실행 중...
# 🔧 설정 로드 완료: config/default.toml
# ⚡ 마이크로 아비트래지 오케스트레이터 시작
# 📊 거래소 모니터링 시작: 4개 거래소
# 💡 아비트래지 기회: WETH/USDC (uniswap_v2 -> mock_binance) - 0.123% 수익 ($24.56)
# ✅ Mock 거래 실행 완료: 수익 $24.56
```

### Production 환경 실행

```bash
# 1. 환경 변수 확인
echo $ALCHEMY_API_KEY
echo $FLASHBOTS_PRIVATE_KEY

# 2. 설정 검증
cargo run --release -- --validate-config

# 3. Production 실행 (조심!)
API_MODE=real ./target/release/xcrack --config config/production.toml
```

---

## 모니터링

### 1. 실시간 로그 모니터링
```bash
# 실시간 로그 확인
tail -f xcrack.log

# 에러만 필터링
tail -f xcrack.log | grep ERROR

# 수익 정보만 필터링
tail -f xcrack.log | grep "수익\|profit"
```

### 2. 성능 메트릭 확인
```bash
# HTTP 메트릭 엔드포인트 (포트 9090)
curl http://localhost:9090/metrics

# JSON 형태 성능 정보
curl http://localhost:9090/performance
```

### 3. Discord/Telegram 알림 설정
```toml
[monitoring]
enable_discord_alerts = true
discord_webhook_url = "https://discord.com/api/webhooks/YOUR_WEBHOOK_HERE"
enable_telegram_alerts = false
profit_report_interval = "0 8 * * *"  # 매일 오전 8시
```

### 4. 중요 메트릭 모니터링

| 메트릭 | 정상 범위 | 경고 임계값 |
|--------|-----------|-------------|
| 응답시간 | < 100ms | > 200ms |
| 성공률 | > 95% | < 90% |
| 메모리 사용량 | < 500MB | > 800MB |
| 네트워크 지연 | < 50ms | > 100ms |

---

## 문제 해결

### 일반적인 문제들

#### 1. 컴파일 에러
```bash
# Rust 버전 확인
rustc --version

# 최신 버전으로 업데이트
rustup update

# 캐시 정리
cargo clean
cargo build --release
```

#### 2. 네트워크 연결 문제
```bash
# RPC 연결 테스트
curl -X POST -H "Content-Type: application/json" \
  --data '{"method":"eth_blockNumber","params":[],"id":1,"jsonrpc":"2.0"}' \
  https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY

# WebSocket 연결 테스트
wscat -c wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
```

#### 3. Mock 모드 문제
```bash
# Mock 서버 포트 충돌 확인
netstat -tlnp | grep :808

# Mock 모드 강제 실행
API_MODE=mock RUST_LOG=debug cargo run
```

#### 4. 설정 파일 문제
```bash
# 설정 파일 구문 검사
cargo run -- --validate-config

# 기본 설정으로 재설정
cp config/default.toml config/local.toml
```

### 로그 분석

#### 정상 실행 로그
```
🚀 xCrack MEV Searcher v1.2.0 시작
📡 네트워크 연결 완료: mainnet (chain_id: 1)
🔧 전략 로드 완료: 3개 전략 활성화
⚡ 성능 추적기 시작
📊 메트릭 서버 시작: http://localhost:9090
✅ 시스템 준비 완료
```

#### 에러 로그 예시
```
❌ ERROR: RPC 연결 실패 - Invalid API key
🔧 해결방법: ALCHEMY_API_KEY 환경 변수 확인

⚠️  WARN: 가스비 임계값 초과 - 현재: 150 gwei, 한계: 100 gwei  
🔧 해결방법: max_gas_price_gwei 설정 조정

❌ ERROR: Flashbots 제출 실패 - Bundle rejected
🔧 해결방법: 가스비 또는 우선순위 수수료 조정 필요
```

---

## 운영 가이드

### 1. 프로덕션 배포

#### systemd 서비스 설정
```bash
# 서비스 파일 생성
sudo tee /etc/systemd/system/xcrack.service > /dev/null << EOF
[Unit]
Description=xCrack MEV Searcher
After=network.target

[Service]
Type=simple
User=xcrack
WorkingDirectory=/opt/xcrack
Environment=API_MODE=real
Environment=RUST_LOG=info
ExecStart=/opt/xcrack/target/release/xcrack --config /opt/xcrack/config/production.toml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# 서비스 활성화
sudo systemctl enable xcrack
sudo systemctl start xcrack
sudo systemctl status xcrack
```

#### Docker 배포

```dockerfile
# Dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY .. .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/xcrack /usr/local/bin/
COPY --from=builder /app/config /opt/config
WORKDIR /opt
CMD ["xcrack", "--config", "/opt/config/production.toml"]
```

```bash
# Docker 빌드 및 실행
docker build -t xcrack:latest .
docker run -d --name xcrack \
  -e API_MODE=real \
  -e ALCHEMY_API_KEY=$ALCHEMY_API_KEY \
  -e FLASHBOTS_PRIVATE_KEY=$FLASHBOTS_PRIVATE_KEY \
  -p 9090:9090 \
  xcrack:latest
```

### 2. 백업 및 복구

#### 설정 백업
```bash
# 설정 백업
tar -czf xcrack-config-$(date +%Y%m%d).tar.gz config/ .env

# 로그 백업  
tar -czf xcrack-logs-$(date +%Y%m%d).tar.gz logs/

# 자동 백업 크론잡 설정
echo "0 2 * * * /opt/xcrack/scripts/backup.sh" | crontab -
```

#### 복구 절차
```bash
# 1. 서비스 중지
sudo systemctl stop xcrack

# 2. 설정 복구
tar -xzf xcrack-config-20250109.tar.gz

# 3. 서비스 재시작
sudo systemctl start xcrack
sudo systemctl status xcrack
```

### 3. 성능 튜닝

#### 시스템 최적화
```bash
# 네트워크 최적화
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
sysctl -p

# 파일 디스크립터 한계 증가
echo '* soft nofile 65536' >> /etc/security/limits.conf
echo '* hard nofile 65536' >> /etc/security/limits.conf
```

#### 애플리케이션 최적화
```toml
[performance]
max_concurrent_analysis = 20      # CPU 코어 수의 2-3배
batch_processing_interval = 50    # 더 빠른 배치 처리
cache_size = 20000               # 더 큰 캐시
```

### 4. 보안 가이드

#### 프라이빗 키 보안
```bash
# 환경 변수로만 관리 (파일에 저장 금지)
export FLASHBOTS_PRIVATE_KEY="0x..."

# 또는 시크릿 매니저 사용
aws ssm get-parameter --name "/xcrack/private_key" --with-decryption
```

#### 네트워크 보안
```bash
# 방화벽 설정 (메트릭 포트만 허용)
sudo ufw allow 9090/tcp
sudo ufw enable

# SSL 인증서 검증 강제
export RUSTLS_PLATFORM_VERIFIER=1
```

### 5. 업그레이드 절차

#### 무중단 업그레이드
```bash
# 1. 새 버전 빌드
git pull origin main
cargo build --release

# 2. 헬스체크 확인
curl http://localhost:9090/health

# 3. 그레이스풀 셧다운
sudo systemctl stop xcrack

# 4. 바이너리 교체
sudo cp target/release/xcrack /opt/xcrack/xcrack

# 5. 서비스 재시작
sudo systemctl start xcrack

# 6. 상태 확인
sudo systemctl status xcrack
curl http://localhost:9090/health
```

---

## 성능 기준 및 SLA

### 성능 목표
- **응답시간**: End-to-end < 100ms
- **처리량**: > 1000 transactions/sec
- **가용성**: > 99.9% uptime
- **정확성**: > 95% 수익성 예측 정확도

### 모니터링 대시보드
```bash
# Prometheus + Grafana 설치 (선택사항)
docker-compose up -d prometheus grafana

# 메트릭 확인
curl http://localhost:9090/metrics | grep xcrack_
```

---

## FAQ

### Q1: Mock 모드와 Real 모드의 차이점은?
**A1**: Mock 모드는 실제 네트워크 연결 없이 시뮬레이션하여 안전하게 테스트할 수 있으며, Real 모드는 실제 이더리움 네트워크와 상호작용하여 실제 거래를 실행합니다.

### Q2: 최소 자금 요구사항은?
**A2**: 테스트용으로는 자금이 불필요하며(Mock 모드), 실제 운영시에는 가스비 및 거래 자금으로 최소 0.1-1 ETH 권장합니다.

### Q3: 지원하는 거래소는?
**A3**: 현재 Uniswap V2, SushiSwap (DEX)와 Binance, Coinbase (CEX Mock) 지원하며, 추가 거래소는 설정을 통해 확장 가능합니다.

### Q4: 24시간 운영이 가능한가요?
**A4**: 네, 안정적인 24시간 운영을 위해 설계되었으며 자동 재시작, 에러 복구 기능을 포함합니다.

### Q5: 수익률은 어느 정도인가요?
**A5**: 시장 상황에 따라 다르며, Mock 모드에서 평균 0.1-0.5% 수익률을 시뮬레이션합니다. 실제 결과는 시장 조건에 따라 달라질 수 있습니다.

---

## 지원 및 문의

- **GitHub Issues**: [프로젝트 이슈 페이지]
- **Discord**: [커뮤니티 Discord 서버] 
- **이메일**: support@xcrack.dev
- **문서**: [전체 문서 사이트]

---

**⚠️ 면책 조항**: 이 소프트웨어는 교육 및 연구 목적으로 제공됩니다. 실제 거래시 손실 위험이 있으므로 충분히 테스트 후 사용하시기 바랍니다.