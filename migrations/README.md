# Database Migrations

This directory contains SQL migrations for the QCK Backend database schema.

## Setup

1. Install sqlx-cli (if not already installed):
```bash
cargo install sqlx-cli --no-default-features --features postgres
```

2. Ensure your DATABASE_URL is set in `.env`:
```bash
DATABASE_URL=postgresql://qck_user:qck_password@localhost:12001/qck_db
```

## Usage

### Using the migration script (recommended):
```bash
# Create a new migration
./scripts/migrate.sh create add_user_preferences

# Run all pending migrations
./scripts/migrate.sh run

# Revert the last migration
./scripts/migrate.sh revert

# Show migration status
./scripts/migrate.sh info
```

### Using sqlx directly:
```bash
# Create a new migration
sqlx migrate add -r migration_name

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert

# Show migration info
sqlx migrate info
```

## Migration Files

Migrations are timestamped SQL files in this directory:
- `{timestamp}_{name}.sql` - Forward migration (up)
- `{timestamp}_{name}.down.sql` - Revert migration (down) [if using reversible migrations]

## Automatic Migrations

By default, migrations run automatically on application startup. This can be controlled via the `RUN_MIGRATIONS` environment variable:

```bash
# Enable auto-migration (default)
RUN_MIGRATIONS=true

# Disable auto-migration (for production deployments)
RUN_MIGRATIONS=false
```

## Current Schema

The initial migration creates:
- **users** - User accounts and authentication
- **links** - Shortened URLs with metadata
- **link_clicks** - Analytics and click tracking
- **user_sessions** - Session management
- **api_keys** - API access tokens
- **domains** - Custom domain configuration

## Best Practices

1. **Always test migrations locally first**
2. **Keep migrations small and focused**
3. **Include both up and down migrations when possible**
4. **Never modify existing migration files**
5. **Use transactions where appropriate**
6. **Add indexes for foreign keys and frequently queried columns**
7. **Document complex migrations with comments**

## Production Deployment

For production:
1. Set `RUN_MIGRATIONS=false` in production
2. Run migrations manually during deployment
3. Always backup database before migrations
4. Test rollback procedures
5. Monitor migration performance on large tables