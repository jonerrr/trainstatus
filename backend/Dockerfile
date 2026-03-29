FROM rust:latest@sha256:f2a0f2b3529c9bbbf5479d131611451a3cc3956d9a11374d6d4ba96f059c1dce AS build

RUN apt update && apt install -y protobuf-compiler

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.6.3@sha256:2b19ea46551a9687b245022551183829d817fdee9b58c5e7b2adb6e422749c43

WORKDIR /app
COPY --from=build /app/target/release/backend /backend
EXPOSE 3055
ENTRYPOINT [ "/backend" ]
