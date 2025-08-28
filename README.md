# bap-onest-lite

A Beckn Application Platform (BAP) backend service that communicates with the Beckn network.

## Features

- REST API for job search, selection, application, and status
- Integrates with Beckn protocol
- Uses Axum, SQLx, Redis, and async Rust ecosystem

## Getting Started

### Prerequisites

- Rust (1.77+)
- PostgreSQL & Redis
- Cargo

### Build

```sh
cargo build --release
```

### Run

```sh
cargo run -- config/local.yaml
```

Replace `config/local.yaml` with your configuration file as needed.

## Docker

Build and run the container manually:

```sh
docker build -t bap-onest-lite .
docker run -p 3008:3008 bap-onest-lite ./bap-onest-lite config/local.yaml
```

Or use Docker Compose for multi-service setup:

```sh
docker compose build --no-cache
docker compose up -d
```

**Note:**  
The application is started in the container with:

```yaml
command: ["./bap-onest-lite", "config/local.yaml"]
```

You can update the config path as needed.

## Configuration

Configuration is loaded from the path you provide as the first argument.