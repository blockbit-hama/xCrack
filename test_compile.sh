#!/bin/bash

echo "🦀 xCrackRust MEV 서쳐 컴파일 테스트"
echo "=================================="

cd /Users/pc-25-011/work/blockbit/xCrack/xCrackRust
cd /Users/pc-25-011/work/blockbit/xCrack

echo "📋 Cargo 체크 실행 중..."
cargo check

echo ""
echo "🔧 컴파일 테스트 실행 중..."
cargo build

echo ""
echo "🧪 테스트 실행 중..."
cargo test

echo ""
echo "✅ 컴파일 테스트 완료!"
