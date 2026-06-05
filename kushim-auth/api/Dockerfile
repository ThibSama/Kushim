FROM rust:1.94-bookworm AS builder

WORKDIR /workspace
COPY ./kushim-auth/api ./kushim-auth/api
WORKDIR /workspace/kushim-auth/api
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /workspace/kushim-auth/api/target/release/kushim-auth-api /usr/local/bin/kushim-auth-api
EXPOSE 3002
CMD ["kushim-auth-api"]
