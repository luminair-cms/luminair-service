# Stage 1: Build stage
FROM rust:1.96.1-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/luminair-service

# Copy manifests for caching
COPY Cargo.toml Cargo.lock ./
COPY src/common/Cargo.toml src/common/Cargo.toml
COPY src/migration/Cargo.toml src/migration/Cargo.toml
COPY src/service/Cargo.toml src/service/Cargo.toml

# Dummy build to cache dependencies
RUN mkdir -p src/common/src src/migration/src src/service/src && \
    echo "fn main() {}" > src/migration/src/main.rs && \
    echo "fn main() {}" > src/service/src/main.rs && \
    echo "pub fn dummy() {}" > src/common/src/lib.rs && \
    cargo build --release && \
    rm -rf src

# Copy real source code
COPY src src

# Build both service and migration binaries
RUN touch src/common/src/lib.rs src/migration/src/main.rs src/service/src/main.rs && \
    cargo build --release

# Stage 2: Runtime base image (common dependencies)
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY config /app/config

# Stage 3: Service image
FROM runtime-base AS service
COPY --from=builder /usr/src/luminair-service/target/release/luminair-service /app/luminair-service
EXPOSE 8080
CMD ["/app/luminair-service"]

# Stage 4: Migration CLI image
FROM runtime-base AS migration
COPY --from=builder /usr/src/luminair-service/target/release/migration /app/migration
CMD ["/app/migration"]
