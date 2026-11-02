FROM rust:1-trixie@sha256:09027f873d45b8ecd1dc86f1fa13b7a62b875c961f017a0e331267b350430137 AS build

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

FROM debian:trixie-slim@sha256:f6b08b09a05725f9431502d3b5714f5c4576e82c1a84fa1debe0c24a7e0c3ec4

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
