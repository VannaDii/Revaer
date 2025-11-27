# syntax=docker/dockerfile:1.7

## Build stage ---------------------------------------------------------------
FROM rust:1.91.0-alpine3.20 AS builder
WORKDIR /workspace

RUN apk add --no-cache \
        boost-dev \
        build-base \
        libtorrent-rasterbar-dev \
        musl-dev \
        openssl-dev \
        pkgconf

RUN rustup target add x86_64-unknown-linux-musl

RUN mkdir -p /bundle/lib /bundle/include

COPY . .

ENV LIBTORRENT_BUNDLE_DIR=/bundle

RUN cargo build --release --locked \
        --package revaer-app \
        --target x86_64-unknown-linux-musl

RUN cargo run --package revaer-api --bin generate_openapi

# Capture the libtorrent/boost/crypto libs used during the build so runtime is pinned.
RUN cp -a /usr/include/libtorrent /bundle/include/ \
    && cp -a /usr/lib/libtorrent-rasterbar.so* /bundle/lib/ \
    && cp -a /usr/lib/libboost_system.so* /bundle/lib/ \
    && cp -a /usr/lib/libssl.so* /bundle/lib/ \
    && cp -a /usr/lib/libcrypto.so* /bundle/lib/ \
    && cp -a /lib/libgcc_s.so* /bundle/lib/ \
    && cp -a /usr/lib/libstdc++.so* /bundle/lib/

## Runtime stage -------------------------------------------------------------
FROM alpine:3.20 AS runtime

RUN addgroup -S revaer && adduser -S revaer -G revaer \
    && apk add --no-cache ca-certificates libstdc++ curl \
    && mkdir -p /app /data /config \
    && chown -R revaer:revaer /app /data /config

WORKDIR /app

COPY --from=builder --chown=revaer:revaer /workspace/target/x86_64-unknown-linux-musl/release/revaer-app /usr/local/bin/revaer-app
COPY --from=builder --chown=revaer:revaer /workspace/docs /app/docs
COPY --from=builder --chown=revaer:revaer /workspace/config /app/config
COPY --from=builder /bundle /usr/local

VOLUME ["/data", "/config"]
ENV RUST_LOG=info
ENV LD_LIBRARY_PATH=/usr/local/lib

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:7070/health/full || exit 1

USER revaer
ENTRYPOINT ["/usr/local/bin/revaer-app"]
