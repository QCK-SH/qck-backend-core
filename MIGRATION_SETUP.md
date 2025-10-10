# QCK Backend Migration Setup

## Current Migration System (Embedded Diesel Migrations)

After migrating from Actix-web/SQLx to Axum/Diesel, we now use **embedded Diesel migrations** that run automatically when the application starts.

### How It Works

**Migrations are embedded into the application binary at compile time** and run automatically during startup. No external scripts are needed.

### Migration Flow

1. **Compile Time**: Diesel embeds all migration SQL files into the binary
2. **Startup**: Application starts and initializes database pool
3. **Auto-Migration**: Application automatically runs pending migrations
4. **Service Start**: Application continues normal operation

### Scripts Overview

| Script | Purpose | Status |
|--------|---------|--------|
| `scripts/clickhouse-entrypoint.sh` | ClickHouse migrations | ✅ ACTIVE |
| `run-tests.sh` | Test runner | ✅ ACTIVE |
| ~~`scripts/diesel-migrate.sh`~~ | Old external migration runner | ❌ REMOVED |
| ~~`scripts/diesel-migrate-prod.sh`~~ | Old production migration runner | ❌ REMOVED |
| ~~`scripts/migrate.sh`~~ | Old SQLx migrations | ❌ ARCHIVED |

### Directory Structure

```
qck-backend/
├── migrations/                 # All migrations
│   ├── diesel/                # Diesel migration files (EMBEDDED)
│   │   ├── 2025-01-08-000001_initial_schema/
│   │   │   ├── up.sql
│   │   │   └── down.sql
│   │   └── 2025-08-22-133222_create_refresh_tokens/
│   │       ├── up.sql
│   │       └── down.sql
│   └── clickhouse/            # ClickHouse migrations (separate system)
├── src/
│   ├── migrations/            # Migration orchestration code
│   │   ├── mod.rs            # Main migration coordinator
│   │   ├── diesel.rs         # Diesel migration runner
│   │   └── clickhouse.rs     # ClickHouse migration runner
│   └── db/
│       └── diesel_pool.rs    # Embeds migrations: embed_migrations!()
└── scripts/
    ├── clickhouse-entrypoint.sh  # ClickHouse setup (KEEP)
    └── test_*.sql                # Performance tests (KEEP)
```

## How Migrations Work

### Embedded Migration System

**File: `src/db/diesel_pool.rs`**
```rust
use diesel_migrations::{embed_migrations, EmbeddedMigrations};

// Embed migrations at compile time
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/diesel");
```

**File: `src/main.rs` (startup flow)**
```rust
// Run embedded migrations automatically
if crate::migrations::should_run_migrations() {
    info!("Running embedded migrations...");
    match crate::migrations::run_all_migrations(&diesel_pool, migration_config).await {
        Ok(()) => info!("✓ All migrations completed successfully"),
        Err(e) => return Err(format!("Migration failed: {}", e).into()),
    }
}
```

### Development (docker-compose.dev.yml)

1. **Database Startup**: PostgreSQL, Redis, ClickHouse start with health checks
2. **Wait for Health**: qck-api-dev waits for all databases via `depends_on: service_healthy`
3. **Application Starts**: Binary runs directly with `CMD ["qck-backend"]`
4. **Auto-Migration**: Application runs embedded migrations on startup
5. **Service Ready**: Application serves requests

### Production Deployment

**Migrations run automatically when the application starts** - no manual migration step needed.

To disable embedded migrations (if using external migration tools):
```bash
export DISABLE_EMBEDDED_MIGRATIONS=true
```

## Docker Setup

### Key Files

- **Dockerfile.dev**: Builds application with embedded migrations
- **docker-compose.dev.yml**: Development environment with health checks
- **No migration scripts needed**: Migrations are embedded in the binary

### Environment Variables

```bash
# Database connection
DATABASE_URL=postgresql://user:pass@host:5432/db

# Optional: Disable embedded migrations (defaults to false)
DISABLE_EMBEDDED_MIGRATIONS=false

# For macOS development (libpq linking)
LIBRARY_PATH=/opt/homebrew/opt/postgresql@15/lib
PKG_CONFIG_PATH=/opt/homebrew/opt/postgresql@15/lib/pkgconfig
```

## Usage

### Development

```bash
# Start environment (migrations run automatically on app startup)
docker-compose --env-file .env.dev -f docker-compose.dev.yml up

# View logs
docker-compose --env-file .env.dev -f docker-compose.dev.yml logs -f qck-backend-oss-dev

# Stop and clean
docker-compose --env-file .env.dev -f docker-compose.dev.yml down -v
```

### Creating New Migrations

```bash
# Create new migration
diesel migration generate <name>

# Edit the generated files
vim migrations/diesel/*/up.sql
vim migrations/diesel/*/down.sql

# Test locally (migrations will run on next app start)
docker-compose --env-file .env.dev -f docker-compose.dev.yml restart qck-backend-oss-dev

# Or run manually with diesel CLI
diesel migration run
```

### Testing

```bash
# Run tests with test database
./run-tests.sh

# Or manually
export DATABASE_URL=postgresql://qck_user:qck_password@localhost:15001/qck_test
cargo test
```

## Migration Systems

### PostgreSQL (Diesel - Embedded)
- **Embedded**: Migrations compiled into binary
- **Auto-Run**: Executes on application startup
- **Tracking**: Uses `__diesel_schema_migrations` table
- **Safety**: Filters seed migrations in production

### ClickHouse (Custom - Embedded)
- **Embedded**: SQL files included at compile time
- **Auto-Run**: Executes after Diesel migrations
- **Tracking**: Uses custom `schema_migrations` table
- **Files**: Located in `migrations/clickhouse/`

## Important Notes

1. **No Scripts Needed**: Migrations are embedded in the binary, no external scripts required
2. **Automatic Execution**: Migrations run automatically when application starts
3. **Idempotent**: Diesel tracks applied migrations, safe to run multiple times
4. **Docker Health Checks**: PostgreSQL must be healthy before app starts (via `depends_on`)
5. **Production Safety**: Seed migrations are automatically filtered in production
6. **Compile Time**: Migration files must exist when building the binary

## Benefits of Embedded Migrations

✅ **Simpler Deployment**: No separate migration step needed
✅ **Always In Sync**: Migrations always match the code version
✅ **Atomic Updates**: Code and schema changes deploy together
✅ **Docker Friendly**: Works perfectly in distroless/minimal containers
✅ **No External Dependencies**: No need for Diesel CLI in production

## Troubleshooting

### libpq not found (macOS)
```bash
export LIBRARY_PATH="/opt/homebrew/opt/postgresql@15/lib:$LIBRARY_PATH"
export PKG_CONFIG_PATH="/opt/homebrew/opt/postgresql@15/lib/pkgconfig:$PKG_CONFIG_PATH"
```

### Check migration status
The application logs show migration status on startup:
```
[MIGRATIONS] Starting migration process for environment: development
[DIESEL] Found 0 pending migrations to apply
[MIGRATIONS] ✓ Migration process completed - all migrations up to date
```

### Database not ready
Check PostgreSQL health:
```bash
docker-compose --env-file .env.dev -f docker-compose.dev.yml logs qck-postgres-oss-dev
```

### Force disable embedded migrations
```bash
export DISABLE_EMBEDDED_MIGRATIONS=true
# Then run migrations manually with diesel CLI
diesel migration run
```
