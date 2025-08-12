use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Database configuration for PostgreSQL connection pool
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    pub database_url: String,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Minimum number of connections to maintain
    pub min_connections: u32,

    /// Timeout for acquiring a connection from the pool
    pub connect_timeout: Duration,

    /// How long a connection can be idle before being closed
    pub idle_timeout: Duration,

    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,

    /// Whether to test connections before using them
    pub test_before_acquire: bool,

    /// Enable statement-level caching
    pub statement_cache_capacity: usize,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL environment variable must be set"),
            max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
            min_connections: std::env::var("DATABASE_MIN_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            connect_timeout: Duration::from_secs(
                std::env::var("DATABASE_CONNECT_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
            ),
            idle_timeout: Duration::from_secs(
                std::env::var("DATABASE_IDLE_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1800), // 30 minutes
            ),
            max_lifetime: Duration::from_secs(
                std::env::var("DATABASE_MAX_LIFETIME")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600), // 1 hour
            ),
            test_before_acquire: std::env::var("DATABASE_TEST_BEFORE_ACQUIRE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            statement_cache_capacity: std::env::var("DATABASE_STATEMENT_CACHE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
        }
    }
}

impl DatabaseConfig {
    /// Create a new DatabaseConfig from environment variables
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_connections < self.min_connections {
            return Err("max_connections must be >= min_connections".to_string());
        }

        if self.max_connections == 0 {
            return Err("max_connections must be > 0".to_string());
        }

        if self.database_url.is_empty() {
            return Err("database_url cannot be empty".to_string());
        }

        Ok(())
    }
}
