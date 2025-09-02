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
/// Skips seed migrations in production environment for safety
pub async fn run_migrations(_pool: &DieselPool) -> Result<usize, Box<dyn Error + Send + Sync>> {
    let config = crate::app_config::config();
    info!(
        "[DIESEL] Starting Diesel migration process (environment: {:?})...",
        config.environment
    );

    // Get database URL from centralized config (migrations need sync connection)
    let database_url = config.database_url.clone();
    let is_production = config.is_production();

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

            // Filter out seed migrations in production
            let (filtered_migrations, _original_count) = if is_production {
                let original_count = pending_migrations.len();
                let filtered: Vec<_> = pending_migrations
                    .into_iter()
                    .filter(|migration| {
                        let migration_name = migration.name().to_string();
                        let is_seed = migration_name.contains("seed")
                            || migration_name.contains("demo")
                            || migration_name.contains("_seed_");

                        if is_seed {
                            info!(
                                "[DIESEL] SKIPPING seed migration in production: {}",
                                migration_name
                            );
                        }

                        !is_seed
                    })
                    .collect();

                info!(
                    "[DIESEL] Production environment: filtered {} seed migrations, {} remaining",
                    original_count - filtered.len(),
                    filtered.len()
                );
                (filtered, original_count)
            } else {
                let count = pending_migrations.len();
                (pending_migrations, count)
            };

            let pending_count = filtered_migrations.len();

            if pending_count == 0 {
                debug!("[DIESEL] No pending migrations to run (after filtering)");
                return Ok(0);
            }

            info!(
                "[DIESEL] Found {} pending migrations to apply",
                pending_count
            );

            // Apply each filtered migration individually
            let mut applied_count = 0;
            for migration in filtered_migrations {
                info!("[DIESEL] Applying migration: {}", migration.name());
                conn.run_migration(&migration)
                    .map_err(|e| format!("Failed to run migration {}: {}", migration.name(), e))?;
                applied_count += 1;
            }

            info!("[DIESEL] Successfully applied {} migrations", applied_count);
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
