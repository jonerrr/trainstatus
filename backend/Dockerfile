FROM rust:slim@sha256:81099830a1e1d244607b9a7a30f3ff6ecadc52134a933b4635faba24f52840c9 AS build

RUN set -eux; \
    apt-get update; \
    apt-get install --yes --no-install-recommends protobuf-compiler;

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.7.0@sha256:c19b052fa132474da719e0e931007cd3931f25c43890f88acf270ba837fdc89f

WORKDIR /app
COPY --chown=ubuntu:ubuntu --from=build /app/target/release/backend /backend
USER ubuntu
# EXPOSE 3055
ENTRYPOINT [ "/backend" ]
