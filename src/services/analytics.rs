// Analytics and Monitoring Service for Rate Limiting
// DEV-115: Monitoring and metrics collection for rate limiting middleware

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
};
use thiserror::Error;
use tracing::{error, info, warn};

use crate::db::RedisPool;

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("Redis connection error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Analytics service unavailable")]
    ServiceUnavailable,
}

// =============================================================================
// ANALYTICS DATA STRUCTURES
// =============================================================================

/// Rate limiting event for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEvent {
    /// Unique event ID
    pub id: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Rate limit key that was checked
    pub key: String,

    /// Endpoint that was accessed
    pub endpoint: String,

    /// Whether the request was blocked
    pub blocked: bool,

    /// Current request count in window
    pub current_count: u32,

    /// Maximum allowed requests
    pub limit: u32,

    /// User subscription tier (if available)
    pub user_tier: Option<String>,

    /// Client IP address
    pub client_ip: Option<String>,

    /// Latency of rate limit check in milliseconds
    pub check_latency_ms: u64,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Aggregated rate limiting metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitMetrics {
    /// Total number of rate limit checks
    pub total_checks: u64,

    /// Number of requests blocked
    pub total_blocked: u64,

    /// Average latency of rate limit checks
    pub avg_latency_ms: f64,

    /// P95 latency
    pub p95_latency_ms: u64,

    /// P99 latency
    pub p99_latency_ms: u64,

    /// Metrics by endpoint
    pub endpoint_metrics: HashMap<String, EndpointMetrics>,

    /// Metrics by subscription tier
    pub tier_metrics: HashMap<String, TierMetrics>,

    /// Time window for these metrics
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

/// Metrics for a specific endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMetrics {
    pub endpoint: String,
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub avg_latency_ms: f64,
    pub block_rate: f64, // Percentage of requests blocked
}

/// Metrics for a specific subscription tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierMetrics {
    pub tier: String,
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub unique_users: u64,
    pub avg_latency_ms: f64,
}

/// Real-time monitoring stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStats {
    /// Current requests per second
    pub current_rps: f64,

    /// Peak requests per second in the last hour
    pub peak_rps: f64,

    /// Number of active rate limit keys
    pub active_keys: u64,

    /// Number of currently blocked keys
    pub blocked_keys: u64,

    /// System health indicators
    pub health: HealthIndicators,
}

/// Health indicators for the rate limiting system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIndicators {
    /// Whether Redis is responding
    pub redis_healthy: bool,

    /// Average Redis response time
    pub redis_latency_ms: f64,

    /// Whether rate limit checks are within SLA (<5ms)
    pub within_sla: bool,

    /// Error rate in the last 5 minutes
    pub error_rate: f64,

    /// Memory usage of rate limiting data
    pub memory_usage_mb: f64,
}

// =============================================================================
// ANALYTICS SERVICE
// =============================================================================

/// Analytics service for rate limiting monitoring
pub struct RateLimitAnalytics {
    redis_pool: RedisPool,

    // In-memory counters for real-time metrics
    total_checks: AtomicU64,
    total_blocked: AtomicU64,

    // Latency tracking
    latency_samples: Arc<RwLock<Vec<u64>>>,

    // Endpoint and tier counters
    endpoint_counters: Arc<RwLock<HashMap<String, (u64, u64)>>>, // (total, blocked)
    tier_counters: Arc<RwLock<HashMap<String, (u64, u64)>>>,     // (total, blocked)

    // Configuration
    max_samples: usize,

    // Sampling optimization - counter-based sampling
    sample_counter: AtomicU64,
    sample_interval: u64, // Sample every N events instead of RNG on each
}

impl RateLimitAnalytics {
    /// Create new analytics service
    pub fn new(redis_pool: RedisPool, sample_rate: f64) -> Self {
        let clamped_rate = sample_rate.clamp(0.0, 1.0);
        // Convert sample rate to interval (e.g., 0.1 = sample 1 in 10)
        let sample_interval = if clamped_rate > 0.0 {
            (1.0 / clamped_rate) as u64
        } else {
            u64::MAX // Never sample if rate is 0
        };

        Self {
            redis_pool,
            total_checks: AtomicU64::new(0),
            total_blocked: AtomicU64::new(0),
            latency_samples: Arc::new(RwLock::new(Vec::new())),
            endpoint_counters: Arc::new(RwLock::new(HashMap::new())),
            tier_counters: Arc::new(RwLock::new(HashMap::new())),
            max_samples: 10000, // Keep last 10k latency samples
            sample_counter: AtomicU64::new(0),
            sample_interval,
        }
    }

