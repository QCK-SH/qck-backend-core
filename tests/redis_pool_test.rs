use qck_backend::db::{RedisConfig, RedisPool};

#[tokio::test]
async fn test_redis_pool_creation() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    let pool = RedisPool::new(config).await;
    
    assert!(pool.is_ok(), "Failed to create Redis pool: {:?}", pool.err());
}

#[tokio::test]
async fn test_redis_health_check() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    
    match RedisPool::new(config).await {
        Ok(pool) => {
            let health = pool.health_check().await;
            
            assert!(health.is_healthy, "Redis health check failed");
            assert!(health.latency_ms < 100, "Redis latency too high: {}ms", health.latency_ms);
            assert!(health.total_connections > 0, "No Redis connections available");
            assert!(health.error.is_none(), "Health check returned error: {:?}", health.error);
        },
        Err(e) => {
            panic!("Failed to create Redis pool for health check: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_connection_management() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Test getting and returning connections
            let conn1 = pool.get_connection().await;
            assert!(conn1.is_ok(), "Failed to get connection: {:?}", conn1.err());
            
            if let Ok(conn) = conn1 {
                pool.return_connection(conn).await;
            }
            
            // Verify connection was returned to pool
            let health = pool.health_check().await;
            assert!(health.total_connections > 0);
        },
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_execute_command() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Test executing a simple command
            let result = pool.execute(|mut conn| async move {
                use redis::AsyncCommands;
                
                let key = "test:execute:key";
                let value = "test_value";
                
                // Set a value
                let _: () = conn.set(key, value).await?;
                
                // Get the value back
                let retrieved: String = conn.get(key).await?;
                
                // Clean up
                let _: () = conn.del(key).await?;
                
                Ok((retrieved, conn))
            }).await;
            
            assert!(result.is_ok(), "Failed to execute command: {:?}", result.err());
            assert_eq!(result.unwrap(), "test_value");
        },
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_high_throughput() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Test with 100 operations (smaller for unit tests)
            let operations = 100;
            let result = pool.test_high_throughput(operations).await;
            
            assert!(result.is_ok(), "High throughput test failed: {:?}", result.err());
            
            if let Ok(duration) = result {
                let ops_per_second = operations as f64 / duration.as_secs_f64();
                println!("Achieved {} ops/second", ops_per_second);
                
                // Should handle at least 100 ops/second even in test environment
                assert!(ops_per_second > 100.0, "Performance too low: {} ops/s", ops_per_second);
            }
        },
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_pool_metrics() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    
    match RedisPool::new(config).await {
        Ok(pool) => {
            let metrics = pool.get_metrics().await;
            
            assert!(metrics.connections_created > 0, "No connections created")
        },
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_redis_pool_shutdown() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = RedisConfig::from_env();
    
    match RedisPool::new(config).await {
        Ok(pool) => {
            // Verify pool is working before shutdown
            let health_before = pool.health_check().await;
            assert!(health_before.is_healthy);
            
            // Shutdown the pool
            pool.shutdown().await;
            
            // After shutdown, getting a connection should create a new one
            // (since the pool is empty but the client still exists)
            let conn_result = pool.get_connection().await;
            assert!(conn_result.is_ok(), "Should still be able to create new connections after shutdown");
        },
        Err(e) => {
            panic!("Failed to create Redis pool: {}", e);
        }
    }
}