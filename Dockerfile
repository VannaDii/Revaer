# syntax=docker/dockerfile:1.7
ARG LIBTORRENT_BUNDLE_DIR=/bundle

## Build stage ---------------------------------------------------------------
FROM rust:1.91.0-alpine3.20 AS builder
ARG LIBTORRENT_BUNDLE_DIR
WORKDIR /workspace

RUN apk add --no-cache \
        boost-dev \
        build-base \
        libtorrent-rasterbar-dev \
        musl-dev \
        openssl-dev \
        pkgconf

RUN rustup target add x86_64-unknown-linux-musl

COPY . .

ENV LIBTORRENT_BUNDLE_DIR=${LIBTORRENT_BUNDLE_DIR}
RUN cargo build --release --locked \
        --package revaer-app \
        --target x86_64-unknown-linux-musl

RUN cargo run --package revaer-api --bin generate_openapi

# Capture the libtorrent/boost/crypto libs used during the build so runtime is pinned.
RUN ./scripts/build-libtorrent-bundle.sh

## Runtime stage -------------------------------------------------------------
FROM alpine:3.20 AS runtime
ARG LIBTORRENT_BUNDLE_DIR

RUN addgroup -S revaer && adduser -S revaer -G revaer \
    && apk add --no-cache ca-certificates libstdc++ curl \
    && mkdir -p /app /data /config \
    && chown -R revaer:revaer /app /data /config

WORKDIR /app

COPY --from=builder --chown=revaer:revaer /workspace/target/x86_64-unknown-linux-musl/release/revaer-app /usr/local/bin/revaer-app
COPY --from=builder --chown=revaer:revaer /workspace/docs /app/docs
COPY --from=builder --chown=revaer:revaer /workspace/config /app/config
COPY --from=builder ${LIBTORRENT_BUNDLE_DIR}.tar.gz /tmp/libtorrent-bundle.tar.gz
RUN tar -xzf /tmp/libtorrent-bundle.tar.gz -C /usr/local && rm /tmp/libtorrent-bundle.tar.gz

VOLUME ["/data", "/config"]
ENV RUST_LOG=info
ENV LD_LIBRARY_PATH=/usr/local/lib

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:7070/health/full || exit 1

USER revaer
ENTRYPOINT ["/usr/local/bin/revaer-app"]
