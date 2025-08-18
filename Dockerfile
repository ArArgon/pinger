# Multi-stage build for Rust pinger application
FROM rust:slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:trixie-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r pinger && useradd -r -g pinger pinger

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/pinger /app/pinger

# Change ownership to non-root user
RUN chown -R pinger:pinger /app

# Switch to non-root user
USER pinger

# Expose metrics port
EXPOSE 3000

# Health check using curl
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/metrics || exit 1

# Default command (user must provide config file)
CMD ["/app/pinger"]
