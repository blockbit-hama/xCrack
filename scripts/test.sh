#!/bin/bash

echo "🧪 Running xCrack Rust MEV Searcher Tests..."

# Set environment variables for testing
export RUST_BACKTRACE=1
export RUST_LOG=debug

echo "🦀 Running unit tests..."
if cargo test --lib 2>&1; then
    echo "✅ Unit tests passed!"
else
    echo "❌ Unit tests failed!"
    echo "Trying to run tests with more verbose output..."
    cargo test --lib -- --nocapture
fi

echo ""
echo "🔗 Running integration tests..."
if cargo test --test '*' 2>&1; then
    echo "✅ Integration tests passed!"
else
    echo "❌ Integration tests failed!"
fi

echo ""
echo "🔧 Checking compilation..."
if cargo check 2>&1; then
    echo "✅ Compilation check passed!"
else
    echo "❌ Compilation check failed!"
fi

echo ""
echo "🎨 Checking code format..."
if cargo fmt --check 2>&1; then
    echo "✅ Code format check passed!"
else
    echo "⚠️  Code format issues found. Run 'cargo fmt' to fix them."
fi

echo ""
echo "📝 Running clippy linter..."
if cargo clippy --all-targets --all-features -- -D warnings 2>&1; then
    echo "✅ Clippy check passed!"
else
    echo "⚠️  Clippy warnings found."
fi

echo ""
echo "🔒 Running security audit..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit 2>&1; then
        echo "✅ Security audit passed!"
    else
        echo "⚠️  Security vulnerabilities found!"
    fi
else
    echo "ℹ️  Install cargo-audit for security checking:"
    echo "   cargo install cargo-audit --locked"
fi

echo ""
echo "✅ All tests completed!"
