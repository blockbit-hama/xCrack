#!/bin/bash

# xCrack 통합 시스템 실행 스크립트
# Rust MEV 엔진 + Python AI 예측 시스템 동시 실행

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# 로그 파일
AI_LOG="$PROJECT_DIR/logs/ai_predictor.log"
RUST_LOG="$PROJECT_DIR/logs/xcrack.log"

echo -e "${BLUE}"
cat << "EOF"
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║  🤖🦀 xCrack AI-Powered MEV 통합 시스템                          ║
║                                                                  ║
║  🔥 AI 예측 + MEV 전략 + 고성능 실행                             ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
EOF
echo -e "${NC}"

# 도움말
show_help() {
    echo "사용법: $0 [옵션]"
    echo ""
    echo "옵션:"
    echo "  -h, --help          이 도움말 표시"
    echo "  -m, --mock          Mock 모드 실행 (기본값)"
    echo "  -p, --production    프로덕션 모드 실행"
    echo "  -d, --dev          개발 모드 (상세 로그)"
    echo "  -v, --verbose       상세 출력"
    echo "  --ai-only          AI 시스템만 실행"
    echo "  --rust-only        Rust 엔진만 실행"
    echo "  --stop             실행 중인 시스템 중지"
    echo "  --status           시스템 상태 확인"
    echo ""
    echo "예시:"
    echo "  $0                 # Mock 모드 통합 실행"
    echo "  $0 --production    # 프로덕션 모드 실행"
    echo "  $0 --dev          # 개발 모드"
    echo "  $0 --stop         # 시스템 중지"
}

# 로그 디렉토리 생성
setup_logs() {
    mkdir -p "$PROJECT_DIR/logs"
    echo -e "${GREEN}✅ 로그 디렉토리 준비: $PROJECT_DIR/logs${NC}"
}

