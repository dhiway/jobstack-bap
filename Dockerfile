# -------- Stage 1: Build --------
FROM rust:1.87 as build-deps

WORKDIR /app

RUN apt-get update && apt-get install -y \
    git \
    cmake \
    g++ \
    clang \
    pkg-config \
    libssl-dev \
    libpq-dev \
    libopenblas-dev \
    libgflags-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN git clone --depth 1 https://github.com/facebookresearch/faiss.git /tmp/faiss && \
    cmake -B /tmp/faiss/build \
      -DFAISS_ENABLE_C_API=ON \
      -DFAISS_ENABLE_PYTHON=OFF \
      -DBUILD_SHARED_LIBS=ON \
      -DFAISS_ENABLE_GPU=OFF \
      /tmp/faiss && \
    cmake --build /tmp/faiss/build -j"$(nproc)" && \
    cmake --install /tmp/faiss/build

ENV LIBRARY_PATH=/usr/local/lib
ENV LD_LIBRARY_PATH=/usr/local/lib
ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true

COPY . .

RUN cargo build --release --bin bap-onest-lite


FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libpq5 \
    libssl3 \
    libopenblas0 \
    libgomp1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=build-deps /usr/local/lib/libfaiss.so* /usr/local/lib/
COPY --from=build-deps /usr/local/lib/libfaiss_c.so* /usr/local/lib/
COPY --from=build-deps /app/target/release/bap-onest-lite /app/bap-onest-lite
COPY config ./config

ENV LD_LIBRARY_PATH=/usr/local/lib

EXPOSE 3008

CMD ["./bap-onest-lite"]