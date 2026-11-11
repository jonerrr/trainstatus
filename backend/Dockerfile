FROM rust:1-trixie@sha256:8a4e90c7946b74626488b1446f712c0fe0ed1a596cbd3d871684bc9c5433ae07 AS build

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

FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

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
