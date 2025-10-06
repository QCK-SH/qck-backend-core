// Rate Limiting Service for QCK Backend
// DEV-115: Build Rate Limiting Middleware with Redis-based sliding window counters

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tracing::{error, info, instrument, warn};

use crate::db::RedisPool;
use crate::services::analytics::{RateLimitAnalytics, RateLimitEvent as AnalyticsEvent};

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Redis connection error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Rate limit exceeded")]
    LimitExceeded,

    #[error("Invalid rate limit key")]
    InvalidKey,
}

// =============================================================================
// CONFIGURATION STRUCTURES
// =============================================================================

/// Comprehensive rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum requests allowed in the time window
    pub max_requests: u32,

    /// Time window in seconds
    pub window_seconds: u32,

    /// Optional burst limit (allows short bursts beyond normal limit)
    pub burst_limit: Option<u32>,

    /// Block duration in seconds when limit is exceeded
    pub block_duration: u32,

    /// Whether to enable distributed rate limiting
    pub distributed: bool,
}

impl RateLimitConfig {
    /// Create authentication endpoint configuration (stricter limits)
    pub fn auth_endpoint() -> Self {
        Self {
            max_requests: 5,
            window_seconds: 900, // 15 minutes
            burst_limit: None,
            block_duration: 1800, // 30 minutes
            distributed: true,
        }
    }

    /// Create link creation configuration (tiered by subscription)
    pub fn link_creation() -> Self {
        Self {
            max_requests: 100,    // Free tier default
            window_seconds: 3600, // 1 hour
            burst_limit: Some(10),
            block_duration: 60,
            distributed: true,
        }
    }

    /// Create redirect endpoint configuration (very high limits)
    pub fn redirect_endpoint() -> Self {
        Self {
            max_requests: 10000,
            window_seconds: 60,
            burst_limit: Some(100),
            block_duration: 60,
            distributed: false, // Single instance can handle redirects
        }
    }

    /// Create default API endpoint configuration
    pub fn default_api() -> Self {
        Self {
            max_requests: 1000,
            window_seconds: 3600,
            burst_limit: Some(20),
            block_duration: 300,
            distributed: true,
        }
    }
}

/// Rate limit check result
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,

    /// Remaining requests in current window
    pub remaining: u32,

    /// Window reset time (Unix timestamp)
    pub reset_time: u64,

    /// Retry after seconds (if blocked)
    pub retry_after: Option<u32>,

    /// Current request count in window
    pub current_count: u32,
}

/// Rate limit analytics event
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitEvent {
    pub key: String,
    pub endpoint: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub blocked: bool,
    pub current_count: u32,
    pub limit: u32,
    pub latency_ms: u64,
}

// =============================================================================
// RATE LIMITING SERVICE
// =============================================================================

/// High-performance rate limiting service with atomic Redis operations
pub struct RateLimitService {
    redis_pool: RedisPool,
    default_config: RateLimitConfig,
    endpoint_configs: HashMap<String, RateLimitConfig>,
    analytics: Option<RateLimitAnalytics>,
}

impl RateLimitService {
    /// Create new rate limiting service with Redis backend
    pub fn new(redis_pool: RedisPool) -> Self {
        let mut endpoint_configs = HashMap::new();

        // Authentication endpoints (stricter limits)
        endpoint_configs.insert(
            "/api/auth/login".to_string(),
            RateLimitConfig::auth_endpoint(),
        );
        endpoint_configs.insert(
            "/api/auth/register".to_string(),
            RateLimitConfig {
                max_requests: 3,
                window_seconds: 3600, // 1 hour
                burst_limit: None,
                block_duration: 3600,
                distributed: true,
            },
        );

        // Link creation endpoint (tiered by subscription)
        endpoint_configs.insert("/api/links".to_string(), RateLimitConfig::link_creation());

        // Redirect endpoint (very high limits)
        endpoint_configs.insert("/r/*".to_string(), RateLimitConfig::redirect_endpoint());

        // Default configuration for unspecified endpoints
        let default_config = RateLimitConfig::default_api();

        Self {
            redis_pool,
            default_config,
            endpoint_configs,
            analytics: None,
        }
    }

    /// Create new rate limiting service with analytics enabled
    pub fn new_with_analytics(redis_pool: RedisPool, sample_rate: f64) -> Self {
        let analytics = RateLimitAnalytics::new(redis_pool.clone(), sample_rate);

        let mut service = Self::new(redis_pool);
        service.analytics = Some(analytics);
        service
    }

