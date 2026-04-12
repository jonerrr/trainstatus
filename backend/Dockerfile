FROM rust:slim@sha256:cf09adf8c3ebaba10779e5c23ff7fe4df4cccdab8a91f199b0c142c53fef3e1a AS build

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
