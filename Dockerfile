FROM rust:1.98-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev git && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim

RUN apt-get update && apt-get install -y ca-certificates git libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/history-flow /usr/local/bin/

ENV PORT=8080
EXPOSE 8080

CMD ["sh", "-c", "history-flow serve --addr 0.0.0.0:$PORT"]
