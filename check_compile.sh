#!/bin/bash

echo "🔍 Checking Rust compilation..."
cd "/Users/pc-25-011/work/blockbit/xCrack/xCrack"
cd "/Users/pc-25-011/work/blockbit/xCrack"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Cargo.toml not found. Are we in the right directory?"
    exit 1
fi

echo "📁 Current directory: $(pwd)"
echo "📋 Running cargo check..."

# Run cargo check
cargo check 2>&1

echo "✅ Cargo check completed"
