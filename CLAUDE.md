# QCK Backend - Project Memory

> **Rust-based monolithic backend API for QCK URL shortener**

## 🚨 CRITICAL RULES

**Task Delegation**: ALWAYS use Task tool with specialized subagents (`general-purpose`, `bug-hunter`, `debugger`, `api-documenter`, `code-reviewer`, `test-runner`, `codex`)

**Safety Check**: Run `git status` before file modifications. Stop if uncommitted changes exist.

**Git Commits**: Never mention "Claude" in commits/PRs. No AI references, no "Co-Authored-By", keep professional

**GitHub PR Review**: Use single command for efficiency:
```bash
gh api repos/OWNER/REPO/pulls/PR_NUMBER/comments --jq '.[] | {user: .user.login, created: .created_at, body: .body[0:200], path, line}'
```

**Package Manager**: `cargo` only (never npm/yarn/pnpm)

## Project Setup

- **Tech Stack**: Rust, Axum, PostgreSQL, Redis, ClickHouse
- **Port**: :8080 | **Database UI**: :8081 (Adminer)
- **Hot Reload**: Use `cargo-watch` for development
- **Required**: `cargo fmt`, `cargo clippy`, `cargo test` before commits

### Quick Start
```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f qck-api
```

## Architecture

### Structure
```
src/
├── main.rs              # Application entry point
├── handlers/            # HTTP request handlers
├── services/            # Business logic layer
├── models/              # Database models
├── middleware/          # Custom middleware
├── utils/               # Utility functions
└── config/              # Configuration management
```

### Database Stack
- **PostgreSQL**: Main data store (users, links, settings)
- **Redis**: Cache layer and session storage
- **ClickHouse**: Analytics and event tracking

## Development Standards

### Rust Conventions
- Use `snake_case` for functions/variables
- Use `PascalCase` for types/structs
- Prefer `Result<T, E>` for error handling
- Use `thiserror` for custom errors
- Keep handlers thin, logic in services
- Use `#[instrument]` for tracing

### Error Handling Pattern
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Not found")]
    NotFound,
}
```

### API Response Format
```rust
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<ErrorDetail>,
    meta: ResponseMeta,
}
```

## Testing Strategy

### Test Organization (REQUIRED: Separate Files)
```
tests/                   # Integration tests
├── postgres_test.rs    # Database pool tests
├── redis_config_test.rs # Redis configuration tests
├── redis_pool_test.rs  # Redis pool tests
└── api_endpoints_test.rs # API integration tests
```

**NO inline #[cfg(test)] modules allowed**

### Test Execution
```bash
# Run all tests (auto-loads .env.test)
cargo test

# Run specific categories  
cargo test --test redis_pool_test
cargo test -- --nocapture
```

## Performance Requirements

### Production Scaling (1M Clicks/Day)
- **Database**: 300 connections (12 avg/s, 100 peak/s)
- **Redis**: 150 connections (95%+ cache hit ratio)
- **Performance**: <50ms redirects, <1ms cache hits
- **Resources**: API(1GB), PostgreSQL(2GB), Redis(512MB)

### General Performance
- Database queries < 50ms
- API responses < 200ms
- Use connection pooling
- Implement query caching

## Common Tasks

### Adding New Endpoint
1. Define route in src/main.rs
2. Create handler in src/handlers/
3. Implement business logic in src/services/
4. Add models in src/models/ if needed
5. Write tests in tests/
6. Update OpenAPI spec

### Database Migrations

**⚠️ CRITICAL**: Follow this EXACT workflow for migrations. The `schema.rs` file is auto-generated but MUST be committed to version control.

#### Step-by-Step Migration Workflow

**Step 1: Create Migration**
```bash
# Install diesel CLI if needed
cargo install diesel_cli --no-default-features --features postgres

# Create new migration
diesel migration generate your_migration_name

# Edit the generated files:
# - migrations/diesel/YYYY-MM-DD-HHMMSS_your_migration_name/up.sql
# - migrations/diesel/YYYY-MM-DD-HHMMSS_your_migration_name/down.sql
```

**Step 2: Apply Migration & Update Schema**
```bash
# IMPORTANT: Load database URL from .env.dev
source ../env.dev

