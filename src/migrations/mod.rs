// Migration orchestrator for QCK Backend
// Handles both Diesel (PostgreSQL) and ClickHouse migrations
// Embedded in the application binary for distroless container compatibility

pub mod clickhouse;
pub mod diesel;

use crate::db::DieselPool;
use std::error::Error;
use tracing::{error, info, warn};

/// Configuration for migration execution
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub skip_diesel: bool,
    pub skip_clickhouse: bool,
    pub environment: String,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        let config = crate::app_config::config();

        Self {
            skip_diesel: false,
            skip_clickhouse: false,
            environment: config.environment.to_string(),
        }
    }
}

/// Main migration orchestrator
/// Runs both Diesel and ClickHouse migrations in the correct order
pub async fn run_all_migrations(
    diesel_pool: &DieselPool,
    config: MigrationConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!(
        "[MIGRATIONS] Starting migration process for environment: {}",
        config.environment
    );

    // Note: Migrations now run in all environments including development
    // This ensures ClickHouse tables are properly created for testing

    let mut migration_count = 0;

    // Run Diesel migrations first (database schema)
    if !config.skip_diesel {
        info!("[MIGRATIONS] Running Diesel (PostgreSQL) migrations...");
        match diesel::run_migrations(diesel_pool).await {
            Ok(applied_count) => {
                migration_count += applied_count;
                if applied_count > 0 {
                    info!("[MIGRATIONS] ✓ Applied {} Diesel migrations", applied_count);
                } else {
                    info!("[MIGRATIONS] ✓ Diesel migrations up to date");
                }
            },
            Err(e) => {
                error!("[MIGRATIONS] ✗ Diesel migration failed: {}", e);
                return Err(format!("Diesel migration failed: {}", e).into());
            },
        }
    } else {
        info!("[MIGRATIONS] Skipping Diesel migrations (disabled in config)");
    }

    // Run ClickHouse migrations second (analytics schema)
    if !config.skip_clickhouse {
        info!("[MIGRATIONS] Running ClickHouse migrations...");
        match clickhouse::run_migrations().await {
            Ok(applied_count) => {
                migration_count += applied_count;
                if applied_count > 0 {
                    info!(
                        "[MIGRATIONS] ✓ Applied {} ClickHouse migrations",
                        applied_count
                    );
                } else {
                    info!("[MIGRATIONS] ✓ ClickHouse migrations up to date");
                }
            },
            Err(e) => {
                error!("[MIGRATIONS] ✗ ClickHouse migration failed: {}", e);
                return Err(format!("ClickHouse migration failed: {}", e).into());
            },
        }
    } else {
        info!("[MIGRATIONS] Skipping ClickHouse migrations (disabled in config)");
    }

    if migration_count > 0 {
        info!(
            "[MIGRATIONS] ✓ Migration process completed successfully - applied {} total migrations",
            migration_count
        );
    } else {
        info!("[MIGRATIONS] ✓ Migration process completed - all migrations up to date");
    }

    Ok(())
}

/// Check if migrations should run based on environment variables
pub fn should_run_migrations() -> bool {
    let config = crate::app_config::config();

    // Check explicit disable flag
    if config.disable_embedded_migrations {
        return false;
    }

    // Always run migrations in all environments
    // This ensures ClickHouse tables are created for development testing
    true
}
