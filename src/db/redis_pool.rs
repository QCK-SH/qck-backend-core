use rand::{thread_rng, Rng};
use redis::{aio::ConnectionManager, AsyncCommands, Client, RedisError};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, instrument, warn};

use super::redis_config::RedisConfig;

/// Maximum delay cap for exponential backoff to prevent extremely long waits
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Timeout for connection validation checks
const VALIDATION_TIMEOUT: Duration = Duration::from_millis(100);

/// Helper function to create a task join error with operation context
fn task_join_error(operation_index: usize, join_error: &tokio::task::JoinError) -> RedisError {
    error!(
        "Task join error in operation {}: {}",
        operation_index, join_error
    );
    RedisError::from((
        redis::ErrorKind::IoError,
        "Task join error - check logs for operation details",
    ))
}

/// Redis connection pool manager
pub struct RedisPool {
    connections: Arc<RwLock<Vec<ConnectionManager>>>,
    client: Client,
    config: RedisConfig,
    // FIXED: Use AtomicUsize to prevent race conditions
    active_count: Arc<AtomicUsize>,
    connections_created: Arc<RwLock<u64>>,
    connections_failed: Arc<RwLock<u64>>,
}

/// Health check status for Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisHealth {
    pub is_healthy: bool,
    pub latency_ms: u64,
    pub active_connections: u32,
    pub total_connections: u32,
    pub error: Option<String>,
}

/// Pool metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisMetrics {
    pub connections_created: u64,
    pub connections_failed: u64,
    pub connections_active: u64,
    pub connections_idle: u64,
    pub pool_size: u64,
}

impl RedisPool {
    /// Create a new Redis connection pool with retry logic
    #[instrument(skip(config))]
    pub async fn new(config: RedisConfig) -> Result<Self, RedisError> {
        // Validate configuration
        config.validate().map_err(|e| {
            error!("Invalid Redis configuration: {}", e);
            RedisError::from((
                redis::ErrorKind::InvalidClientConfig,
                "Invalid configuration",
            ))
        })?;

        info!("Initializing Redis connection pool");
        info!("Redis URL: {}", mask_redis_url(&config.redis_url));
        info!("Pool size: {}", config.pool_size);

        // Create Redis client
        let client = Client::open(config.redis_url.as_str())?;

        // Create connection pool with retry
        let connections = Arc::new(RwLock::new(Vec::new()));
        let pool = Self {
            connections: connections.clone(),
            client: client.clone(),
            config: config.clone(),
            // FIXED: Use AtomicUsize for thread-safe counter
            active_count: Arc::new(AtomicUsize::new(0)),
            connections_created: Arc::new(RwLock::new(0)),
            connections_failed: Arc::new(RwLock::new(0)),
        };

        // Initialize connections
        pool.initialize_pool().await?;

        info!("Redis connection pool initialized successfully");
        Ok(pool)
    }

    /// Initialize the connection pool
    async fn initialize_pool(&self) -> Result<(), RedisError> {
        let mut connections = Vec::new();
        let mut successful = 0;

        for i in 0..self.config.pool_size {
            match self.create_connection_with_retry().await {
                Ok(conn) => {
                    connections.push(conn);
                    successful += 1;

                    // Track successful connection creation
                    let mut created = self.connections_created.write().await;
                    *created += 1;

                    if successful % 10 == 0 {
                        info!("Created {} Redis connections", successful);
                    }
                }
                Err(e) => {
                    warn!("Failed to create connection {}: {}", i, e);

                    // Track failed connection attempt
                    let mut failed = self.connections_failed.write().await;
                    *failed += 1;

                    if successful < 1 {
                        return Err(e);
                    }
                }
            }
        }

        let mut pool = self.connections.write().await;
        *pool = connections;

        info!("Redis pool initialized with {} connections", successful);
        Ok(())
    }

