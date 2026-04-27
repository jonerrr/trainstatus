FROM rust:slim@sha256:81099830a1e1d244607b9a7a30f3ff6ecadc52134a933b4635faba24f52840c9 AS build

RUN set -eux; \
    apt-get update; \
    apt-get install --yes --no-install-recommends protobuf-compiler;

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.6.3@sha256:2b19ea46551a9687b245022551183829d817fdee9b58c5e7b2adb6e422749c43

WORKDIR /app
COPY --chown=ubuntu:ubuntu --from=build /app/target/release/backend /backend
USER ubuntu
# EXPOSE 3055
ENTRYPOINT [ "/backend" ]
