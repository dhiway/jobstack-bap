# BAP Onest Lite

A Beckn Application Provider (BAP) backend service that acts as middleware between the **Jobstack application** and the **Beckn network**. It handles protocol translation, job search, profile synchronization, and job-profile matching.

## Architecture Overview

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│  Jobstack App   │──────│  BAP Onest Lite │──────│  Beckn Network  │
│   (Frontend)    │      │   (This App)    │      │   (BPP/BC)      │
└─────────────────┘      └─────────────────┘      └─────────────────┘
                                │
                                ├── PostgreSQL (Jobs, Profiles, Applications)
                                ├── Redis (Caching, Pub/Sub)
                                ├── FAISS (Vector Search)
                                └── GCP Gemini (Embeddings)
```

### What This App Does

1. **Beckn Protocol Communication**: Translates between REST API and Beckn protocol format
2. **Job Search**: Searches for jobs from BPP (Beckn Provider Platform) via Jobstack
3. **Profile Sync**: Fetches and stores candidate profiles from Jobstack seeker service
4. **Match Scoring**: Computes job-profile compatibility scores using embeddings
5. **Job Applications**: Handles job application submissions
6. **Notifications**: Sends WhatsApp notifications for high-match candidates
7. **Vector Search**: Uses FAISS for semantic job matching

## Features

- RESTful API for job search, selection, application, and status tracking
- Beckn protocol integration (on_search, on_select, on_init, on_confirm, on_status)
- PostgreSQL database for persistent storage
- Redis for caching and pub/sub messaging
- FAISS vector search for semantic job matching
- GCP Gemini embeddings for candidate-job matching
- Background cron jobs for data synchronization
- WhatsApp notifications for high-match candidates

## Prerequisites

- **Rust** 1.77+ (with cargo)
- **PostgreSQL** 14+
- **Redis** 6+
- **Docker & Docker Compose** (optional, for containerized setup)
- **FAISS** (optional, for vector search on macOS)

### For macOS FAISS Development

```bash
brew install faiss
export LIBRARY_PATH="/opt/homebrew/lib:$LIBRARY_PATH"
export DYLD_LIBRARY_PATH="/opt/homebrew/lib:$DYLD_LIBRARY_PATH"
```

## Installation

### 1. Clone the Repository

```bash
git clone <repository-url>
cd bap-onest-lite
```

### 2. Install Rust Dependencies

```bash
cargo build --release
```

### 3. Setup Environment

```bash
# Copy environment template
cp .env.example .env

# Edit .env with your database credentials
```

### 4. Setup Configuration

```bash
# Copy example configuration
cp config/example.yaml config/local.yaml

# Edit config/local.yaml with your settings
```

### 5. Run Database Migrations

```bash
# Make migration script executable
chmod +x migrate.sh

# Run migrations
./migrate.sh
```

## Configuration

### Configuration Files

| File | Purpose |
|------|---------|
| `config/local.yaml` | Local development configuration |
| `config/dev.yaml` | Development environment |
| `config/prod.yaml` | Production environment |
| `config/example.yaml` | Template with all options |

### Configuration Sections

The YAML configuration includes:

- **http**: Server address and port
- **bap**: BAP identifier, URIs, domain, version, TTL
- **redis**: Redis connection URL
- **db**: PostgreSQL connection string
- **cache**: TTL and throttle settings
- **cron**: Scheduled task intervals
- **gcp**: Google Gemini embedding config
- **services**: External service URLs (seeker, notification, geocoding)
- **bpp**: BPP configuration for profiles
- **auth**: API key authentication
- **match_score**: Match scoring configuration

### Environment Variables

See `.env.example` for all required environment variables.

## Running the Application

### Local Development

```bash
cargo run -- config/local.yaml
```

The server will start on `http://0.0.0.0:3008` (or configured port).

### Using Docker Compose

```bash
# Build and start all services
docker compose build --no-cache
docker compose up -d

# View logs
docker compose logs -f bap-onest-lite

# Stop services
docker compose down
```

### Docker Compose Services

- **bap-postgres**: PostgreSQL database
- **bap-redis**: Redis cache
- **bap-onest-lite**: This application

## API Endpoints

### Job Search
- `GET /api/v1/search` - Basic job search
- `GET /api/v2/search` - Advanced search with filtering
- `GET /api/v3/search` - Database-backed search
- `GET /api/v1/search/top` - Vector similarity search

### Job Applications
- `POST /api/v1/apply` - Submit job application (V1)
- `POST /api/v2/apply` - Submit job application (V2)
- `GET /api/v1/job-applications` - List applications

