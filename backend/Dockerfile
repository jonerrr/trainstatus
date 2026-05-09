FROM rust:slim@sha256:a6ed0cbc27f063c367aee8373f35fdf4dcf8be7596c4d19d6643e3252e514c2e AS build

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
