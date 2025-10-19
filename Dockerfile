# syntax=docker/dockerfile:1.7

## Build stage ---------------------------------------------------------------
FROM rust:1.85-alpine3.20 AS builder
WORKDIR /workspace

RUN apk add --no-cache \
        build-base \
        musl-dev \
        pkgconfig \
        openssl-dev

RUN rustup target add x86_64-unknown-linux-musl

COPY . .

RUN cargo build --release --locked \
        --package revaer-app \
        --target x86_64-unknown-linux-musl

RUN cargo run --package revaer-api --bin generate_openapi

## Runtime stage -------------------------------------------------------------
FROM alpine:3.20 AS runtime

RUN addgroup -S revaer && adduser -S revaer -G revaer \
    && apk add --no-cache ca-certificates curl \
    && mkdir -p /app /data /config \
    && chown -R revaer:revaer /app /data /config

WORKDIR /app

COPY --from=builder --chown=revaer:revaer /workspace/target/x86_64-unknown-linux-musl/release/revaer-app /usr/local/bin/revaer-app
COPY --from=builder --chown=revaer:revaer /workspace/docs /app/docs
COPY --from=builder --chown=revaer:revaer /workspace/config /app/config

VOLUME ["/data", "/config"]
ENV RUST_LOG=info

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:7070/health/full || exit 1

USER revaer
ENTRYPOINT ["/usr/local/bin/revaer-app"]