    /// Check rate limit with custom configuration
    pub async fn check_rate_limit_with_config(
        &self,
        key: &str,
        config: &RateLimitConfig,
    ) -> Result<RateLimitResult, RateLimitError> {
        self.sliding_window_check(key, config).await
    }

    /// Check rate limit using atomic Redis Lua script
    #[instrument(skip(self), fields(key, endpoint))]
    pub async fn check_rate_limit(
        &self,
        key: &str,
        endpoint: &str,
    ) -> Result<RateLimitResult, RateLimitError> {
        let start_time = std::time::Instant::now();
        let config = self.get_config_for_endpoint(endpoint);

        let result = self.sliding_window_check(key, config).await?;

        // Record analytics event
        let latency_ms = start_time.elapsed().as_millis() as u64;

        // Send event to analytics pipeline if enabled
        if let Some(ref analytics) = self.analytics {
            let analytics_event = AnalyticsEvent {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now(),
                key: key.to_string(),
                endpoint: endpoint.to_string(),
                blocked: !result.allowed,
                current_count: result.current_count,
                limit: config.max_requests,
                user_tier: None, // Will be populated by middleware if available
                client_ip: None, // Will be populated by middleware
                check_latency_ms: latency_ms,
                metadata: std::collections::HashMap::new(),
            };

            // Record asynchronously to avoid blocking the request
            if let Err(e) = analytics.record_event(analytics_event).await {
                warn!("Failed to record analytics event: {}", e);
            }
        }

        // Log performance metrics
        if latency_ms > 5 {
            warn!(
                "Rate limit check exceeded 5ms target: {}ms for key: {}",
                latency_ms, key
            );
        }

        Ok(result)
    }

    /// Atomic sliding window rate limiting with burst support using Lua script
    async fn sliding_window_check(
        &self,
        key: &str,
        config: &RateLimitConfig,
    ) -> Result<RateLimitResult, RateLimitError> {
        let mut conn = self.redis_pool.get_connection().await?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let window_start = now - (config.window_seconds as u64 * 1000);
        let window_key = format!("rate_limit:{}", key);

        // Atomic Lua script for sliding window with burst support
        let script = r#"
            local key = KEYS[1]
            local now = tonumber(ARGV[1])
            local window_start = tonumber(ARGV[2])
            local max_requests = tonumber(ARGV[3])
            local window_seconds = tonumber(ARGV[4])
            local burst_limit = tonumber(ARGV[5]) or max_requests
            local block_duration = tonumber(ARGV[6])
            
            -- Remove old entries outside the sliding window
            redis.call('ZREMRANGEBYSCORE', key, '-inf', window_start)
            
            -- Count current requests in window
            local current_count = redis.call('ZCARD', key)
            
            -- Check if currently blocked
            local block_key = key .. ':blocked'
            local is_blocked = redis.call('EXISTS', block_key)
            
            if is_blocked == 1 then
                local block_ttl = redis.call('TTL', block_key)
                return {0, 0, now + (window_seconds * 1000), block_ttl > 0 and block_ttl or block_duration, current_count}
            end
            
            -- Determine if request is allowed (with burst consideration)
            local effective_limit = math.min(burst_limit, max_requests + (burst_limit - max_requests))
            local allowed = current_count < effective_limit
            
            if allowed then
                -- Add current request with unique identifier using timestamp and random number
                local rand = math.random(1000000)
                local request_id = now .. ':' .. rand
                redis.call('ZADD', key, now, request_id)
                current_count = current_count + 1
                
                -- Set window expiration for key using millisecond precision
                local expire_at = now + (window_seconds * 1000)
                redis.call('PEXPIREAT', key, expire_at)
            else
                -- Block the key for block_duration
                redis.call('SETEX', block_key, block_duration, '1')
            end
            
            -- Calculate remaining requests and reset time
            local remaining = math.max(0, effective_limit - current_count)
            local reset_time = now + (window_seconds * 1000)
            local retry_after = allowed and 0 or block_duration
            
            return {allowed and 1 or 0, remaining, reset_time, retry_after, current_count}
        "#;

        // Execute atomic Lua script
        let burst_limit = config.burst_limit.unwrap_or(config.max_requests);
        let result: Vec<u64> = redis::Script::new(script)
            .key(&window_key)
            .arg(now)
            .arg(window_start)
            .arg(config.max_requests)
            .arg(config.window_seconds)
            .arg(burst_limit)
            .arg(config.block_duration)
            .invoke_async(&mut conn)
            .await?;

        // Parse Lua script result
        let allowed = result[0] == 1;
        let remaining = result[1] as u32;
        let reset_time = result[2] / 1000; // Convert milliseconds back to seconds for API
        let retry_after = if result[3] > 0 {
            Some(result[3] as u32)
        } else {
            None
        };
        let current_count = result[4] as u32;

        Ok(RateLimitResult {
            allowed,
            remaining,
            reset_time,
            retry_after,
            current_count,
        })
    }

