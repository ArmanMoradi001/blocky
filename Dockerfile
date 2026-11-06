# Build stage: compile the Rust binary.
FROM rust:1.89-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

# Runtime stage: minimal image with just the binary.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/mini_block ./mini_block
EXPOSE 5000/tcp 15000/udp
CMD ["./mini_block"]
