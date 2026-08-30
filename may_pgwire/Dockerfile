# Stage 1: Cargo Chef Planner
FROM lukemathwalker/cargo-chef:latest-rust-1.98-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Builder
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this layer is cached unless Cargo.toml/Cargo.lock changes
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and build the gateway service binary
COPY . .
RUN cargo build --release -p may_pgwire

# Stage 3: Runtime
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install certificates and update packages
# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder
COPY --from=builder /app/target/release/may_pgwire /usr/local/bin/may_pgwire

# Create a non-root user for execution
RUN useradd -m -u 10001 -U appuser && \
    chown -R appuser:appuser /app

USER appuser

# Expose PGWire Gateway port
EXPOSE 5432

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/may_pgwire"]