# Run migrations locally
diesel migration run

# Generate/update schema.rs locally
diesel print-schema > src/schema.rs

# OR if using Docker (migrations auto-run on startup via diesel-migrate.sh)
docker compose --env-file ../.env.dev -f ../docker-compose.dev.yml restart qck-api-dev

# Wait 5-10 seconds for migrations to complete

# CRITICAL: Copy the auto-generated schema from container (required every time)
docker cp qck-api-dev:/app/src/schema.rs src/schema.rs

# Format the schema file
cargo fmt
```

**Step 3: Verify & Commit**
```bash
# Verify the build works locally
cargo build --lib

# If build succeeds, commit BOTH migration and schema
git add migrations/ src/schema.rs
git commit -m "feat: add migration for [feature_name]

- Migration: [describe database changes]
- Updated schema.rs with latest structure"
```

#### Why This Workflow?
1. **Migrations run automatically** on container startup via `diesel-migrate.sh`
2. **Schema.rs is auto-generated** by Diesel after migrations
3. **Schema.rs must be committed** so the project builds without a database
4. **Never manually edit schema.rs** - it's always generated

#### If schema.rs Gets Deleted
```bash
# Option 1: Restore from git
git checkout HEAD -- src/schema.rs

# Option 2: Regenerate from database
docker exec qck-api-dev sh -c "RUST_LOG=error diesel print-schema 2>/dev/null" > src/schema.rs
cargo fmt
```

## Security Checklist
- [ ] Input validation on all endpoints
- [ ] SQL injection prevention (parameterized queries)
- [ ] Rate limiting implemented
- [ ] JWT validation on protected routes
- [ ] Secrets in environment variables
- [ ] CORS properly configured

## Environment Variables

**IMPORTANT**: Always use `../.env.dev` for development configuration. This file contains all database URLs, ports, and credentials for local development.

Required in `../.env.dev`:
```bash
# Check .env.dev for actual values - DO NOT hardcode URLs
# The .env.dev file is the single source of truth for:
# - DATABASE_URL (PostgreSQL connection)
# - REDIS_URL (Redis connection)  
# - CLICKHOUSE_URL (ClickHouse connection)
# - All JWT secrets and configuration
# - Port mappings and service endpoints
```

**Never use hardcoded database URLs. Always reference .env.dev for the current configuration.**

## 📊 Linear Configuration

### Backend Label Priority
- **Primary Platform**: `backend` - ID: `3d9d6f26-756e-48f6-918b-b54c47dccac1`
- **Common Types**: `api` (ff9a7ae0), `database` (9abb6b86), `auth` (50086f97)

### Status Flow
Todo → In Progress → In Review → QA → **waiting for review** → Done

### Issue Management
- Use `backend` label for all backend work
- Link commits to Linear issues using issue ID
- Mark "waiting for review" when complete
- Document API changes in issue comments
- Never mark "Done" directly

## Testing

### Running Tests in Docker (Recommended)

**IMPORTANT**: Always run tests inside the Docker container where `libpq` is already installed:

```bash
# Start development environment
docker-compose -f docker-compose.dev.yml up -d

# Run all tests inside the container
docker exec qck-api-dev cargo test

# Run specific test file
docker exec qck-api-dev cargo test --test postgres_test
docker exec qck-api-dev cargo test --test redis_pool_test
docker exec qck-api-dev cargo test --test jwt_service_test

# Watch logs
docker logs -f qck-api-dev
```

### Local Testing Setup (If Not Using Docker)

**Note**: Running tests locally requires PostgreSQL client libraries (`libpq`):

```bash
# macOS - Install PostgreSQL client
brew install libpq
export PKG_CONFIG_PATH="/opt/homebrew/opt/libpq/lib/pkgconfig"

# Linux - Install PostgreSQL client  
apt-get install libpq-dev  # Debian/Ubuntu
yum install postgresql-devel  # RHEL/CentOS

