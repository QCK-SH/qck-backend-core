use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub redis_url: String,
    pub pool_size: u32,
    pub connection_timeout: Duration,
    pub command_timeout: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl RedisConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://redis:6379".to_string()),
            pool_size: std::env::var("REDIS_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50), // Default 50 for high performance
            connection_timeout: Duration::from_secs(
                std::env::var("REDIS_CONNECTION_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5)
            ),
            // Command timeout should allow for complex operations while being responsive
            // 5 seconds allows for most Redis operations including slower ones
            command_timeout: Duration::from_secs(
                std::env::var("REDIS_COMMAND_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5)
            ),
            retry_attempts: std::env::var("REDIS_RETRY_ATTEMPTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            retry_delay: Duration::from_millis(
                std::env::var("REDIS_RETRY_DELAY_MS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100)
            ),
            idle_timeout: Duration::from_secs(
                std::env::var("REDIS_IDLE_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(300) // 5 minutes
            ),
            max_lifetime: Duration::from_secs(
                std::env::var("REDIS_MAX_LIFETIME")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600) // 1 hour
            ),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.redis_url.is_empty() {
            return Err("Redis URL cannot be empty".to_string());
        }
        if self.pool_size == 0 {
            return Err("Pool size must be greater than 0".to_string());
        }
        if self.pool_size > 1000 {
            return Err("Pool size too large (max: 1000)".to_string());
        }
        if self.connection_timeout.as_secs() == 0 {
            return Err("Connection timeout must be greater than 0".to_string());
        }
        if self.command_timeout.as_secs() == 0 {
            return Err("Command timeout must be greater than 0".to_string());
        }
        if self.retry_attempts == 0 {
            return Err("Retry attempts must be greater than 0".to_string());
        }
        Ok(())
    }
}