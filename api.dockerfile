ARG BUILDER_IMAGE
FROM ${BUILDER_IMAGE} as builder

FROM debian:bullseye-slim
RUN apt-get update && apt-get install libpq-dev -y && apt-get install ca-certificates -y
COPY --from=builder ./target/release/api ./api
COPY .env .env
ENTRYPOINT ["./api"]
