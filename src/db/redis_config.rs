use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    /// Create configuration from centralized app config
    pub fn from_env() -> Self {
        let config = &crate::CONFIG.redis;

        Self {
            redis_url: config.url.clone(),
            pool_size: config.pool_size,
            connection_timeout: Duration::from_secs(config.connection_timeout),
            command_timeout: Duration::from_secs(config.command_timeout),
            retry_attempts: config.retry_attempts,
            retry_delay: Duration::from_millis(config.retry_delay_ms),
            idle_timeout: Duration::from_secs(config.idle_timeout),
            max_lifetime: Duration::from_secs(config.max_lifetime),
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
