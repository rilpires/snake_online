FROM rust:1.70 as builder

WORKDIR /usr/src/snake_online
COPY . .

RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/snake_online/target/release/snake_online /app/
COPY --from=builder /usr/src/snake_online/public /app/public/

EXPOSE 8080

CMD ["./snake_online"]
