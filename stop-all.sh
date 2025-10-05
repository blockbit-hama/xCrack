#!/bin/bash

# xCrack 프로젝트 전체 종료 스크립트

set -e

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo -e "${YELLOW}=====================================${NC}"
echo -e "${YELLOW}xCrack 프로젝트 종료${NC}"
echo -e "${YELLOW}=====================================${NC}"

# PID 파일에서 프로세스 종료
if [ -f "$PROJECT_ROOT/.backend.pid" ]; then
  BACKEND_PID=$(cat "$PROJECT_ROOT/.backend.pid")
  if kill -0 $BACKEND_PID 2>/dev/null; then
    echo -e "${YELLOW}백엔드 종료 중... (PID: $BACKEND_PID)${NC}"
    kill $BACKEND_PID
    echo -e "${GREEN}✓ 백엔드 종료됨${NC}"
  fi
  rm -f "$PROJECT_ROOT/.backend.pid"
fi

if [ -f "$PROJECT_ROOT/.frontend.pid" ]; then
  FRONTEND_PID=$(cat "$PROJECT_ROOT/.frontend.pid")
  if kill -0 $FRONTEND_PID 2>/dev/null; then
    echo -e "${YELLOW}프론트엔드 종료 중... (PID: $FRONTEND_PID)${NC}"
    kill $FRONTEND_PID
    echo -e "${GREEN}✓ 프론트엔드 종료됨${NC}"
  fi
  rm -f "$PROJECT_ROOT/.frontend.pid"
fi

# 추가로 관련 프로세스 정리
echo -e "${YELLOW}관련 프로세스 정리 중...${NC}"
pkill -f "cargo run" 2>/dev/null || true
pkill -f "next dev" 2>/dev/null || true

# package.json 복원
if [ -f "$PROJECT_ROOT/crack_front/package.json.backup" ]; then
  echo -e "${YELLOW}package.json 복원 중...${NC}"
  cp "$PROJECT_ROOT/crack_front/package.json.backup" "$PROJECT_ROOT/crack_front/package.json"
  echo -e "${GREEN}✓ package.json 복원됨${NC}"
fi

echo ""
echo -e "${GREEN}=====================================${NC}"
echo -e "${GREEN}모든 서비스가 종료되었습니다.${NC}"
echo -e "${GREEN}=====================================${NC}"
