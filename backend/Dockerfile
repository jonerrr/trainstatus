FROM rust:slim@sha256:319f354c6f068b01c19292555033a0e6133b4249a893c479aec3b0b8ab2f660a AS build

RUN set -eux; \
    apt-get update; \
    apt-get install --yes --no-install-recommends protobuf-compiler;

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.6.3@sha256:2b19ea46551a9687b245022551183829d817fdee9b58c5e7b2adb6e422749c43

WORKDIR /app
COPY --from=build /app/target/release/backend /backend
# EXPOSE 3055
ENTRYPOINT [ "/backend" ]
