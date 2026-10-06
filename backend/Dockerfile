FROM rust:1-trixie@sha256:3745c050d12adc738eff16ebfc81ed044bfb2cc27c6828850ff1666beb1c7a49 AS build

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

ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:trixie-slim@sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates \
    libprotobuf-lite32t64 \
    zlib1g \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /src/target/release/backend /backend

ENTRYPOINT [ "/backend" ]
