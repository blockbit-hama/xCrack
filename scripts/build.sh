#!/bin/bash

set -e

echo "🦀 Building xCrack Rust MEV Searcher..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check Rust version
RUST_VERSION=$(rustc --version | cut -d' ' -f2)
echo "🔧 Using Rust version: $RUST_VERSION"

# Update Cargo index
echo "📦 Updating Cargo index..."
cargo update

# Build in release mode
echo "🔨 Building in release mode..."
cargo build --release

# Run tests
echo "🧪 Running tests..."
cargo test --release

# Create necessary directories
mkdir -p logs
mkdir -p data

# Copy default config if production config doesn't exist
if [ ! -f config/production.toml ]; then
    echo "📋 Creating production config from default..."
    cp config/default.toml config/production.toml
    echo "⚠️  Please edit config/production.toml with your settings before running!"
fi

echo "✅ Build completed successfully!"
echo ""
echo "📋 Next steps:"
echo "   1. Edit config/production.toml with your RPC URLs and private key"
echo "   2. Run: ./target/release/searcher --config config/production.toml"
echo "   3. Monitor logs in the logs/ directory"
echo ""
echo "🚀 Binary location: ./target/release/searcher"