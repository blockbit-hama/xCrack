# Multi-stage Docker build for xCrack Rust MEV Searcher
FROM rust:1.75 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build dependencies (cached layer)
RUN mkdir src_backup && mv src/* src_backup/ && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src && mv src_backup/* src/ && rmdir src_backup

# Build application
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false searcher

# Create app directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/searcher /usr/local/bin/searcher

# Copy configuration
COPY config/ ./config/

# Create necessary directories
RUN mkdir -p logs data && \
    chown -R searcher:searcher /app /usr/local/bin/searcher

# Switch to app user
USER searcher

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9090/health || exit 1

# Expose metrics port
EXPOSE 9090

# Default command
CMD ["searcher", "--config", "config/default.toml"]