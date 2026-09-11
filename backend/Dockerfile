FROM rust:slim@sha256:bce1476d4be4d78b83705bc5f428b86d640eeeea33e9dadafbc037b5703a53bf AS build

RUN set -eux; \
    apt-get update; \
    apt-get install --yes --no-install-recommends protobuf-compiler;

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.8.3@sha256:70b45295d81035e3562e1bbf996a28d5fc55e1ccc5d7e3fff9f297d3b1a1359f

WORKDIR /app
COPY --chown=ubuntu:ubuntu --from=build /app/target/release/backend /backend
USER ubuntu
# EXPOSE 3055
ENTRYPOINT [ "/backend" ]
