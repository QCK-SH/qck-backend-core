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
        let config = &crate::CONFIG.database;

        Self {
            database_url: config.url.clone(),
            max_connections: config.max_connections,
            min_connections: config.min_connections,
            connect_timeout: Duration::from_secs(config.connect_timeout),
            idle_timeout: Duration::from_secs(config.idle_timeout),
            max_lifetime: Duration::from_secs(config.max_lifetime),
            test_before_acquire: true, // Always test connections before use
            statement_cache_capacity: config.statement_cache_capacity,
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
