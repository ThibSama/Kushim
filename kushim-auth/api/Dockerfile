FROM rust:1.94-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/kushim-auth-api /usr/local/bin/kushim-auth-api
EXPOSE 3002
CMD ["kushim-auth-api"]
