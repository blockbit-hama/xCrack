#!/bin/bash

# xCrack 프로젝트 전체 실행 스크립트
# 백엔드(Rust) + 프론트엔드(Next.js) 동시 실행

set -e

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 프로젝트 루트 디렉토리
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/crack_front"

# 로그 파일
LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"
BACKEND_LOG="$LOG_DIR/backend.log"
FRONTEND_LOG="$LOG_DIR/frontend.log"

echo -e "${GREEN}=====================================${NC}"
echo -e "${GREEN}xCrack 프로젝트 시작${NC}"
echo -e "${GREEN}=====================================${NC}"

# 기존 프로세스 종료
echo -e "${YELLOW}기존 프로세스 확인 및 종료...${NC}"
pkill -f "cargo run" 2>/dev/null || true
pkill -f "next dev" 2>/dev/null || true
sleep 2

# 백엔드 시작 (포트 5000)
echo -e "${GREEN}[1/2] 백엔드 시작 (포트 5000)...${NC}"
cd "$PROJECT_ROOT"

# 백엔드 환경변수 설정
export API_MODE=mock
export RUST_LOG=info
export SERVER_PORT=5000

# 백엔드 실행 (백그라운드)
cargo run --quiet --bin searcher -- --strategies sandwich,liquidation,micro_arbitrage > "$BACKEND_LOG" 2>&1 &
BACKEND_PID=$!
echo -e "${GREEN}✓ 백엔드 시작됨 (PID: $BACKEND_PID, 포트: 5000)${NC}"
echo -e "  로그: $BACKEND_LOG"

# 프론트엔드 시작 (포트 5001)
echo -e "${GREEN}[2/2] 프론트엔드 시작 (포트 5001)...${NC}"
cd "$FRONTEND_DIR"

# package.json에서 포트 변경
if [ ! -f "package.json.backup" ]; then
  cp package.json package.json.backup
fi

# 포트 5001로 변경
sed -i.tmp 's/"dev": "next dev -p [0-9]*"/"dev": "next dev -p 5001"/' package.json
sed -i.tmp 's/"start": "next start -p [0-9]*"/"start": "next start -p 5001"/' package.json
rm -f package.json.tmp

# 프론트엔드 실행 (백그라운드)
npm run dev > "$FRONTEND_LOG" 2>&1 &
FRONTEND_PID=$!
echo -e "${GREEN}✓ 프론트엔드 시작됨 (PID: $FRONTEND_PID, 포트: 5001)${NC}"
echo -e "  로그: $FRONTEND_LOG"

# PID 저장
echo "$BACKEND_PID" > "$PROJECT_ROOT/.backend.pid"
echo "$FRONTEND_PID" > "$PROJECT_ROOT/.frontend.pid"

echo ""
echo -e "${GREEN}=====================================${NC}"
echo -e "${GREEN}서비스 시작 완료!${NC}"
echo -e "${GREEN}=====================================${NC}"
echo ""
echo -e "백엔드:      ${YELLOW}http://localhost:5000${NC}"
echo -e "프론트엔드:  ${YELLOW}http://localhost:5001${NC}"
echo ""
echo -e "로그 확인:"
echo -e "  백엔드:    tail -f $BACKEND_LOG"
echo -e "  프론트엔드: tail -f $FRONTEND_LOG"
echo ""
echo -e "종료하려면: ${YELLOW}./stop-all.sh${NC}"
echo ""

# 프로세스 모니터링
echo -e "${YELLOW}프로세스 상태 모니터링 중... (Ctrl+C로 종료)${NC}"
echo ""

while true; do
  # 백엔드 상태 확인
  if ! kill -0 $BACKEND_PID 2>/dev/null; then
    echo -e "${RED}⚠ 백엔드가 종료되었습니다!${NC}"
    break
  fi

  # 프론트엔드 상태 확인
  if ! kill -0 $FRONTEND_PID 2>/dev/null; then
    echo -e "${RED}⚠ 프론트엔드가 종료되었습니다!${NC}"
    break
  fi

  sleep 5
done

echo -e "${RED}서비스가 중단되었습니다. 로그를 확인하세요.${NC}"
