# syntax=docker/dockerfile:1
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
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release \
    && mkdir /out \
    && cp target/release/agt-configure target/release/agt-server target/release/agt-control /out/

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates \
       libssl3 \
       libsqlite3-0 \
       rsync \
       openssl \
       logrotate \
    && rm -rf /var/lib/apt/lists/*

RUN printf '/log/agt-server.log {\n    hourly\n    rotate 24\n    compress\n    missingok\n    notifempty\n    copytruncate\n}\n' > /etc/logrotate.d/agt-server

COPY --from=builder /out/agt-configure /usr/local/bin/
COPY --from=builder /out/agt-server    /usr/local/bin/
COPY --from=builder /out/agt-control   /usr/local/bin/