### Draft Applications
- `POST /api/v1/draft` - Create draft
- `GET /api/v1/draft` - List drafts
- `PUT /api/v1/draft/:id` - Update draft
- `DELETE /api/v1/draft/:id` - Delete draft

### Profiles
- `GET /api/v1/profiles` - Search profiles
- `GET /api/v1/profiles/:id` - Get profile by ID

### Beckn Webhooks
- `POST /webhook/on_search` - Job search response
- `POST /webhook/on_select` - Selection response
- `POST /webhook/on_init` - Initialization response
- `POST /webhook/on_confirm` - Confirmation response
- `POST /webhook/on_status` - Status update

### Admin
- `POST /api/v1/admin/rebuild-faiss` - Rebuild FAISS index

## Cron Jobs

The application uses `tokio-cron-scheduler` for background tasks:

| Task | Default Interval | Description |
|------|------------------|-------------|
| `fetch_jobs` | 3600s (1 hour) | Fetches open jobs from BPP |
| `fetch_profiles` | 86400s (24 hours) | Syncs profiles from Jobstack |
| `compute_match_scores` | 10800s (3 hours) | Calculates job-profile match scores |
| `notification` | Weekly (configurable) | Sends WhatsApp notifications for high matches |

### Configuring Cron Jobs

Edit the `cron` section in your YAML config:

```yaml
cron:
  fetch_jobs:
    seconds: 3600
  fetch_profiles:
    seconds: 86400
  compute_match_scores:
    seconds: 10800
    batch: 50
    source: 'empeding'
  notification:
    schedule_type: 'weekly'
    weekday: 1
    hour: 1
    minute: 1
    min_score: 7
    batch: 30
```

## Database Migrations

Migrations are stored in the `migrations/` directory and use SQLx.

### Available Migrations

| File | Description |
|------|-------------|
| `20250702105235_job_applications.sql` | Job application tracking |
| `20250805051511_draft_applications.sql` | Draft applications |
| `20260113090834_profiles.sql` | Candidate profiles |
| `20260202091820_jobs.sql` | Job records |
| `20260204070509_job_profile_matches.sql` | Match scores with trigram indexes |
| `20260307193403_add_is_active_column_to_jobs.sql` | Active job flag |
| `20260324053703_add_embedding_to_jobs.sql` | Embedding column for vector search |

### Running Migrations

```bash
# Run all pending migrations
./migrate.sh

# Or use SQLx CLI directly
cargo install sqlx-cli
sqlx migrate run
```

## Match Score Configuration

The match scoring system uses `config/match_score.json` to define weights for different factors. The configuration supports:

- Skill matching weights
- Experience matching weights
- Location proximity scoring
- Salary range matching
- Business logic application

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
cargo clippy
```

### Adding Dependencies

Edit `Cargo.toml` and run:

```bash
cargo update
```

## Troubleshooting

### Connection Issues

- Ensure PostgreSQL is running and accessible
- Check Redis connection URL in config
- Verify firewall rules allow connections

### FAISS Issues on macOS

```bash
# If FAISS fails to load, set library paths
export LIBRARY_PATH="/opt/homebrew/lib:$LIBRARY_PATH"
export DYLD_LIBRARY_PATH="/opt/homebrew/lib:$DYLD_LIBRARY_PATH"
```

### Cron Jobs Not Running

- Check application logs for cron initialization messages
- Verify cron intervals are properly configured
- Ensure database is accessible for background tasks

### Memory Issues

- Adjust batch sizes in cron configuration
- Monitor Redis memory usage
- Consider increasing PostgreSQL connection pool size

## Project Structure

```
bap-onest-lite/
├── src/
│   ├── main.rs           # Application entry point
│   ├── config.rs         # Configuration management
│   ├── http/             # HTTP server and routes
│   │   └── routes/       # API endpoint handlers
│   ├── services/         # Business logic layer
│   ├── db/               # Database operations
│   ├── models/           # Data models
│   ├── utils/            # Utility functions
│   ├── cron/             # Scheduled tasks
│   ├── events/           # Event system
│   ├── workers/          # Background workers
│   └── vector/           # FAISS vector search
├── migrations/           # Database migrations
├── config/               # Configuration files
├── docker-compose.yml    # Docker setup
├── Dockerfile            # Container build
├── migrate.sh            # Migration script
└── README.md             # This file
```

## License

[Specify your license here]

## Support

For issues and questions, please [open an issue](issues-url) or contact the maintainers.