# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.77-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/

RUN cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/battlesnake /usr/local/bin/battlesnake

ENV PORT=8080
EXPOSE 8080

CMD ["battlesnake"]
