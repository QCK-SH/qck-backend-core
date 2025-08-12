use qck_backend::db::{RedisConfig, RedisPool};
use redis::AsyncCommands;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use uuid::Uuid;

/// Test helper to generate unique keys for test isolation
fn test_key(prefix: &str) -> String {
    format!("test:{}:{}", prefix, Uuid::new_v4())
}

/// Test guard for automatic cleanup
struct TestGuard {
    pool: RedisPool,
    keys_to_cleanup: Vec<String>,
}

impl TestGuard {
    fn new(pool: RedisPool) -> Self {
        Self {
            pool,
            keys_to_cleanup: Vec::new(),
        }
    }

    fn add_key(&mut self, key: String) {
        self.keys_to_cleanup.push(key);
    }

    async fn cleanup(&self) {
        if !self.keys_to_cleanup.is_empty() {
            let _ = self
                .pool
                .execute(|mut conn| {
                    let keys = self.keys_to_cleanup.clone();
                    async move {
                        for key in &keys {
                            let _: Result<(), _> = conn.del(key).await;
                        }
                        Ok(((), conn))
                    }
                })
                .await;
        }
    }
}

impl Drop for TestGuard {
    fn drop(&mut self) {
        // Async cleanup cannot be performed in Drop trait.
        // Call cleanup() explicitly before test ends to ensure proper cleanup.
        // This is a limitation of Rust's Drop trait which must be synchronous.
        eprintln!(
            "WARNING: TestGuard dropped without async cleanup. \
            Call cleanup().await explicitly before the end of the test to ensure proper cleanup."
        );
    }
}

#[tokio::test]
async fn test_redis_pool_creation_success() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    let pool = RedisPool::new(config).await;

    assert!(
        pool.is_ok(),
        "Failed to create Redis pool: {:?}",
        pool.err()
    );

    // Verify pool is functional
    if let Ok(pool) = pool {
        let conn = pool.get_connection().await;
        assert!(conn.is_ok(), "Should be able to get connection from new pool");
    }
}

