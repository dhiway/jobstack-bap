# -------- Stage 1: Build --------
FROM debian:bookworm as build-deps

RUN apt-get update && apt-get install -y curl build-essential pkg-config libssl-dev ca-certificates

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true

COPY . .
RUN cargo build --release --bin bap-onest-lite

# -------- Stage 2: Runtime --------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libpq5 \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=build-deps /app/target/release/bap-onest-lite /app/bap-onest-lite
COPY config ./config

EXPOSE 3008

CMD ["./bap-onest-lite"]