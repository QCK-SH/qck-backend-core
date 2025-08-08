use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::{Duration, Instant};
use tracing::{error, info, instrument, warn};
use tokio::time::sleep;
use serde::{Deserialize, Serialize};
use rand::{thread_rng, Rng};

use super::config::DatabaseConfig;

/// PostgreSQL connection pool manager
pub struct PostgresPool {
    pool: PgPool,
    config: DatabaseConfig,
}

/// Health check status for the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub is_healthy: bool,
    pub latency_ms: u64,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub max_connections: u32,
    pub error: Option<String>,
}

/// Pool metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    pub connections_created: u64,
    pub connections_closed: u64,
    pub acquire_count: u64,
    pub acquire_duration_ms: u64,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub wait_queue_length: u32,
}

impl PostgresPool {
    /// Create a new PostgreSQL connection pool with retry logic
    #[instrument(skip(config))]
    pub async fn new(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        // Validate configuration
        config.validate()
            .map_err(|e| sqlx::Error::Configuration(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                e
            ))))?;
        
        info!("Initializing PostgreSQL connection pool");
        info!("Database URL: {}", mask_connection_string(&config.database_url));
        info!("Pool size: min={}, max={}", config.min_connections, config.max_connections);
        
        // Create pool with exponential backoff retry
        let pool = Self::create_pool_with_retry(&config, 3, Duration::from_secs(1)).await?;
        
        // Test the connection
        Self::test_connection(&pool).await?;
        
        info!("PostgreSQL connection pool initialized successfully");
        
        Ok(Self { pool, config })
    }
    
    /// Create pool with exponential backoff retry logic
    async fn create_pool_with_retry(
        config: &DatabaseConfig,
        max_retries: u32,
        initial_delay: Duration,
    ) -> Result<PgPool, sqlx::Error> {
        let mut retry_count = 0;
        let mut delay = initial_delay;
        
        loop {
            match Self::create_pool(config).await {
                Ok(pool) => return Ok(pool),
                Err(e) if retry_count < max_retries => {
                    warn!(
                        "Failed to create database pool (attempt {}/{}): {}",
                        retry_count + 1,
                        max_retries + 1,
                        e
                    );
                    
                    sleep(delay).await;
                    
                    // Exponential backoff with jitter
                    let jitter = thread_rng().gen_range(0..1000);
                    delay = delay * 2 + Duration::from_millis(jitter);
                    retry_count += 1;
                }
                Err(e) => {
                    error!("Failed to create database pool after {} attempts", max_retries + 1);
                    return Err(e);
                }
            }
        }
    }
    
    /// Create the actual database pool
    async fn create_pool(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
        PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.connect_timeout)
            .idle_timeout(Some(config.idle_timeout))
            .max_lifetime(Some(config.max_lifetime))
            .test_before_acquire(config.test_before_acquire)
            .before_acquire(|_conn, _meta| {
                Box::pin(async move {
                    // Custom connection validation
                    // You can add custom checks here
                    Ok(true)
                })
            })
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    // Set connection parameters after connecting
                    // e.g., SET statement_timeout, timezone, etc.
                    sqlx::query("SET statement_timeout = '30s'")
                        .execute(conn)
                        .await?;
                    
                    Ok(())
                })
            })
            .connect(&config.database_url)
            .await
    }
    
    /// Test the database connection
    async fn test_connection(pool: &PgPool) -> Result<(), sqlx::Error> {
        let start = Instant::now();
        
        sqlx::query("SELECT 1")
            .fetch_one(pool)
            .await?;
        
        let latency = start.elapsed();
        info!("Database connection test successful (latency: {:?})", latency);
        
        Ok(())
    }
    
    /// Get a reference to the underlying pool
    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }
    
    /// Clone the pool (cheap operation, pools are Arc internally)
    pub fn clone_pool(&self) -> PgPool {
        self.pool.clone()
    }
    
    /// Perform a health check on the database
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> DatabaseHealth {
        Self::health_check_with_pool(&self.pool, self.config.max_connections).await
    }
    
    /// Static method to perform health check with an existing pool
    pub async fn health_check_with_pool(pool: &PgPool, max_connections: u32) -> DatabaseHealth {
        let start = Instant::now();
        
        let health = match sqlx::query("SELECT 1 as health")
            .fetch_one(pool)
            .await
        {
            Ok(_) => {
                let latency = start.elapsed();
                DatabaseHealth {
                    is_healthy: true,
                    latency_ms: latency.as_millis() as u64,
                    active_connections: pool.size() - (pool.num_idle() as u32),
                    idle_connections: pool.num_idle() as u32,
                    max_connections,
                    error: None,
                }
            }
            Err(e) => {
                error!("Database health check failed: {}", e);
                DatabaseHealth {
                    is_healthy: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    active_connections: 0,
                    idle_connections: 0,
                    max_connections,
                    error: Some(e.to_string()),
                }
            }
        };
        
        if !health.is_healthy {
            warn!("Database health check failed: {:?}", health.error);
        }
        
        health
    }
    
    /// Get pool metrics for monitoring
    pub fn get_metrics(&self) -> PoolMetrics {
        PoolMetrics {
            connections_created: 0, // These would need custom tracking
            connections_closed: 0,
            acquire_count: 0,
            acquire_duration_ms: 0,
            active_connections: (self.pool.size() - self.pool.num_idle() as u32),
            idle_connections: self.pool.num_idle() as u32,
            wait_queue_length: 0, // Would need custom tracking
        }
    }
    
    /// Execute a query with retry logic
    #[instrument(skip(self, query_fn))]
    pub async fn execute_with_retry<F, Fut>(
        &self,
        query_fn: F,
        max_retries: u32,
    ) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<sqlx::postgres::PgQueryResult, sqlx::Error>>,
    {
        let mut retry_count = 0;
        let mut delay = Duration::from_millis(100);
        
        loop {
            match query_fn().await {
                Ok(result) => return Ok(result),
                Err(e) if Self::is_retryable_error(&e) && retry_count < max_retries => {
                    warn!("Retryable database error (attempt {}): {}", retry_count + 1, e);
                    sleep(delay).await;
                    delay = delay * 2;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }
    
    /// Check if an error is retryable
    fn is_retryable_error(error: &sqlx::Error) -> bool {
        matches!(
            error,
            sqlx::Error::PoolTimedOut
                | sqlx::Error::PoolClosed
                | sqlx::Error::Io(_)
                | sqlx::Error::Protocol(_)
        )
    }
    
    /// Gracefully shutdown the pool
    pub async fn shutdown(&self) {
        info!("Shutting down database connection pool");
        self.pool.close().await;
        info!("Database connection pool closed");
    }
}

/// Mask sensitive parts of connection string for logging
pub fn mask_connection_string(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("unknown");
        let path = parsed.path();
        
        // Check if URL has username/password
        if parsed.username() != "" || parsed.password().is_some() {
            format!("postgresql://***:***@{}{}", host, path)
        } else {
            format!("postgresql://{}{}", host, path)
        }
    } else {
        "postgresql://***:***@***".to_string()
    }
}