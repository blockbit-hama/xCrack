#!/bin/bash

set -e

echo "🦀 Starting xCrack Rust MEV Searcher..."

# Check if binary exists
if [ ! -f "target/release/searcher" ]; then
    echo "❌ Binary not found. Please run ./scripts/build.sh first"
    exit 1
fi

# Check if config exists
if [ ! -f "config/production.toml" ]; then
    echo "❌ Production config not found. Please copy and edit config/default.toml"
    echo "   cp config/default.toml config/production.toml"
    exit 1
fi

# Validate config (basic check for private key)
if grep -q "your_private_key_here" config/production.toml; then
    echo "❌ Please configure your private key in config/production.toml"
    exit 1
fi

# Create logs directory
mkdir -p logs

# Set log file
LOG_FILE="logs/searcher-$(date +%Y%m%d-%H%M%S).log"

echo "📊 Starting MEV Searcher..."
echo "📋 Config: config/production.toml"
echo "📝 Logs: $LOG_FILE"
echo ""

# Start the searcher
RUST_LOG=info ./target/release/searcher \
    --config config/production.toml \
    2>&1 | tee "$LOG_FILE"