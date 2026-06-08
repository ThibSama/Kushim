FROM rust:1.94-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/kushim-worker /usr/local/bin/kushim-worker
CMD ["kushim-worker"]
