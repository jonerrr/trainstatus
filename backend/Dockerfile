FROM rust:1-trixie@sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc AS build

ARG MTA_API_URL
ENV MTA_API_URL=${MTA_API_URL}
ENV PROTOC=/usr/bin/protoc

RUN set -eux; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
    build-essential \
    cmake \
    pkg-config \
    clang \
    libboost-dev \
    libprotobuf-dev \
    protobuf-compiler \
    zlib1g-dev; \
    rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates \
    libprotobuf-lite32t64 \
    zlib1g \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /app && chown 10001:10001 /app

WORKDIR /app
COPY --from=build /src/target/release/backend /backend

USER 10001:10001

ENTRYPOINT [ "/backend" ]
