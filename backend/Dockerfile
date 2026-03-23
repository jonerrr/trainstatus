FROM rust:latest AS build

RUN apt update && apt install -y protobuf-compiler

WORKDIR /app
COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release

FROM ghcr.io/valhalla/valhalla:3.6.3

WORKDIR /app
COPY --from=build /app/target/release/backend /backend
EXPOSE 3055
ENTRYPOINT [ "/backend" ]
