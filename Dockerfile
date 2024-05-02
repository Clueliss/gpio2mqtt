FROM rust:1-bullseye AS builder

RUN apt-get update && apt-get install -y cmake gcc

WORKDIR /usr/local/src/gpio2mqtt
COPY ./Cargo.toml ./
COPY ./Cargo.lock ./
COPY ./src ./src
RUN cargo build --release


FROM debian:bullseye-slim
ENV RUST_BACKTRACE=1
COPY --from=builder /usr/local/src/gpio2mqtt/target/release/gpio2mqtt /usr/local/bin/
RUN chmod +x /usr/local/bin/gpio2mqtt

ENTRYPOINT ["/usr/local/bin/gpio2mqtt"]