    /// Record a rate limit event
    pub async fn record_event(&self, event: RateLimitEvent) -> Result<(), AnalyticsError> {
        // Update in-memory counters with Acquire/Release ordering for consistency
        self.total_checks.fetch_add(1, Ordering::AcqRel);
        if event.blocked {
            self.total_blocked.fetch_add(1, Ordering::AcqRel);
        }

        // Record latency sample
        {
            match self.latency_samples.write() {
                Ok(mut samples) => {
                    samples.push(event.check_latency_ms);

                    // Keep only the most recent samples
                    if samples.len() > self.max_samples {
                        let excess = samples.len() - self.max_samples;
                        samples.drain(0..excess);
                    }
                },
                Err(e) => {
                    error!("Failed to acquire write lock for latency samples: {}", e);
                    // Continue without recording the sample rather than panicking
                },
            }
        }

        // Update endpoint counters
        {
            match self.endpoint_counters.write() {
                Ok(mut counters) => {
                    let entry = counters.entry(event.endpoint.clone()).or_insert((0, 0));
                    entry.0 += 1; // total
                    if event.blocked {
                        entry.1 += 1; // blocked
                    }
                },
                Err(e) => {
                    error!("Failed to acquire write lock for endpoint counters: {}", e);
                    // Continue without updating the counter rather than panicking
                },
            }
        }

        // Update tier counters
        if let Some(ref tier) = event.user_tier {
            match self.tier_counters.write() {
                Ok(mut counters) => {
                    let entry = counters.entry(tier.clone()).or_insert((0, 0));
                    entry.0 += 1; // total
                    if event.blocked {
                        entry.1 += 1; // blocked
                    }
                },
                Err(e) => {
                    error!("Failed to acquire write lock for tier counters: {}", e);
                    // Continue without updating the counter rather than panicking
                },
            }
        }

        // Sample events for detailed storage (respects sample rate)
        if self.should_sample() {
            self.store_event_sample(&event).await?;
        }

        // Check for alerts
        self.check_alerts(&event).await;

        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(
        &self,
        window_minutes: u64,
    ) -> Result<RateLimitMetrics, AnalyticsError> {
        let window_end = Utc::now();
        let window_start = window_end - chrono::Duration::minutes(window_minutes as i64);

        let total_checks = self.total_checks.load(Ordering::Acquire);
        let total_blocked = self.total_blocked.load(Ordering::Acquire);

        // Calculate latency percentiles
        let latency_samples = match self.latency_samples.read() {
            Ok(samples) => samples.clone(),
            Err(e) => {
                error!("Failed to acquire read lock for latency samples: {}", e);
                Vec::new() // Use empty vector as fallback
            },
        };
        let (avg_latency, p95_latency, p99_latency) =
            self.calculate_latency_stats(&latency_samples);

        // Build endpoint metrics
        let endpoint_metrics = match self.endpoint_counters.read() {
            Ok(counters) => counters
                .iter()
                .map(|(endpoint, (total, blocked))| {
                    let block_rate = if *total > 0 {
                        (*blocked as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };

                    (
                        endpoint.clone(),
                        EndpointMetrics {
                            endpoint: endpoint.clone(),
                            total_requests: *total,
                            blocked_requests: *blocked,
                            avg_latency_ms: avg_latency,
                            block_rate,
                        },
                    )
                })
                .collect(),
            Err(e) => {
                error!("Failed to acquire read lock for endpoint counters: {}", e);
                HashMap::new() // Return empty metrics as fallback
            },
        };

        // Build tier metrics
        let tier_metrics = match self.tier_counters.read() {
            Ok(counters) => {
                counters
                    .iter()
                    .map(|(tier, (total, blocked))| {
                        (
                            tier.clone(),
                            TierMetrics {
                                tier: tier.clone(),
                                total_requests: *total,
                                blocked_requests: *blocked,
                                unique_users: 0, // TODO: Track unique users
                                avg_latency_ms: avg_latency,
                            },
                        )
                    })
                    .collect()
            },
            Err(e) => {
                error!("Failed to acquire read lock for tier counters: {}", e);
                HashMap::new() // Return empty metrics as fallback
            },
        };

        Ok(RateLimitMetrics {
            total_checks,
            total_blocked,
            avg_latency_ms: avg_latency,
            p95_latency_ms: p95_latency,
            p99_latency_ms: p99_latency,
            endpoint_metrics,
            tier_metrics,
            window_start,
            window_end,
        })
    }

    /// Get real-time monitoring statistics
    pub async fn get_monitoring_stats(&self) -> Result<MonitoringStats, AnalyticsError> {
        // Calculate current RPS (requests in last minute)
        let current_rps = self.calculate_current_rps().await?;

        // Get Redis health
        let redis_health = self.check_redis_health().await;

        // Get active keys count
        let (active_keys, blocked_keys) = self.get_key_counts().await?;

        // Calculate health indicators
        let latency_samples = match self.latency_samples.read() {
            Ok(samples) => samples.clone(),
            Err(e) => {
                error!(
                    "Failed to acquire read lock for latency samples in monitoring stats: {}",
                    e
                );
                Vec::new() // Use empty vector as fallback
            },
        };
        let avg_redis_latency = self.calculate_latency_stats(&latency_samples).0;
        let within_sla = avg_redis_latency < 5.0; // <5ms requirement

        let health = HealthIndicators {
            redis_healthy: redis_health,
            redis_latency_ms: avg_redis_latency,
            within_sla,
            error_rate: 0.0,      // TODO: Calculate error rate
            memory_usage_mb: 0.0, // TODO: Calculate memory usage
        };

        Ok(MonitoringStats {
            current_rps,
            peak_rps: 0.0, // TODO: Track peak RPS
            active_keys,
            blocked_keys,
            health,
        })
    }

    /// Store event sample in Redis for detailed analysis
    async fn store_event_sample(&self, event: &RateLimitEvent) -> Result<(), AnalyticsError> {
        let mut conn = self.redis_pool.get_connection().await?;

        let key = format!(
            "analytics:rate_limit:events:{}",
            event.timestamp.format("%Y%m%d")
        );
        let value = serde_json::to_string(event)?;

        // Store event with TTL (keep for 7 days)
        redis::cmd("LPUSH")
            .arg(&key)
            .arg(&value)
            .query_async::<()>(&mut conn)
            .await?;

        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(7 * 24 * 3600) // 7 days
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// Check if we should sample this event
    fn should_sample(&self) -> bool {
        // Use counter-based sampling for better performance at high load
        // Instead of RNG on every call, sample deterministically based on counter
        if self.sample_interval == u64::MAX {
            return false; // Never sample if rate is 0
        }

        if self.sample_interval == 1 {
            return true; // Always sample if rate is 1.0
        }

        // Increment counter and check if we hit the interval
        // Using AcqRel ordering to ensure consistent sampling behavior across threads
        let count = self.sample_counter.fetch_add(1, Ordering::AcqRel);
        count % self.sample_interval == 0
    }

    /// Calculate latency statistics
    fn calculate_latency_stats(&self, samples: &[u64]) -> (f64, u64, u64) {
        if samples.is_empty() {
            return (0.0, 0, 0);
        }

        let mut sorted_samples = samples.to_vec();
        sorted_samples.sort_unstable();

        let avg = samples.iter().sum::<u64>() as f64 / samples.len() as f64;

        let p95_idx = (sorted_samples.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted_samples.len() as f64 * 0.99) as usize;

        let p95 = sorted_samples.get(p95_idx).copied().unwrap_or(0);
        let p99 = sorted_samples.get(p99_idx).copied().unwrap_or(0);

        (avg, p95, p99)
    }

    /// Calculate current requests per second
    async fn calculate_current_rps(&self) -> Result<f64, AnalyticsError> {
        // This is a simplified implementation
        // In production, you might want to use a sliding window counter
        let total_checks = self.total_checks.load(Ordering::Acquire);

        // Assume these checks happened over the last minute for simplicity
        Ok(total_checks as f64 / 60.0)
    }

    /// Check Redis health
    async fn check_redis_health(&self) -> bool {
        let redis_health = self.redis_pool.health_check().await;
        redis_health.is_healthy
    }

    /// Get count of active and blocked keys
    async fn get_key_counts(&self) -> Result<(u64, u64), AnalyticsError> {
        let mut conn = self.redis_pool.get_connection().await?;

        // Count rate limit keys using SCAN to avoid blocking Redis
        let mut active_count: u64 = 0;
        let mut blocked_count: u64 = 0;

        // Use SCAN with cursor to iterate through keys without blocking
        // Removed MAX_ITERATIONS to ensure all keys are scanned in production
        let mut cursor = 0u64;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("rate_limit:*")
                .arg("COUNT")
                .arg(100) // Process 100 keys at a time
                .query_async(&mut conn)
                .await?;

            for key in &keys {
                if key.ends_with(":blocked") {
                    blocked_count += 1;
                } else {
                    active_count += 1;
                }
            }

            cursor = new_cursor;

            // Stop when we've completed the full scan
            if cursor == 0 {
                break;
            }
        }

        Ok((active_count, blocked_count))
    }

    /// Check for alerting conditions
    async fn check_alerts(&self, event: &RateLimitEvent) {
        // Alert on high latency
        if event.check_latency_ms > 5 {
            warn!(
                "Rate limit check exceeded 5ms SLA: {}ms for endpoint: {}",
                event.check_latency_ms, event.endpoint
            );
        }

        // Alert on high block rate
        let total_checks = self.total_checks.load(Ordering::Acquire);
        let total_blocked = self.total_blocked.load(Ordering::Acquire);

        if total_checks > 100 {
            let block_rate = (total_blocked as f64 / total_checks as f64) * 100.0;
            if block_rate > 50.0 {
                error!(
                    "High block rate detected: {:.2}% of requests are being blocked",
                    block_rate
                );
            }
        }
    }

    /// Reset counters (useful for testing or periodic resets)
    pub fn reset_counters(&self) {
        self.total_checks.store(0, Ordering::Release);
        self.total_blocked.store(0, Ordering::Release);
        self.sample_counter.store(0, Ordering::Release);

        match self.latency_samples.write() {
            Ok(mut samples) => samples.clear(),
            Err(e) => error!(
                "Failed to acquire write lock for latency samples during reset: {}",
                e
            ),
        }

        match self.endpoint_counters.write() {
            Ok(mut counters) => counters.clear(),
            Err(e) => error!(
                "Failed to acquire write lock for endpoint counters during reset: {}",
                e
            ),
        }

        match self.tier_counters.write() {
            Ok(mut counters) => counters.clear(),
            Err(e) => error!(
                "Failed to acquire write lock for tier counters during reset: {}",
                e
            ),
        }

        info!("Rate limiting analytics counters reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_event(blocked: bool, latency_ms: u64) -> RateLimitEvent {
        RateLimitEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            key: "test-key".to_string(),
            endpoint: "/api/test".to_string(),
            blocked,
            current_count: 5,
            limit: 10,
            user_tier: Some("pro".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            check_latency_ms: latency_ms,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_latency_stats_calculation() {
        // Test latency calculation logic without Redis pool
        let samples = vec![1, 2, 3, 4, 5, 10, 15, 20, 100];

        // Calculate stats manually for verification
        let avg = samples.iter().sum::<u64>() as f64 / samples.len() as f64;
        assert!((avg - 17.77).abs() < 0.1); // Approximately 17.77

        // Test percentile calculation
        let mut sorted_samples = samples.clone();
        sorted_samples.sort_unstable();

        let p95_idx = (sorted_samples.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted_samples.len() as f64 * 0.99) as usize;

        let expected_p95 = sorted_samples.get(p95_idx).copied().unwrap_or(0);
        let expected_p99 = sorted_samples.get(p99_idx).copied().unwrap_or(0);

        assert!(expected_p95 > 0);
        assert!(expected_p99 > 0);
    }

    #[test]
    fn test_sample_rate_logic() {
        // Test sample rate clamping
        let sample_rate_too_low: f64 = -0.5;
        let sample_rate_too_high: f64 = 1.5;

        // Rates should be clamped between 0.0 and 1.0
        assert!(sample_rate_too_low.clamp(0.0, 1.0) == 0.0);
        assert!(sample_rate_too_high.clamp(0.0, 1.0) == 1.0);

        // Test valid sample rates
        let valid_rates = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        for rate in valid_rates {
            assert!((0.0..=1.0).contains(&rate));
        }
    }

    #[test]
    fn test_event_creation() {
        let event = create_test_event(true, 3);
        assert_eq!(event.endpoint, "/api/test");
        assert!(event.blocked);
        assert_eq!(event.check_latency_ms, 3);
        assert_eq!(event.user_tier, Some("pro".to_string()));
    }
}
