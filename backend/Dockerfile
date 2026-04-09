FROM rust:latest@sha256:e8e2bb5ff27ad3b369a4f667392464e6ec399cfe81c1230ae78edb1036b9bd74 AS build

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
