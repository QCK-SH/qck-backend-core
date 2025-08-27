# QCK Backend Migration Setup

## Current Migration System (Diesel)

After migrating from Actix-web/SQLx to Axum/Diesel, we now use Diesel for database migrations.

### Scripts Overview

| Script | Purpose | Usage | Status |
|--------|---------|-------|--------|
| `scripts/diesel-migrate.sh` | Main migration runner for Docker | Used by `Dockerfile.dev` | ✅ ACTIVE |
| `scripts/clickhouse-entrypoint.sh` | ClickHouse migrations | Used in production | ✅ ACTIVE |
| `run-tests.sh` | Test runner | Manual testing | ✅ ACTIVE |
| ~~`scripts/migrate.sh`~~ | Old SQLx migrations | Archived | ❌ ARCHIVED |

### Directory Structure

```
qck-backend/
├── migrations/                 # All migrations
│   ├── diesel/                # Diesel migration files (CURRENT)
│   │   ├── 2025-01-08-000001_initial_schema/
│   │   │   ├── up.sql
│   │   │   └── down.sql
│   │   └── 2025-08-22-133222_create_refresh_tokens/
│   │       ├── up.sql
│   │       └── down.sql
│   └── clickhouse/            # ClickHouse migrations (KEEP)
├── scripts/
│   ├── diesel-migrate.sh      # Diesel migration runner (ACTIVE)
│   ├── clickhouse-entrypoint.sh  # ClickHouse setup (KEEP)
│   └── test_*.sql              # Performance tests (KEEP)
└── archived/old-sqlx-migrations/  # Old SQLx files (ARCHIVED)
```

## How Migrations Work

### Development (docker-compose.dev.yml)

1. **Database Startup**: PostgreSQL, Redis, ClickHouse start with health checks
2. **Wait for Health**: qck-api-dev waits for all databases to be healthy
3. **Migration Runner**: `scripts/diesel-migrate.sh` executes:
   - Waits for PostgreSQL connection (30 attempts)
   - Runs `diesel migration run` if `RUN_MIGRATIONS=true`
   - Starts service with `cargo-watch` for hot reload

### Production (docker-compose.yml)

- Set `RUN_MIGRATIONS=false` (migrations run separately)
- Run migrations manually before deployment:
  ```bash
  diesel migration run --database-url $DATABASE_URL
  ```

## Docker Setup

### Key Files

- **Dockerfile.dev**: Development container with Diesel CLI
- **docker-compose.dev.yml**: Development environment with auto-migrations
- **scripts/diesel-migrate.sh**: Migration runner script

### Environment Variables

```bash
# Required for migrations
DATABASE_URL=postgresql://user:pass@host:5432/db
RUN_MIGRATIONS=true  # Enable auto-migrations in dev

# For macOS development (libpq linking)
LIBRARY_PATH=/opt/homebrew/opt/postgresql@15/lib
PKG_CONFIG_PATH=/opt/homebrew/opt/postgresql@15/lib/pkgconfig
```

## Usage

### Development

```bash
# Start with auto-migrations
docker-compose -f docker-compose.dev.yml up

# View logs
docker-compose -f docker-compose.dev.yml logs -f qck-api-dev

# Stop and clean
docker-compose -f docker-compose.dev.yml down -v
```

### Creating New Migrations

```bash
# Create new migration
diesel migration generate <name>

# Edit the generated files
vim migrations/diesel/*/up.sql
vim migrations/diesel/*/down.sql

# Run migration
diesel migration run

# Revert if needed
diesel migration revert
```

### Testing

```bash
# Run tests with test database
./run-tests.sh

# Or manually
export DATABASE_URL=postgresql://qck_user:qck_password@localhost:15001/qck_test
cargo test
```

## Migration from SQLx to Diesel

- **Old System**: SQLx with `migrations/*.sql` files
- **New System**: Diesel with `migrations/diesel/` structure
- **Archived**: Old SQLx files moved to `archived/old-sqlx-migrations/`

## Important Notes

1. **Diesel Embedded Migrations**: Migrations are compiled into the binary
2. **Idempotent**: Diesel tracks applied migrations in `__diesel_schema_migrations` table
3. **Hot Reload**: Changes trigger automatic rebuild via cargo-watch
4. **Health Checks**: All services must be healthy before migrations run
5. **ClickHouse**: Uses separate migration system via `clickhouse-entrypoint.sh`

## Troubleshooting

### libpq not found (macOS)
```bash
export LIBRARY_PATH="/opt/homebrew/opt/postgresql@15/lib:$LIBRARY_PATH"
export PKG_CONFIG_PATH="/opt/homebrew/opt/postgresql@15/lib/pkgconfig:$PKG_CONFIG_PATH"
```

### Migrations already applied
Diesel tracks migrations in the database. Check status:
```bash
diesel migration list
```

### Database not ready
Increase wait time in `diesel-migrate.sh` or check database logs:
```bash
docker-compose -f docker-compose.dev.yml logs postgres-dev
```