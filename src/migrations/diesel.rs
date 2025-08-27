// Diesel migration runner for PostgreSQL
// Uses embedded migrations from diesel_migrations crate
// Note: diesel_migrations requires sync connections, not async

use crate::db::{diesel_pool::MIGRATIONS, DieselPool};
use diesel::Connection;
use diesel::PgConnection;
use diesel_migrations::MigrationHarness;
use std::error::Error;
use tracing::{debug, info};

/// Run all pending Diesel migrations
/// Returns the number of migrations applied
pub async fn run_migrations(_pool: &DieselPool) -> Result<usize, Box<dyn Error + Send + Sync>> {
    info!("[DIESEL] Starting Diesel migration process...");

    // Get database URL from centralized config (migrations need sync connection)
    let database_url = crate::app_config::config().database_url.clone();

    // Run migrations in a blocking task since MigrationHarness is sync
    let applied_migrations =
        tokio::task::spawn_blocking(move || -> Result<usize, Box<dyn Error + Send + Sync>> {
            debug!("[DIESEL] Establishing sync connection for migrations...");

            // Create sync connection for migrations
            let mut conn = PgConnection::establish(&database_url)
                .map_err(|e| format!("Failed to establish sync connection: {}", e))?;

            debug!("[DIESEL] Checking for pending migrations...");

            // Check which migrations need to be run
            let pending_migrations = conn
                .pending_migrations(MIGRATIONS)
                .map_err(|e| format!("Failed to check pending migrations: {}", e))?;

            let pending_count = pending_migrations.len();

            if pending_count == 0 {
                debug!("[DIESEL] No pending migrations found");
                return Ok(0);
            }

            info!("[DIESEL] Found {} pending migrations", pending_count);

            // Run pending migrations
            let applied = conn
                .run_pending_migrations(MIGRATIONS)
                .map_err(|e| format!("Failed to run migrations: {}", e))?;

            let applied_count = applied.len();
            info!("[DIESEL] Successfully applied {} migrations", applied_count);

            // Log applied migrations for debugging
            for migration in applied {
                debug!("[DIESEL] Applied migration: {}", migration);
            }

            Ok(applied_count)
        })
        .await
        .map_err(|e| format!("Migration task panicked: {}", e))??;

    info!("[DIESEL] Diesel migration process completed successfully");
    Ok(applied_migrations)
}

/// Check migration status without applying
/// Useful for health checks and debugging
pub async fn check_migration_status(
    _pool: &DieselPool,
) -> Result<MigrationStatus, Box<dyn Error + Send + Sync>> {
    let database_url = crate::app_config::config().database_url.clone();

    let status = tokio::task::spawn_blocking(
        move || -> Result<MigrationStatus, Box<dyn Error + Send + Sync>> {
            let mut conn = PgConnection::establish(&database_url)
                .map_err(|e| format!("Failed to establish sync connection: {}", e))?;

            let applied = conn
                .applied_migrations()
                .map_err(|e| format!("Failed to get applied migrations: {}", e))?;

            let pending = conn
                .pending_migrations(MIGRATIONS)
                .map_err(|e| format!("Failed to get pending migrations: {}", e))?;

            Ok(MigrationStatus {
                applied_count: applied.len(),
                pending_count: pending.len(),
                total_migrations: applied.len() + pending.len(), // Can't get total from MIGRATIONS directly
                applied_migrations: applied.iter().map(|m| m.to_string()).collect(),
                pending_migrations: pending.iter().map(|m| m.name().to_string()).collect(),
            })
        },
    )
    .await
    .map_err(|e| format!("Status check task panicked: {}", e))??;

    Ok(status)
}

/// Migration status information
#[derive(Debug)]
pub struct MigrationStatus {
    pub applied_count: usize,
    pub pending_count: usize,
    pub total_migrations: usize,
    pub applied_migrations: Vec<String>,
    pub pending_migrations: Vec<String>,
}

impl MigrationStatus {
    pub fn is_up_to_date(&self) -> bool {
        self.pending_count == 0
    }

    pub fn needs_migration(&self) -> bool {
        self.pending_count > 0
    }
}

/// Rollback the last applied migration (use with caution)
/// Only available in non-production environments for safety
pub async fn rollback_last_migration(
    _pool: &DieselPool,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    // Safety check - only allow rollbacks in development/test
    let config = crate::app_config::config();
    if config.is_production() {
        return Err("Migration rollbacks are disabled in production for safety".into());
    }

    info!(
        "[DIESEL] Rolling back last migration (environment: {:?})",
        config.environment
    );

    let database_url = config.database_url.clone();

    let rolled_back =
        tokio::task::spawn_blocking(move || -> Result<String, Box<dyn Error + Send + Sync>> {
            let mut conn = PgConnection::establish(&database_url)
                .map_err(|e| format!("Failed to establish sync connection: {}", e))?;

            let applied = conn
                .applied_migrations()
                .map_err(|e| format!("Failed to get applied migrations: {}", e))?;

            if applied.is_empty() {
                return Err("No migrations to rollback".into());
            }

            let last_migration = applied.last().unwrap();

            conn.revert_last_migration(MIGRATIONS)
                .map_err(|e| format!("Failed to rollback migration: {}", e))?;

            info!(
                "[DIESEL] Successfully rolled back migration: {}",
                last_migration
            );
            Ok(last_migration.to_string())
        })
        .await
        .map_err(|e| format!("Rollback task panicked: {}", e))??;

    Ok(rolled_back)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_diesel_pool, DieselDatabaseConfig};

    #[tokio::test]
    async fn test_migration_status_check() {
        // Skip test if config is not available (e.g., in CI without database)
        use std::panic;
        let config_result =
            panic::catch_unwind(|| crate::app_config::config().database_url.clone());
        if config_result.is_err() {
            eprintln!("Skipping test: Database configuration not available");
            return;
        }

        let config = DieselDatabaseConfig::default();
        let pool = match create_diesel_pool(config).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Skipping test: Failed to create pool: {}", e);
                return;
            },
        };

        let status = check_migration_status(&pool)
            .await
            .expect("Failed to check status");

        // Should have at least one migration (initial schema)
        assert!(status.total_migrations > 0);
        println!("Migration status: {:?}", status);
    }
}