    /// Create a connection with retry logic
    async fn create_connection_with_retry(&self) -> Result<ConnectionManager, RedisError> {
        let mut retry_count = 0;
        let mut delay = self.config.retry_delay;

        loop {
            match ConnectionManager::new(self.client.clone()).await {
                Ok(conn) => return Ok(conn),
                Err(e) if retry_count < self.config.retry_attempts => {
                    warn!(
                        "Failed to create Redis connection (attempt {}/{}): {}",
                        retry_count + 1,
                        self.config.retry_attempts,
                        e
                    );

                    sleep(delay).await;

                    // Exponential backoff with jitter and maximum delay cap
                    let jitter = thread_rng().gen_range(0..100);
                    delay =
                        std::cmp::min(delay * 2 + Duration::from_millis(jitter), MAX_RETRY_DELAY);
                    retry_count += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to create Redis connection after {} attempts",
                        self.config.retry_attempts
                    );
                    return Err(e);
                }
            }
        }
    }

    /// Get a connection from the pool
    ///
    /// # Behavior when pool is exhausted
    ///
    /// This method may create connections beyond the configured `pool_size` when the pool is exhausted.
    /// Specifically, if all pooled connections are checked out (i.e., the pool is empty) and a new
    /// connection is requested, a temporary connection will be created to maintain availability.
    /// This can happen under high load or if connections are not returned to the pool promptly.
    ///
    /// # Resource management implications
    ///
    /// Creating connections beyond `pool_size` can lead to increased resource usage, such as higher
    /// memory consumption and more open connections to the Redis server. If the pool is exhausted
    /// frequently or for extended periods, this may result in resource exhaustion or degraded performance.
    ///
    /// # Monitoring
    ///
    /// - The number of extra (temporary) connections created can be monitored via the `connections_created` counter.
    /// - The current number of active connections is tracked by the `active_count` field.
    /// - Log messages with level `warn` are emitted when the pool is exhausted and a temporary connection is created.
    ///   Monitoring logs for these warnings can help identify when the pool is being exceeded.
    ///
    /// Consider tuning `pool_size` or reviewing connection usage patterns if you observe frequent pool exhaustion.
    pub async fn get_connection(&self) -> Result<ConnectionManager, RedisError> {
        // First try with read lock to check availability
        {
            let pool = self.connections.read().await;
            if pool.is_empty() {
                // Pool is empty, drop read lock and create new connection
                drop(pool);
                warn!("Redis pool exhausted, creating temporary connection beyond pool size");

                let conn = self.create_connection_with_retry().await?;

                // Track new connection creation
                let mut created = self.connections_created.write().await;
                *created += 1;

                // FIXED: Use atomic operation for thread safety
                self.active_count.fetch_add(1, Ordering::Relaxed);

                return Ok(conn);
            }
        }

        // Pool has connections, acquire write lock to pop one
        let mut pool = self.connections.write().await;

        if let Some(conn) = pool.pop() {
            // FIXED: Use atomic operation for thread safety
            self.active_count.fetch_add(1, Ordering::Relaxed);
            Ok(conn)
        } else {
            // Race condition: pool became empty between locks
            drop(pool);
            warn!("Redis pool exhausted after re-check, creating temporary connection");

            let conn = self.create_connection_with_retry().await?;
            let mut created = self.connections_created.write().await;
            *created += 1;

            // FIXED: Use atomic operation for thread safety
            self.active_count.fetch_add(1, Ordering::Relaxed);

            Ok(conn)
        }
    }

    /// Return a connection to the pool
    pub async fn return_connection(&self, conn: ConnectionManager) {
        // Only validate if connection has been idle for a while or under low load
        // This reduces overhead while maintaining safety
        let should_validate = {
            let active = self.active_count.load(Ordering::Relaxed);
            let total = self.connections.read().await.len();
            // Validate only when pool utilization is low (< 50%)
            active < total / 2
        };

        if should_validate {
            // Validate connection before returning to pool
            let mut conn_to_validate = conn;
            if let Err(e) = self.validate_connection(&mut conn_to_validate).await {
                warn!("Not returning unhealthy connection to pool: {}", e);
                self.active_count.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            // Return validated connection to pool
            self.return_to_pool(conn_to_validate).await;
        } else {
            // Skip validation under high load for better performance
            self.return_to_pool(conn).await;
        }
    }

    /// Internal method to return connection to pool
    async fn return_to_pool(&self, conn: ConnectionManager) {
        let mut pool = self.connections.write().await;

        if pool.len() < self.config.pool_size as usize {
            pool.push(conn);
            self.active_count.fetch_sub(1, Ordering::Relaxed);
        } else {
            // Pool is full, let connection drop and decrement counter
            self.active_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Validate connection health - lightweight check
    async fn validate_connection(&self, conn: &mut ConnectionManager) -> Result<(), RedisError> {
        // Use lightweight PING command with timeout
        match tokio::time::timeout(
            VALIDATION_TIMEOUT,
            redis::cmd("PING").query_async::<_, String>(conn)
        ).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(RedisError::from((
                redis::ErrorKind::IoError, 
                "Connection validation timeout",
                format!("Connection validation timeout after {}ms", VALIDATION_TIMEOUT.as_millis())
            )))
        }
    }

    /// Execute a command with automatic connection management
    pub async fn execute<T, F, Fut>(&self, f: F) -> Result<T, RedisError>
    where
        F: FnOnce(ConnectionManager) -> Fut,
        Fut: std::future::Future<Output = Result<(T, ConnectionManager), RedisError>>,
    {
        let conn = self.get_connection().await?;

        match f(conn).await {
            Ok((result, conn)) => {
                self.return_connection(conn).await;
                Ok(result)
            }
            Err(e) => {
                // Don't return failed connections to the pool
                error!("Redis command failed: {}", e);
                Err(e)
            }
        }
    }

    /// Perform a health check on Redis
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> RedisHealth {
        let start = Instant::now();

        match self
            .execute(|mut conn| async move {
                let pong: String = redis::cmd("PING").query_async(&mut conn).await?;
                Ok((pong, conn))
            })
            .await
        {
            Ok(_) => {
                let latency = start.elapsed();
                let pool = self.connections.read().await;
                // FIXED: Use atomic load for thread safety
                let active = self.active_count.load(Ordering::Relaxed);

                RedisHealth {
                    is_healthy: true,
                    latency_ms: latency.as_millis() as u64,
                    active_connections: active as u32,
                    total_connections: pool.len() as u32,
                    error: None,
                }
            }
            Err(e) => {
                error!("Redis health check failed: {}", e);
                RedisHealth {
                    is_healthy: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    active_connections: 0,
                    total_connections: 0,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Test high-throughput operations
    pub async fn test_high_throughput(&self, operations: usize) -> Result<Duration, RedisError> {
        let start = Instant::now();
        let mut tasks = Vec::new();

        for i in 0..operations {
            let pool = self.clone();
            let task = tokio::spawn(async move {
                pool.execute(|mut conn| async move {
                    let key = format!("test:key:{}", i);
                    let _: () = conn.set(&key, i).await?;
                    let _: i32 = conn.get(&key).await?;
                    let _: () = conn.del(&key).await?;
                    Ok(((), conn))
                })
                .await
            });
            tasks.push((i, task));
        }

        // Wait for all operations to complete
        for (operation_index, task) in tasks {
            task.await
                .map_err(|e| task_join_error(operation_index, &e))??;
        }

        Ok(start.elapsed())
    }

    /// Get pool metrics
    pub async fn get_metrics(&self) -> RedisMetrics {
        let pool = self.connections.read().await;
        // FIXED: Use atomic load for thread safety
        let active = self.active_count.load(Ordering::Relaxed);
        let created = self.connections_created.read().await;
        let failed = self.connections_failed.read().await;

        RedisMetrics {
            connections_created: *created,
            connections_failed: *failed,
            connections_active: active as u64,
            connections_idle: pool.len() as u64,
            pool_size: self.config.pool_size as u64,
        }
    }

    /// Shutdown the pool gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down Redis connection pool");
        let mut pool = self.connections.write().await;
        pool.clear();
        info!("Redis connection pool shut down");
    }
}

impl Clone for RedisPool {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            client: self.client.clone(),
            config: self.config.clone(),
            active_count: self.active_count.clone(),
            connections_created: self.connections_created.clone(),
            connections_failed: self.connections_failed.clone(),
        }
    }
}

/// Mask Redis URL for logging
fn mask_redis_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("***");
        let port = parsed.port().unwrap_or(6379);

        // Check if URL has authentication
        if !parsed.username().is_empty() || parsed.password().is_some() {
            format!("redis://***:***@{}:{}", host, port)
        } else {
            format!("redis://{}:{}", host, port)
        }
    } else {
        // Don't expose any part of invalid URL
        "redis://***:***@***:***".to_string()
    }
}
