FROM rust:1.70 AS builder
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install libpq-dev -y
COPY --from=builder ./target/release/indexer ./target/release/indexer
COPY --from=builder ./target/release/api ./target/release/api
CMD ["./target/release/indexer"]