# syntax=docker/dockerfile:1.7
## Build stage ---------------------------------------------------------------
FROM rust:alpine AS builder
WORKDIR /workspace

RUN apk add --no-cache \
        boost-dev \
        build-base \
        clang \
        libtorrent-rasterbar-dev \
        musl-dev \
        openssl-dev \
        pkgconf

COPY . .

# Install the toolchain components listed in rust-toolchain.toml for the host arch.
RUN rustup toolchain install stable --profile minimal \
        --component rustfmt --component clippy --component llvm-tools-preview \
    && rustup default stable

# Link dynamically against musl for third-party libs (libtorrent/openssl) on Alpine.
ENV RUSTFLAGS="-C target-feature=-crt-static"

RUN cargo build --release --locked --package revaer-app

RUN cargo run --package revaer-api --bin generate_openapi

## Runtime stage -------------------------------------------------------------
FROM alpine:latest AS runtime

RUN addgroup -S revaer && adduser -S revaer -G revaer \
    && apk add --no-cache ca-certificates libstdc++ curl openssl libtorrent-rasterbar \
    && mkdir -p /app /data /config \
    && chown -R revaer:revaer /app /data /config

WORKDIR /app

COPY --from=builder --chown=revaer:revaer /workspace/target/release/revaer-app /usr/local/bin/revaer-app
COPY --from=builder --chown=revaer:revaer /workspace/docs /app/docs
COPY --from=builder --chown=revaer:revaer /workspace/config /app/config

VOLUME ["/data", "/config"]
ENV RUST_LOG=info
ENV LD_LIBRARY_PATH=/usr/local/lib

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:7070/health/full || exit 1

USER revaer
ENTRYPOINT ["/usr/local/bin/revaer-app"]
