FROM rust:slim@sha256:f47a8de237dcbb0b0ce1099901e60a89728e3d51f24e664b40e947171538ade7 AS build

RUN set -eux; \
    apt-get update; \
    apt-get install --yes --no-install-recommends protobuf-compiler;

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.9.0@sha256:511c095b8caf393dccceb8b519ec96b6f85a0166b2288ba014a8a748acc5a63c

WORKDIR /app
COPY --chown=ubuntu:ubuntu --from=build /app/target/release/backend /backend
USER ubuntu
# EXPOSE 3055
ENTRYPOINT [ "/backend" ]