# Load environment variables from .env.dev
source ../.env.dev
# OR use dotenv-cli
dotenv -f ../.env.dev cargo test
```

**IMPORTANT**: Always source `../.env.dev` for database URLs and configuration. Do not hardcode connection strings.

### Test Environment with docker-compose.test.yml

```bash
# Start test environment
docker-compose -f docker-compose.test.yml up -d

# Wait for services to be healthy
sleep 10

# Export test environment variables
export DATABASE_URL="postgresql://qck_user:qck_password@localhost:15001/qck_test"
export REDIS_URL="redis://localhost:15002"
export CLICKHOUSE_URL="http://localhost:15003"
export JWT_ACCESS_SECRET="test-access-secret-hs256"
export JWT_REFRESH_SECRET="test-refresh-secret-hs256"
export JWT_ACCESS_EXPIRY="3600"
export JWT_REFRESH_EXPIRY="604800"
export JWT_KEY_VERSION="1"
export REDIS_CONNECTION_TIMEOUT="5"
export REDIS_COMMAND_TIMEOUT="5"

# Run all tests
cargo test

# Run specific test file
cargo test --test postgres_test
cargo test --test redis_pool_test
cargo test --test jwt_service_test

# Stop test environment
docker-compose -f docker-compose.test.yml down
```

### Quick Test Script
Use `./run-tests.sh` for automated test execution with proper environment setup.

### Test Environment Ports
- PostgreSQL: localhost:15001
- Redis: localhost:15002
- ClickHouse: localhost:15003 (HTTP), localhost:15004 (Native)
- API: localhost:15000

## Common Issues & Solutions

### Database Connection Issues
- Check docker-compose is running
- Verify DATABASE_URL is correct
- Check PostgreSQL logs: `docker-compose logs postgres`

### Compilation Errors
- Run `cargo clean` and rebuild
- Update dependencies: `cargo update`
- Check for breaking changes in Cargo.toml

### Performance Problems
- Enable debug logging
- Check slow query logs
- Monitor Redis hit rate

## Git Workflow

### Branches
`feature/`, `fix/`, `refactor/`, `docs/`, `test/`, `perf/`

### Commits
`feat:`, `fix:`, `docs:`, `style:`, `refactor:`, `test:`, `perf:`, `build:`, `ci:`

### PR Requirements
- Include Linear issue ID in PR title
- Pass all CI checks
- Get at least one review
- Squash and merge to main

## 🧠 Recent Implementation

### Migration from SQLx to Diesel (Completed Aug 2025)
- Migrated all database operations from SQLx to Diesel ORM
- Replaced PostgresPool wrapper with DieselPool using bb8
- Updated all models to use Diesel's type system
- Converted all tests to use Diesel async operations
- **All 85 tests passing** (0 failures)

### Functions
- `create_diesel_pool()` - Diesel connection pool with bb8
- `RedisPool::new()` - Redis connection pool (DEV-91)
- `comprehensive_health_check()` - Multi-service health check
- `check_clickhouse_health()` - ClickHouse connectivity test
- Embedded Diesel migrations via `diesel::embed_migrations!()`

### Key Decisions
- HS256 JWT algorithm (HMAC SHA-256) per Linear DEV-113 (NOT ES256)
- Diesel ORM with async support for PostgreSQL
- Redis for session storage and token blacklisting
- Separate test files (no inline #[cfg(test)] modules)
- Centralized configuration in `app_config.rs` (JavaScript config.js pattern)
- JWT validation with `leeway = 0` for strict expiry
- <50ms redirect performance requirement

### Recently Completed (Aug 2025)
- Fixed all JWT test error expectations (EncodingError not InvalidToken)
- Consolidated dual config files into single `app_config.rs`
- All routes using `/v1/` prefix
- Database URL masking for security
- Tests run inside Docker (libpq pre-installed)

### Architecture Notes
- Production scaling: 300 DB connections, 150 Redis connections
- Performance target: 1800-2000 Redis ops/second
- Test organization: Separate files in tests/ directory
- Docker-based development (docker-compose.dev.yml)

---
*Complete self-contained documentation for qck-backend. Always update with changes.*
