# ── Builder ───────────────────────────────────────────────────────────────────
FROM rust:slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       pkg-config \
       libssl-dev \
       libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .
RUN cargo build --release

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates \
       libssl3 \
       libsqlite3-0 \
       rsync \
       openssl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/agt-configure /usr/local/bin/
COPY --from=builder /build/target/release/agt-server    /usr/local/bin/
COPY --from=builder /build/target/release/agt-control   /usr/local/bin/