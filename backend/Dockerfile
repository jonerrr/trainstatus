FROM rust:latest AS build

RUN apt update && apt install -y musl-tools musl-dev protobuf-compiler
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM scratch

WORKDIR /app
COPY --from=build /app/target/x86_64-unknown-linux-musl/release/backend /backend
EXPOSE 3055
ENTRYPOINT [ "/backend" ]