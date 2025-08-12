# Migration Safety Guide

## 🛡️ Development Safety Features

### Automatic Protection Against Compilation Errors

Our development environment is configured to **never crash the running API** if there's a compilation error:

1. **cargo-watch behavior**: 
   - Detects file changes and attempts to recompile
   - If compilation fails, the old binary keeps running
   - Shows compilation errors in the logs
   - Only restarts the API when compilation succeeds

2. **How it works**:
   ```
   File change detected → Compile attempt → 
   ✅ Success: Restart with new binary
   ❌ Failure: Keep old binary running, show errors
   ```

### Migration Safety Rules

#### ✅ SAFE Operations

1. **Adding new migrations**:
   ```bash
   # Create new migration
   sqlx migrate add your_feature_name
   # Edit the generated file
   # cargo-watch will auto-compile and apply
   ```

2. **Testing migrations locally**:
   ```bash
   # Validate before applying
   ./scripts/validate-migrations.sh
   
   # Manually run if needed
   cargo sqlx migrate run
   ```

#### ⛔ DANGEROUS Operations (Never Do These!)

1. **Never modify existing migrations** that have been applied
   - Will cause checksum mismatch
   - API will fail to start
   - Requires manual database intervention

2. **Never delete migration files** that have been applied
   - Will cause "missing migration" errors
   - API will fail to start

3. **Never rename migration files**
   - Treated as deleted + new migration
   - Will cause conflicts

### What Happens When You Add a New Migration?

1. **In Development (Docker)**:
   ```
   Add migration file → Volume mount makes it available →
   cargo-watch detects → Recompiles with new migration →
   On restart, migration runs automatically
   ```

2. **In Production**:
   ```
   Add migration file → Commit to git →
   CI/CD builds new Docker image → 
   Deploy new image → Migration runs on startup
   ```

### Recovery Procedures

#### If You Accidentally Modified an Existing Migration:

1. **Immediate fix** (if not pushed to git):
   ```bash
   # Revert the file to its original state
   git checkout -- migrations/YOUR_MIGRATION.sql
   
   # Restart the container
   docker-compose restart qck-api
   ```

2. **If already applied** with wrong checksum:
   ```bash
   # Option 1: Reset development database (data loss!)
   docker-compose down -v
   docker-compose up -d
   
   # Option 2: Fix checksum manually (advanced)
   docker exec qck-postgres psql -U qck_user -d qck_db
   # Update _sqlx_migrations table manually
   ```

### Using the Safety Scripts

#### 1. Safe Development Mode
```bash
# Instead of cargo-watch, use our safe wrapper:
./scripts/safe-watch.sh

# This provides:
# - Health checks before restart
# - Graceful handling of compilation errors
# - Clear status messages
```

#### 2. Migration Validator
```bash
# Before committing any migration changes:
./scripts/validate-migrations.sh

# This checks:
# - No modified existing migrations
# - Proper file naming
# - No duplicate timestamps
# - Successful compilation
# - Optional database backup
```

### Production Deployment Safety

1. **Migrations are embedded at build time**
   - Compilation fails in CI/CD if migrations are invalid
   - Bad builds never reach production

2. **Automatic rollback on failure**
   - If migration fails, container doesn't start
   - Kubernetes/Docker Swarm reverts to previous version
   - No partial migration state

3. **Zero-downtime deployments**
   - Deploy new version alongside old
   - Migrations run on new instance startup
   - Traffic switches only after health check passes

### Best Practices Checklist

- [ ] Never modify existing migrations
- [ ] Always use `sqlx migrate add` for new migrations
- [ ] Run `validate-migrations.sh` before committing
- [ ] Test migrations locally first
- [ ] Keep migrations small and focused
- [ ] Write idempotent migrations when possible
- [ ] Include down migrations for rollback capability
- [ ] Document breaking changes in migration files

### Emergency Contacts

If you encounter migration issues in production:
1. Check container logs: `docker logs qck-api`
2. Verify migration status: `SELECT * FROM _sqlx_migrations;`
3. Rollback if needed: Deploy previous Docker image
4. Contact: DevOps team for database intervention

---

Remember: **The system is designed to fail safely**. If something goes wrong with migrations, the API won't start rather than corrupting data.