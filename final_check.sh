#!/bin/bash

echo "🦀 xCrackRust 컴파일 체크 및 수정 사항 확인"
echo "🦀 xCrack 컴파일 체크 및 수정 사항 확인"
echo "============================================"

cd "/Users/pc-25-011/work/blockbit/xCrack/xCrackRust"
cd "/Users/pc-25-011/work/blockbit/xCrack"
cd "/Users/pc-25-011/work/blockbit/xCrack/xCrack"
cd "/Users/pc-25-011/work/blockbit/xCrack"

if [ ! -f "Cargo.toml" ]; then
    echo "❌ Cargo.toml not found!"
    exit 1
fi

echo "📁 프로젝트 위치: $(pwd)"
echo ""

echo "🔍 적용된 수정 사항:"
echo "- ✅ utils/mod.rs에서 중복 constants 모듈 제거"
echo "- ✅ types.rs에서 중복 constants 모듈 제거"  
echo "- ✅ constants 참조를 crate::constants로 수정"
echo "- ✅ strategies/utils.rs에 is_known_dex_router_internal 함수 추가"
echo "- ✅ mempool.rs에서 monitor 모듈 참조 제거"
echo "- ✅ utils.rs 파일을 utils_backup.rs로 이동 (충돌 방지)"
echo "- ✅ Strategy 트레이트에 Send + Sync 바운드 추가"
echo ""

echo "🔧 컴파일 체크 시작..."
echo "----------------------------------------"

# Run cargo check with detailed output
cargo check --message-format=human --color=always 2>&1 | tee compile_output.log

COMPILE_RESULT=$?

echo ""
echo "📊 컴파일 결과 분석:"
echo "----------------------------------------"

if [ $COMPILE_RESULT -eq 0 ]; then
    echo "✅ 컴파일 성공! 모든 오류가 수정되었습니다."
    echo ""
    echo "🎯 다음 단계 권장사항:"
    echo "- cargo test 실행으로 테스트 확인"
    echo "- cargo clippy 실행으로 코드 품질 검사"
    echo "- 핵심 비즈니스 로직 구현"
else
    echo "⚠️ 컴파일 오류가 남아있습니다."
    echo ""
    echo "🔍 남은 오류들:"
    grep -i "error" compile_output.log | head -10
    echo ""
    echo "💡 다음 수정이 필요할 수 있습니다:"
    echo "- Provider 타입 불일치 해결"
    echo "- 누락된 import 문 추가"
    echo "- async/await 관련 lifetime 이슈"
    echo "- 의존성 버전 호환성 문제"
fi

echo ""
echo "📝 상세한 컴파일 로그는 compile_output.log 파일을 확인하세요."