# 의존성 확인
check_dependencies() {
    echo -e "${YELLOW}의존성 확인 중...${NC}"
    
    # Rust 확인
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Rust/Cargo가 설치되지 않았습니다${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Rust: $(rustc --version | cut -d' ' -f2)${NC}"
    
    # Python 확인
    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}❌ Python3가 설치되지 않았습니다${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Python: $(python3 --version)${NC}"
    
    # AI 의존성 확인
    if [ ! -f "$PROJECT_DIR/ai_predictor/requirements.txt" ]; then
        echo -e "${RED}❌ AI 예측 시스템 설정 누락${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ AI 시스템 설정 확인${NC}"
}

# 시스템 빌드
build_system() {
    echo -e "${YELLOW}시스템 빌드 중...${NC}"
    
    # Rust 빌드
    cd "$PROJECT_DIR"
    if [ "$DEV_MODE" = true ]; then
        cargo build
    else
        cargo build --release
    fi
    echo -e "${GREEN}✅ Rust 엔진 빌드 완료${NC}"
    
    # Python 의존성 설치
    cd "$PROJECT_DIR/ai_predictor"
    if [ ! -d "venv" ]; then
        echo -e "${YELLOW}Python 가상환경 생성 중...${NC}"
        python3 -m venv venv
    fi
    
    source venv/bin/activate
    pip install --upgrade pip > /dev/null 2>&1
    pip install -r requirements.txt > /dev/null 2>&1
    echo -e "${GREEN}✅ AI 시스템 의존성 설치 완료${NC}"
}

# AI 예측 시스템 시작
start_ai_system() {
    echo -e "${PURPLE}🤖 AI 예측 시스템 시작 중...${NC}"
    
    cd "$PROJECT_DIR"
    
    if [ "$BACKGROUND" = true ]; then
        # 백그라운드 실행
        ./scripts/run_ai_predictor.sh $AI_FLAGS > "$AI_LOG" 2>&1 &
        AI_PID=$!
        echo $AI_PID > "$PROJECT_DIR/logs/ai_predictor.pid"
        echo -e "${GREEN}✅ AI 시스템 백그라운드 실행 (PID: $AI_PID)${NC}"
    else
        # 포그라운드 실행
        ./scripts/run_ai_predictor.sh $AI_FLAGS
    fi
}

# Rust MEV 엔진 시작
start_rust_engine() {
    echo -e "${PURPLE}🦀 Rust MEV 엔진 시작 중...${NC}"
    
    cd "$PROJECT_DIR"
    
    # 바이너리 경로 결정
    if [ "$DEV_MODE" = true ]; then
        BINARY="./target/debug/xcrack"
    else
        BINARY="./target/release/xcrack"
    fi
    
    if [ ! -f "$BINARY" ]; then
        echo -e "${RED}❌ 바이너리를 찾을 수 없습니다: $BINARY${NC}"
        echo -e "${YELLOW}먼저 시스템을 빌드하세요: $0 --build${NC}"
        exit 1
    fi
    
    # 환경 변수 설정
    export $RUST_ENV_VARS
    
    if [ "$BACKGROUND" = true ]; then
        # 백그라운드 실행
        $BINARY $RUST_FLAGS > "$RUST_LOG" 2>&1 &
        RUST_PID=$!
        echo $RUST_PID > "$PROJECT_DIR/logs/xcrack.pid"
        echo -e "${GREEN}✅ Rust 엔진 백그라운드 실행 (PID: $RUST_PID)${NC}"
    else
        # 포그라운드 실행
        $BINARY $RUST_FLAGS
    fi
}

# 시스템 상태 확인
check_status() {
    echo -e "${BLUE}📊 시스템 상태 확인${NC}"
    
    # AI 시스템 상태
    if [ -f "$PROJECT_DIR/logs/ai_predictor.pid" ]; then
        AI_PID=$(cat "$PROJECT_DIR/logs/ai_predictor.pid")
        if ps -p $AI_PID > /dev/null 2>&1; then
            echo -e "${GREEN}🤖 AI 예측 시스템: 실행 중 (PID: $AI_PID)${NC}"
        else
            echo -e "${RED}🤖 AI 예측 시스템: 중지됨${NC}"
            rm -f "$PROJECT_DIR/logs/ai_predictor.pid"
        fi
    else
        echo -e "${RED}🤖 AI 예측 시스템: 실행되지 않음${NC}"
    fi
    
    # Rust 엔진 상태
    if [ -f "$PROJECT_DIR/logs/xcrack.pid" ]; then
        RUST_PID=$(cat "$PROJECT_DIR/logs/xcrack.pid")
        if ps -p $RUST_PID > /dev/null 2>&1; then
            echo -e "${GREEN}🦀 Rust MEV 엔진: 실행 중 (PID: $RUST_PID)${NC}"
        else
            echo -e "${RED}🦀 Rust MEV 엔진: 중지됨${NC}"
            rm -f "$PROJECT_DIR/logs/xcrack.pid"
        fi
    else
        echo -e "${RED}🦀 Rust MEV 엔진: 실행되지 않음${NC}"
    fi
    
    # 로그 파일 확인
    echo -e "${BLUE}📝 로그 파일:${NC}"
    if [ -f "$AI_LOG" ]; then
        echo -e "  AI: $AI_LOG ($(wc -l < "$AI_LOG") 라인)"
    fi
    if [ -f "$RUST_LOG" ]; then
        echo -e "  Rust: $RUST_LOG ($(wc -l < "$RUST_LOG") 라인)"
    fi
}

# 시스템 중지
stop_system() {
    echo -e "${YELLOW}🛑 시스템 중지 중...${NC}"
    
    # AI 시스템 중지
    if [ -f "$PROJECT_DIR/logs/ai_predictor.pid" ]; then
        AI_PID=$(cat "$PROJECT_DIR/logs/ai_predictor.pid")
        if ps -p $AI_PID > /dev/null 2>&1; then
            kill -TERM $AI_PID
            echo -e "${GREEN}🤖 AI 예측 시스템 중지됨${NC}"
        fi
        rm -f "$PROJECT_DIR/logs/ai_predictor.pid"
    fi
    
    # Rust 엔진 중지
    if [ -f "$PROJECT_DIR/logs/xcrack.pid" ]; then
        RUST_PID=$(cat "$PROJECT_DIR/logs/xcrack.pid")
        if ps -p $RUST_PID > /dev/null 2>&1; then
            kill -TERM $RUST_PID
            echo -e "${GREEN}🦀 Rust MEV 엔진 중지됨${NC}"
        fi
        rm -f "$PROJECT_DIR/logs/xcrack.pid"
    fi
    
    echo -e "${GREEN}✅ 시스템 중지 완료${NC}"
}

# 로그 모니터링
monitor_logs() {
    echo -e "${BLUE}📊 실시간 로그 모니터링 (Ctrl+C로 종료)${NC}"
    
    if [ -f "$AI_LOG" ] && [ -f "$RUST_LOG" ]; then
        # 두 로그를 동시에 모니터링
        tail -f "$AI_LOG" "$RUST_LOG"
    elif [ -f "$AI_LOG" ]; then
        tail -f "$AI_LOG"
    elif [ -f "$RUST_LOG" ]; then
        tail -f "$RUST_LOG"
    else
        echo -e "${YELLOW}⚠️  로그 파일이 없습니다${NC}"
    fi
}

# 통합 실행
run_integrated() {
    echo -e "${BLUE}🚀 통합 시스템 실행 시작${NC}"
    
    setup_logs
    check_dependencies
    build_system
    
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}  시스템 시작 중...${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    
    # AI 시스템 먼저 시작
    BACKGROUND=true start_ai_system
    sleep 3
    
    # Rust 엔진 시작
    BACKGROUND=true start_rust_engine
    sleep 2
    
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${GREEN}✅ 통합 시스템 실행 완료${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    
    check_status
    
    echo -e "${YELLOW}📝 로그 모니터링을 시작하려면: $0 --logs${NC}"
    echo -e "${YELLOW}🛑 시스템을 중지하려면: $0 --stop${NC}"
}

# 시그널 핸들링
cleanup() {
    echo -e "\n${YELLOW}⚠️  종료 신호 수신, 시스템 정리 중...${NC}"
    stop_system
    exit 0
}

trap cleanup INT TERM

# 메인 함수
main() {
    # 기본 설정
    MOCK_MODE=true
    DEV_MODE=false
    VERBOSE=false
    AI_ONLY=false
    RUST_ONLY=false
    BACKGROUND=false
    
    # 인수 파싱
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -m|--mock)
                MOCK_MODE=true
                shift
                ;;
            -p|--production)
                MOCK_MODE=false
                shift
                ;;
            -d|--dev)
                DEV_MODE=true
                VERBOSE=true
                shift
                ;;
            -v|--verbose)
                VERBOSE=true
                shift
                ;;
            --ai-only)
                AI_ONLY=true
                shift
                ;;
            --rust-only)
                RUST_ONLY=true
                shift
                ;;
            --stop)
                stop_system
                exit 0
                ;;
            --status)
                check_status
                exit 0
                ;;
            --logs)
                monitor_logs
                exit 0
                ;;
            --build)
                setup_logs
                check_dependencies
                build_system
                exit 0
                ;;
            *)
                echo -e "${RED}알 수 없는 옵션: $1${NC}"
                show_help
                exit 1
                ;;
        esac
    done
    
    # 환경 변수 설정
    if [ "$MOCK_MODE" = true ]; then
        export API_MODE=mock
        RUST_ENV_VARS="API_MODE=mock"
        echo -e "${YELLOW}🎭 Mock 모드 활성화${NC}"
    else
        export API_MODE=real
        RUST_ENV_VARS="API_MODE=real"
        echo -e "${GREEN}🌐 프로덕션 모드 활성화${NC}"
    fi
    
    # 플래그 설정
    AI_FLAGS=""
    RUST_FLAGS="--strategies sandwich,liquidation,predictive"
    
    if [ "$DEV_MODE" = true ]; then
        AI_FLAGS="$AI_FLAGS --dev"
        RUST_FLAGS="$RUST_FLAGS --dev --log-level debug"
        export LOG_LEVEL=DEBUG
    fi
    
    if [ "$VERBOSE" = true ]; then
        AI_FLAGS="$AI_FLAGS --verbose"
        RUST_FLAGS="$RUST_FLAGS --log-level debug"
    fi
    
    # 실행 모드에 따른 분기
    if [ "$AI_ONLY" = true ]; then
        setup_logs
        start_ai_system
    elif [ "$RUST_ONLY" = true ]; then
        setup_logs
        start_rust_engine
    else
        run_integrated
    fi
}

# 메인 함수 실행
main "$@"