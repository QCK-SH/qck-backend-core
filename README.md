# QCK Backend API

Rust-based monolithic backend API for QCK URL shortener platform.

## Tech Stack

- **Language**: Rust
- **Framework**: Axum
- **Databases**: PostgreSQL (primary), Redis (cache), ClickHouse (analytics)
- **ORM**: Diesel with async support
- **Authentication**: JWT (HS256)

## Development Setup

### Prerequisites

#### Required for Docker Development (Recommended)
- Docker & Docker Compose
- Rust 1.82+ (for IDE support)

#### Required for Local Development
- Rust 1.82+
- PostgreSQL client libraries (`libpq`)
- Diesel CLI (for migrations)

##### macOS Setup
```bash
# Install PostgreSQL client libraries
brew install libpq

# Set environment variables (add to ~/.zshrc or ~/.bashrc)
export PKG_CONFIG_PATH="/opt/homebrew/opt/libpq/lib/pkgconfig"
export RUSTFLAGS="-L /opt/homebrew/opt/libpq/lib"

# Install Diesel CLI
cargo install diesel_cli --no-default-features --features postgres
```

##### Linux Setup
```bash
# Ubuntu/Debian
sudo apt-get install libpq-dev

# RHEL/CentOS/Fedora
sudo yum install postgresql-devel

# Install Diesel CLI
cargo install diesel_cli --no-default-features --features postgres
```

### Quick Start

#### Docker Development (Recommended)
```bash
# Start development environment
docker compose --env-file .env.dev -f docker-compose.dev.yml up -d

# View logs
docker logs -f qck-api-dev

# Build inside container (no local dependencies needed)
docker exec qck-api-dev cargo build

# Run tests inside container
docker exec qck-api-dev cargo test
```

#### Local Development
```bash
# Ensure libpq is installed and environment variables are set (see Prerequisites)

# Build locally
cargo build

# Run tests locally (requires test database)
cargo test

# Run locally (requires all services running)
cargo run
```

**Note**: Docker development is recommended as it handles all dependencies automatically.

## Database Migrations

### ⚠️ CRITICAL: Migration Workflow

**IMPORTANT**: Always follow this exact workflow when creating new migrations. The `schema.rs` file is auto-generated but MUST be committed to version control.

### Step 1: Create Migration

```bash
# Install diesel CLI if not already installed
cargo install diesel_cli --no-default-features --features postgres

# Create new migration
diesel migration generate your_migration_name

# Edit the generated files:
# - migrations/diesel/YYYY-MM-DD-HHMMSS_your_migration_name/up.sql
# - migrations/diesel/YYYY-MM-DD-HHMMSS_your_migration_name/down.sql
```

### Step 2: Apply Migration & Update Schema

```bash
# Restart container (migrations auto-run on startup via diesel-migrate.sh)
docker compose --env-file .env.dev -f docker-compose.dev.yml restart qck-api-dev

# Wait 5-10 seconds for migrations to complete

# Copy the auto-generated schema from container
docker cp qck-api-dev:/app/src/schema.rs src/schema.rs

# Format the schema file
cargo fmt
```

### Step 3: Verify & Commit

```bash
# Verify the build works locally
cargo build --lib

# If build succeeds, commit BOTH migration and schema
git add migrations/ src/schema.rs
git commit -m "feat: add migration for [feature_name]

- Migration: [describe database changes]
- Updated schema.rs with latest structure"
```

### Why This Workflow?

1. **Migrations run automatically** on container startup via `diesel-migrate.sh`
2. **Schema.rs is auto-generated** by Diesel after migrations
3. **Schema.rs must be committed** so the project builds without a database
4. **Never manually edit schema.rs** - it's always generated

### Troubleshooting

If schema.rs gets deleted or corrupted:

```bash
# Option 1: Restore from git
git checkout HEAD -- src/schema.rs

# Option 2: Regenerate from database
docker exec qck-api-dev sh -c "RUST_LOG=error diesel print-schema 2>/dev/null" > src/schema.rs
cargo fmt
```

## API Endpoints

- `GET /v1/health` - Health check
- `GET /v1/health/db` - Database health check
- `POST /v1/auth/login` - User login
- `POST /v1/auth/refresh` - Refresh access token
- `POST /v1/auth/logout` - Logout user
- `GET /v1/auth/me` - Get current user info

## Testing

### Run Tests in Docker (Recommended)

```bash
# Run all tests
docker exec qck-api-dev cargo test

# Run specific test file
docker exec qck-api-dev cargo test --test jwt_service_test

# Run with output
docker exec qck-api-dev cargo test -- --nocapture
```

### Local Testing

Requires PostgreSQL client libraries (`libpq`):

```bash
# macOS
brew install libpq
export PKG_CONFIG_PATH="/opt/homebrew/opt/libpq/lib/pkgconfig"

# Linux
apt-get install libpq-dev  # Debian/Ubuntu
yum install postgresql-devel  # RHEL/CentOS

# Run tests
cargo test
```

## Project Structure

```
src/
├── main.rs              # Application entry point
├── lib.rs               # Library exports for testing
├── handlers/            # HTTP request handlers
├── services/            # Business logic layer
├── models/              # Database models (Diesel)
├── middleware/          # Custom middleware
├── migrations/          # Embedded migrations
├── schema.rs            # Auto-generated by Diesel (DO NOT EDIT)
├── app.rs               # Application state
├── app_config.rs        # Configuration management
└── db/                  # Database connection pools
```

## Configuration

Environment variables are loaded from `.env.dev` for development:

```bash
# Database
DATABASE_URL=postgresql://qck_user:qck_password@postgres-dev:5432/qck_db

# Redis
REDIS_URL=redis://redis-dev:6379

# JWT
JWT_ACCESS_SECRET=dev-access-secret-change-in-production-hs256
JWT_REFRESH_SECRET=dev-refresh-secret-change-in-production-hs256

# See .env.dev for complete list
```

## Docker Services

- `qck-api-dev` - Backend API with hot reload
- `postgres-dev` - PostgreSQL database
- `redis-dev` - Redis cache
- `clickhouse-dev` - ClickHouse analytics
- `adminer-dev` - Database UI (port 10114)

## Contributing

1. Create feature branch from `main`
2. Follow the migration workflow exactly
3. Run tests before committing
4. Ensure `cargo fmt` and `cargo clippy` pass
5. Create PR with Linear issue reference

## License

Proprietary - QCK Platform
