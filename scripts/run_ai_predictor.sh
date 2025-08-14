#!/bin/bash

# xCrack AI 예측 시스템 실행 스크립트

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
AI_DIR="$PROJECT_DIR/ai_predictor"

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🤖 xCrack AI 예측 시스템 시작${NC}"

# Python 가상환경 확인
check_python() {
    echo -e "${YELLOW}Python 환경 확인 중...${NC}"
    
    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}❌ Python3가 설치되지 않았습니다${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ Python3 발견: $(python3 --version)${NC}"
}

# 가상환경 생성/활성화
setup_venv() {
    VENV_DIR="$AI_DIR/venv"
    
    if [ ! -d "$VENV_DIR" ]; then
        echo -e "${YELLOW}가상환경 생성 중...${NC}"
        python3 -m venv "$VENV_DIR"
    fi
    
    echo -e "${YELLOW}가상환경 활성화...${NC}"
    source "$VENV_DIR/bin/activate"
    echo -e "${GREEN}✅ 가상환경 활성화 완료${NC}"
}

# 의존성 설치
install_dependencies() {
    echo -e "${YELLOW}의존성 설치 확인 중...${NC}"
    
    if [ -f "$AI_DIR/requirements.txt" ]; then
        pip install --upgrade pip
        pip install -r "$AI_DIR/requirements.txt"
        echo -e "${GREEN}✅ 의존성 설치 완료${NC}"
    else
        echo -e "${RED}❌ requirements.txt를 찾을 수 없습니다${NC}"
        exit 1
    fi
}

# 설정 파일 확인
check_config() {
    CONFIG_FILE="$AI_DIR/config/settings.yaml"
    if [ ! -f "$CONFIG_FILE" ]; then
        echo -e "${RED}❌ 설정 파일을 찾을 수 없습니다: $CONFIG_FILE${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ 설정 파일 확인 완료${NC}"
}

# GPU 지원 확인
check_gpu() {
    echo -e "${YELLOW}GPU 지원 확인 중...${NC}"
    
    if command -v nvidia-smi &> /dev/null; then
        echo -e "${GREEN}✅ NVIDIA GPU 감지${NC}"
        nvidia-smi --query-gpu=name,memory.total --format=csv,noheader
    else
        echo -e "${YELLOW}⚠️  GPU 미감지, CPU 모드로 실행${NC}"
    fi
}

# 로그 디렉토리 생성
setup_logs() {
    LOG_DIR="$AI_DIR/logs"
    mkdir -p "$LOG_DIR"
    echo -e "${GREEN}✅ 로그 디렉토리 준비: $LOG_DIR${NC}"
}

# AI 예측 시스템 실행
run_ai_system() {
    echo -e "${BLUE}🚀 AI 예측 시스템 실행 중...${NC}"
    
    cd "$AI_DIR"
    export PYTHONPATH="$AI_DIR/src:$PYTHONPATH"
    export CONFIG_PATH="$AI_DIR/config/settings.yaml"
    
    # 환경 변수 설정
    if [ -f "$AI_DIR/.env" ]; then
        export $(cat "$AI_DIR/.env" | xargs)
        echo -e "${GREEN}✅ 환경 변수 로드 완료${NC}"
    fi
    
    # 메인 실행
    python3 src/main.py "$@"
}

# 도움말
show_help() {
    echo "사용법: $0 [옵션]"
    echo ""
    echo "옵션:"
    echo "  -h, --help          이 도움말 표시"
    echo "  -v, --verbose       상세 로그 출력"
    echo "  -d, --dev          개발 모드 (디버그 활성화)"
    echo "  -c, --config FILE   설정 파일 지정"
    echo "  --gpu              GPU 강제 사용"
    echo "  --cpu              CPU 강제 사용"
    echo "  --install-only     의존성만 설치하고 종료"
    echo ""
    echo "예시:"
    echo "  $0                 # 기본 실행"
    echo "  $0 --dev          # 개발 모드"
    echo "  $0 --verbose      # 상세 로그"
}

# 메인 실행 로직
main() {
    # 인수 파싱
    VERBOSE=false
    DEV_MODE=false
    INSTALL_ONLY=false
    CONFIG_FILE=""
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -v|--verbose)
                VERBOSE=true
                shift
                ;;
            -d|--dev)
                DEV_MODE=true
                shift
                ;;
            -c|--config)
                CONFIG_FILE="$2"
                shift 2
                ;;
            --gpu)
                export FORCE_GPU=true
                shift
                ;;
            --cpu)
                export FORCE_CPU=true
                shift
                ;;
            --install-only)
                INSTALL_ONLY=true
                shift
                ;;
            *)
                echo -e "${RED}알 수 없는 옵션: $1${NC}"
                show_help
                exit 1
                ;;
        esac
    done
    
    # 환경 변수 설정
    if [ "$VERBOSE" = true ]; then
        export LOG_LEVEL=DEBUG
    fi
    
    if [ "$DEV_MODE" = true ]; then
        export DEV_MODE=true
        export LOG_LEVEL=DEBUG
    fi
    
    if [ -n "$CONFIG_FILE" ]; then
        export CONFIG_PATH="$CONFIG_FILE"
    fi
    
    # 실행 단계
    check_python
    setup_venv
    install_dependencies
    
    if [ "$INSTALL_ONLY" = true ]; then
        echo -e "${GREEN}✅ 의존성 설치 완료, 종료${NC}"
        exit 0
    fi
    
    check_config
    check_gpu
    setup_logs
    
    # 시스템 실행
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}  xCrack AI 예측 시스템 v1.0.0${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    
    run_ai_system
}

# 시그널 핸들링
cleanup() {
    echo -e "\n${YELLOW}⚠️  종료 신호 수신, 정리 중...${NC}"
    # 여기에 정리 로직 추가
    exit 0
}

trap cleanup INT TERM

# 메인 함수 실행
main "$@"