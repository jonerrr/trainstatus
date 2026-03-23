FROM rust:latest@sha256:f17e723020f87c1b4ac4ff6d73c9dfbb7d5cb978754c76641e47337d65f61e12 AS build

RUN apt update && apt install -y musl-tools musl-dev protobuf-compiler
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release --target x86_64-unknown-linux-musl

# TODO: replace this with valhalla image
FROM scratch

WORKDIR /app
COPY --from=build /app/target/x86_64-unknown-linux-musl/release/backend /backend
EXPOSE 3055
ENTRYPOINT [ "/backend" ]