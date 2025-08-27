// Diesel Database Pool Configuration
// Replaces SQLx pool with Diesel-async + bb8 connection pooling

use bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use std::time::Duration;

// Embed migrations at compile time
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/diesel");

pub type DieselPool = Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DieselDatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub test_on_checkout: bool,
}

impl Default for DieselDatabaseConfig {
    fn default() -> Self {
        let config = crate::app_config::config();
        Self {
            url: config.database_url.clone(),
            max_connections: config.database_max_connections,
            min_connections: config.database_min_connections,
            connection_timeout: Duration::from_secs(config.database_connect_timeout),
            idle_timeout: Duration::from_secs(config.database_idle_timeout),
            max_lifetime: Duration::from_secs(config.database_max_lifetime),
            test_on_checkout: true,
        }
    }
}

/// Create Diesel connection pool
pub async fn create_diesel_pool(
    config: DieselDatabaseConfig,
) -> Result<DieselPool, Box<dyn std::error::Error>> {
    // Create connection manager
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(config.url.clone());

    // Configure bb8 pool
    let pool = Pool::builder()
        .max_size(config.max_connections)
        .min_idle(Some(config.min_connections))
        .connection_timeout(config.connection_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .test_on_check_out(config.test_on_checkout)
        .build(manager)
        .await?;

    // Test the connection
    let conn = pool.get().await?;
    drop(conn);

    tracing::info!(
        "Diesel pool initialized with {} max connections",
        config.max_connections
    );

    Ok(pool)
}

/// Health check for database pool
pub async fn check_diesel_health(pool: &DieselPool) -> Result<(), Box<dyn std::error::Error>> {
    let conn = pool.get().await?;

    // Simple health check - just getting a connection is enough
    drop(conn);

    Ok(())
}

/// Mask database connection string for logging
pub fn mask_connection_string(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let scheme = parsed.scheme();
        let host = parsed.host_str().unwrap_or("***");
        let path = parsed.path();

        // Always normalize to postgresql:// prefix
        let normalized_scheme = if scheme == "postgres" {
            "postgresql"
        } else {
            scheme
        };

        // Check if URL has credentials
        if parsed.username().is_empty() && parsed.password().is_none() {
            // No credentials in URL
            format!("{}://{}{}", normalized_scheme, host, path)
        } else {
            // Has credentials, mask them
            format!("{}://***:***@{}{}", normalized_scheme, host, path)
        }
    } else {
        "postgresql://***:***@***".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_creation() {
        // Skip test if config is not available (e.g., in CI without database)
        use std::panic;
        let config_result =
            panic::catch_unwind(|| crate::app_config::config().database_url.clone());
        if config_result.is_err() {
            eprintln!("Skipping test: Database configuration not available");
            return;
        }

        let config = DieselDatabaseConfig::default();
        let pool = create_diesel_pool(config).await;

        assert!(pool.is_ok(), "Failed to create pool: {:?}", pool.err());
    }
}