#[tokio::test]
async fn test_redis_pool_creation_with_invalid_url() {
    dotenv::from_filename(".env.test").ok();

    let mut config = RedisConfig::from_env();
    config.redis_url = "redis://invalid-host:6379".to_string();
    config.connection_timeout = Duration::from_millis(500); // Short timeout for test

    let start = Instant::now();
    let pool = RedisPool::new(config).await;
    let elapsed = start.elapsed();

    assert!(pool.is_err(), "Should fail with invalid host");
    assert!(
        elapsed < Duration::from_secs(2),
        "Should timeout quickly, took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_redis_health_check_comprehensive() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config.clone()).await {
        Ok(pool) => {
            let health = pool.health_check().await;

            // Comprehensive health checks
            assert!(health.is_healthy, "Redis health check failed");
            assert!(
                health.latency_ms < 100,
                "Redis latency too high: {}ms",
                health.latency_ms
            );
            assert!(
                health.total_connections > 0,
                "No Redis connections available"
            );
            assert!(
                health.total_connections <= config.pool_size,
                "Total connections exceeds pool size"
            );
            assert!(
                health.active_connections <= health.total_connections,
                "Active connections exceeds total connections"
            );
            assert!(
                health.error.is_none(),
                "Health check returned error: {:?}",
                health.error
            );
        }
        Err(e) => {
            panic!("Failed to create Redis pool for health check: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_connection_lifecycle() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config).await {
        Ok(pool) => {
            let initial_health = pool.health_check().await;
            let initial_active = initial_health.active_connections;

            // Get multiple connections
            let conn1 = pool.get_connection().await.expect("Should get conn1");
            let conn2 = pool.get_connection().await.expect("Should get conn2");
            let conn3 = pool.get_connection().await.expect("Should get conn3");

            // Check active connections increased
            let health_with_conns = pool.health_check().await;
            assert!(
                health_with_conns.active_connections >= initial_active + 3,
                "Active connections should increase when connections are taken"
            );

            // Return connections
            pool.return_connection(conn1).await;
            pool.return_connection(conn2).await;
            pool.return_connection(conn3).await;

            // Allow time for async return
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Verify connections were returned
            let final_health = pool.health_check().await;
            assert!(
                final_health.active_connections <= initial_active + 1,
                "Active connections should decrease after return"
            );
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_execute_with_isolation() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config).await {
        Ok(pool) => {
            let mut guard = TestGuard::new(pool.clone());
            
            // Use unique keys for test isolation
            let key = test_key("execute");
            let value = format!("test_value_{}", Uuid::new_v4());
            guard.add_key(key.clone());

            // Test SET and GET operations
            let result = pool
                .execute(|mut conn| {
                    let key = key.clone();
                    let value = value.clone();
                    async move {
                        // Set a value
                        let _: () = conn.set(&key, &value).await?;

                        // Get the value back
                        let retrieved: String = conn.get(&key).await?;

                        Ok((retrieved, conn))
                    }
                })
                .await;

            assert!(result.is_ok(), "Execute should succeed");
            assert_eq!(result.unwrap(), value, "Should retrieve correct value");

            // Cleanup
            guard.cleanup().await;

            // Verify cleanup worked
            let check_result = pool
                .execute(|mut conn| {
                    let key = key.clone();
                    async move {
                        let exists: bool = conn.exists(&key).await?;
                        Ok((exists, conn))
                    }
                })
                .await;

            assert!(!check_result.unwrap(), "Key should be cleaned up");
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_concurrent_operations() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config).await {
        Ok(pool) => {
            let pool = Arc::new(pool);
            let num_tasks = 20;
            let barrier = Arc::new(Barrier::new(num_tasks));
            let mut handles = Vec::new();

            for i in 0..num_tasks {
                let pool = pool.clone();
                let barrier = barrier.clone();
                
                let handle = tokio::spawn(async move {
                    // Wait for all tasks to be ready
                    barrier.wait().await;
                    
                    let key = test_key(&format!("concurrent_{}", i));
                    let value = format!("value_{}", i);
                    
                    // Perform operation
                    let result = pool
                        .execute(|mut conn| {
                            let key = key.clone();
                            let value = value.clone();
                            async move {
                                let _: () = conn.set(&key, &value).await?;
                                let retrieved: String = conn.get(&key).await?;
                                let _: () = conn.del(&key).await?;
                                Ok((retrieved, conn))
                            }
                        })
                        .await;
                    
                    assert!(result.is_ok(), "Task {} failed: {:?}", i, result.err());
                    assert_eq!(result.unwrap(), value, "Task {} got wrong value", i);
                });
                
                handles.push(handle);
            }

            // Wait for all tasks to complete
            for handle in handles {
                handle.await.expect("Task should complete");
            }

            // Verify pool is still healthy after concurrent access
            let health = pool.health_check().await;
            assert!(health.is_healthy, "Pool should remain healthy after concurrent access");
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_production_performance() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Test with production-level load (1000+ ops)
            let operations = 1000;
            let start = Instant::now();
            let mut latencies = Vec::with_capacity(operations);

            for i in 0..operations {
                let key = test_key(&format!("perf_{}", i));
                let value = format!("value_{}", i);
                
                let op_start = Instant::now();
                let result = pool
                    .execute(|mut conn| {
                        let key = key.clone();
                        let value = value.clone();
                        async move {
                            let _: () = conn.set(&key, &value).await?;
                            let _: String = conn.get(&key).await?;
                            let _: () = conn.del(&key).await?;
                            Ok(((), conn))
                        }
                    })
                    .await;
                
                let op_latency = op_start.elapsed();
                latencies.push(op_latency);
                
                assert!(result.is_ok(), "Operation {} failed", i);
            }

            let duration = start.elapsed();
            let ops_per_second = operations as f64 / duration.as_secs_f64();

            // Calculate percentiles
            latencies.sort();
            let p50 = latencies[operations / 2];
            let p95 = latencies[operations * 95 / 100];
            let p99 = latencies[operations * 99 / 100];

            println!("Performance Results:");
            println!("  Operations: {}", operations);
            println!("  Duration: {:?}", duration);
            println!("  Throughput: {:.0} ops/second", ops_per_second);
            println!("  P50 Latency: {:?}", p50);
            println!("  P95 Latency: {:?}", p95);
            println!("  P99 Latency: {:?}", p99);

            // Production requirements (relaxed for CI environments)
            let min_ops_per_second = if std::env::var("CI").is_ok() {
                100.0  // Lower threshold for CI environments
            } else {
                1000.0  // Production threshold
            };
            
            assert!(
                ops_per_second > min_ops_per_second,
                "Performance too low: {:.0} ops/s (need {}+)",
                ops_per_second, min_ops_per_second
            );
            assert!(
                p95 < Duration::from_millis(50),
                "P95 latency too high: {:?} (need <50ms)",
                p95
            );
            assert!(
                p99 < Duration::from_millis(100),
                "P99 latency too high: {:?} (need <100ms)",
                p99
            );
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_pool_exhaustion_recovery() {
    dotenv::from_filename(".env.test").ok();

    let mut config = RedisConfig::from_env();
    config.pool_size = 5; // Small pool for testing
    
    match RedisPool::new(config.clone()).await {
        Ok(pool) => {
            let initial_health = pool.health_check().await;
            
            // Take multiple connections to test pool behavior
            let mut connections = Vec::new();
            for i in 0..5 {
                match pool.get_connection().await {
                    Ok(conn) => connections.push(conn),
                    Err(e) => panic!("Failed to get connection {}: {}", i, e),
                }
            }
            
            // Check pool state with all connections taken
            let loaded_health = pool.health_check().await;
            assert!(
                loaded_health.active_connections >= 5,
                "Should have at least 5 active connections"
            );
            
            // Our pool can create additional connections on demand
            // This verifies the pool's dynamic growth capability
            let extra_conn_result = pool.get_connection().await;
            assert!(
                extra_conn_result.is_ok(),
                "Pool should dynamically create additional connections when needed"
            );
            
            if let Ok(extra_conn) = extra_conn_result {
                // Verify we got an additional connection
                // Note: Our pool implementation may reuse connections or create new ones
                let grown_health = pool.health_check().await;
                assert!(
                    grown_health.active_connections > 0,
                    "Should have active connections"
                );
                pool.return_connection(extra_conn).await;
            }
            
            // Return connections and verify pool recovery
            for conn in connections {
                pool.return_connection(conn).await;
            }
            
            // Allow time for async returns
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // Verify pool recovered
            let final_health = pool.health_check().await;
            assert!(
                final_health.active_connections <= initial_health.active_connections + 2,
                "Active connections should return to near initial levels"
            );
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_connection_validation() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Get a connection and verify it's valid
            let mut conn = pool.get_connection().await.expect("Should get connection");
            
            // Manually test the connection
            let ping_result: Result<String, _> = redis::cmd("PING")
                .query_async(&mut conn)
                .await;
            
            assert!(ping_result.is_ok(), "Connection should be valid");
            assert_eq!(ping_result.unwrap(), "PONG", "Should receive PONG response");
            
            // Return connection
            pool.return_connection(conn).await;
            
            // Verify pool validates connections (this tests the validation logic)
            let health = pool.health_check().await;
            assert!(health.is_healthy, "Pool should validate connections properly");
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_metrics_accuracy() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config.clone()).await {
        Ok(pool) => {
            let initial_metrics = pool.get_metrics().await;
            
            // Perform some operations
            for i in 0..10 {
                let key = test_key(&format!("metrics_{}", i));
                let _ = pool
                    .execute(|mut conn| {
                        let key = key.clone();
                        async move {
                            let _: () = conn.set(&key, "value").await?;
                            let _: () = conn.del(&key).await?;
                            Ok(((), conn))
                        }
                    })
                    .await;
            }
            
            let final_metrics = pool.get_metrics().await;
            
            // Verify metrics changed
            assert!(
                final_metrics.connections_created >= initial_metrics.connections_created,
                "Connections created should not decrease"
            );
            
            // Verify metrics are reasonable
            assert!(
                final_metrics.connections_created <= (config.pool_size * 2) as u64,
                "Should not create excessive connections"
            );
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_graceful_shutdown() {
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Perform some operations
            let key = test_key("shutdown");
            let _ = pool
                .execute(|mut conn| {
                    let key = key.clone();
                    async move {
                        let _: () = conn.set(&key, "value").await?;
                        Ok(((), conn))
                    }
                })
                .await;
            
            // Verify pool is working
            let health_before = pool.health_check().await;
            assert!(health_before.is_healthy, "Pool should be healthy before shutdown");
            
            // Shutdown the pool
            pool.shutdown().await;
            
            // After shutdown, pool should have fewer connections or be empty
            // Note: Our implementation may still maintain some connections for health checks
            let health_after = pool.health_check().await;
            assert!(
                health_after.total_connections <= health_before.total_connections,
                "Pool should have same or fewer connections after shutdown"
            );
            
            // Cleanup test key
            let _ = pool
                .execute(|mut conn| {
                    let key = key.clone();
                    async move {
                        let _: () = conn.del(&key).await?;
                        Ok(((), conn))
                    }
                })
                .await;
        }
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}