    /// Get configuration for specific endpoint with fallback logic
    fn get_config_for_endpoint(&self, endpoint: &str) -> &RateLimitConfig {
        // Direct endpoint match
        if let Some(config) = self.endpoint_configs.get(endpoint) {
            return config;
        }

        // Pattern matching for dynamic routes
        if endpoint.starts_with("/r/") {
            if let Some(config) = self.endpoint_configs.get("/r/*") {
                return config;
            }
        }

        // Authentication endpoints pattern
        if endpoint.starts_with("/api/auth/") {
            if let Some(config) = self.endpoint_configs.get("/api/auth/login") {
                return config;
            }
        }

        // Default configuration
        &self.default_config
    }

    /// Check user-specific rate limit (OSS: all users get same default config)
    #[instrument(skip(self))]
    pub async fn check_user_rate_limit(
        &self,
        user_id: &str,
        _subscription_tier: &str,
        endpoint: &str,
    ) -> Result<RateLimitResult, RateLimitError> {
        let key = format!("user:{}:{}", user_id, endpoint);
        self.sliding_window_check(&key, &self.default_config).await
    }

    /// Get rate limiting statistics for monitoring
    pub async fn get_statistics(&self) -> Result<HashMap<String, u64>, RateLimitError> {
        let mut conn = self.redis_pool.get_connection().await?;

        // Get basic Redis statistics
        let mut stats = HashMap::new();
        let mut total_count: u64 = 0;
        let mut blocked_count: u64 = 0;

        // Use SCAN to count keys without blocking Redis
        let mut cursor = 0u64;
        loop {
            let result: redis::RedisResult<(u64, Vec<String>)> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("rate_limit:*")
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await;

            match result {
                Ok((new_cursor, keys)) => {
                    for key in &keys {
                        if key.contains(":blocked") {
                            blocked_count += 1;
                        } else {
                            total_count += 1;
                        }
                    }
                    cursor = new_cursor;
                    if cursor == 0 {
                        break;
                    }
                },
                Err(e) => return Err(RateLimitError::Redis(e)),
            }
        }

        stats.insert("total_keys".to_string(), total_count);
        stats.insert("blocked_keys".to_string(), blocked_count);

        Ok(stats)
    }

    /// Clear rate limit for a specific key (admin function)
    pub async fn clear_rate_limit(&self, key: &str) -> Result<(), RateLimitError> {
        let mut conn = self.redis_pool.get_connection().await?;

        let window_key = format!("rate_limit:{}", key);
        let block_key = format!("{}:blocked", window_key);

        let _: () = conn.del(&[&window_key, &block_key]).await?;

        info!("Cleared rate limit for key: {}", key);
        Ok(())
    }

    /// Get analytics metrics if analytics are enabled
    pub async fn get_analytics_metrics(
        &self,
        window_minutes: u64,
    ) -> Option<crate::services::analytics::RateLimitMetrics> {
        if let Some(ref analytics) = self.analytics {
            analytics.get_metrics(window_minutes).await.ok()
        } else {
            None
        }
    }

    /// Get monitoring statistics if analytics are enabled
    pub async fn get_monitoring_stats(
        &self,
    ) -> Option<crate::services::analytics::MonitoringStats> {
        if let Some(ref analytics) = self.analytics {
            analytics.get_monitoring_stats().await.ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_creation() {
        let auth_config = RateLimitConfig::auth_endpoint();
        assert_eq!(auth_config.max_requests, 5);
        assert_eq!(auth_config.window_seconds, 900);
        assert_eq!(auth_config.block_duration, 1800);

        let link_config = RateLimitConfig::link_creation();
        assert_eq!(link_config.max_requests, 100);
        assert!(link_config.burst_limit.is_some());
        assert_eq!(link_config.burst_limit.unwrap(), 10);
    }

}
