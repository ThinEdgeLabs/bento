FROM rust:1.70 AS builder
COPY . .
RUN cargo build --release