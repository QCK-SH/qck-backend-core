use qck_backend::db::{DatabaseConfig, PostgresPool, postgres::mask_connection_string};

#[tokio::test]
async fn test_database_config_validation() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let mut config = DatabaseConfig::from_env();
    
    // Valid config should pass
    assert!(config.validate().is_ok());
    
    // Invalid: max < min
    config.max_connections = 1;
    config.min_connections = 5;
    assert!(config.validate().is_err(), "Should fail when max < min");
    
    // Reset
    config.max_connections = 20;
    config.min_connections = 5;
    
    // Invalid: max = 0
    config.max_connections = 0;
    config.min_connections = 0;
    assert!(config.validate().is_err(), "Should fail when max = 0");
    
    // Reset
    config.max_connections = 20;
    
    // Invalid: empty URL
    config.database_url = String::new();
    assert!(config.validate().is_err(), "Should fail with empty URL");
}

#[test]
fn test_mask_connection_string() {
    // Test valid PostgreSQL URL
    let url = "postgresql://user:pass@localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");
    
    // Test URL with special characters in password
    let url = "postgresql://user:p@ss!word@localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");
    
    // Test URL without credentials
    let url = "postgresql://localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://localhost/mydb");
    
    // Test invalid URL
    let invalid = "not a url";
    let masked = mask_connection_string(invalid);
    assert_eq!(masked, "postgresql://***:***@***");
}

#[tokio::test]
async fn test_postgres_pool_creation() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = DatabaseConfig::from_env();
    let pool_result = PostgresPool::new(config).await;
    
    assert!(pool_result.is_ok(), "Failed to create PostgreSQL pool: {:?}", pool_result.err());
    
    if let Ok(pool) = pool_result {
        // Verify pool is functional
        let pool_ref = pool.get_pool();
        assert!(pool_ref.size() > 0, "Pool should have connections");
        assert!(pool_ref.num_idle() > 0, "Pool should have idle connections");
    }
}

#[tokio::test]
async fn test_postgres_health_check() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = DatabaseConfig::from_env();
    let max_connections = config.max_connections;
    
    match PostgresPool::new(config).await {
        Ok(pool) => {
            let pool_ref = pool.get_pool();
            let health = PostgresPool::health_check_with_pool(&pool_ref, max_connections).await;
            
            assert!(health.is_healthy, "PostgreSQL health check failed");
            assert!(health.latency_ms < 100, "PostgreSQL latency too high: {}ms", health.latency_ms);
            assert_eq!(health.max_connections, max_connections);
            assert!(health.error.is_none(), "Health check returned error: {:?}", health.error);
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_postgres_pool_metrics() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = DatabaseConfig::from_env();
    
    match PostgresPool::new(config).await {
        Ok(pool) => {
            let metrics = pool.get_metrics();
            
            assert!(metrics.active_connections >= 0, "Active connections should be non-negative");
            assert!(metrics.idle_connections >= 0, "Idle connections should be non-negative");
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_postgres_connection_test() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let config = DatabaseConfig::from_env();
    
    match PostgresPool::new(config).await {
        Ok(pool) => {
            // Test that we can execute a simple query
            let pool_ref = pool.get_pool();
            let result = sqlx::query("SELECT 1 as test")
                .fetch_one(pool_ref)
                .await;
            
            assert!(result.is_ok(), "Failed to execute test query: {:?}", result.err());
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        }
    }
}

#[tokio::test]
async fn test_postgres_connection_with_invalid_url() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();
    
    let mut config = DatabaseConfig::from_env();
    
    // Test with invalid URL to verify error handling
    config.database_url = "postgresql://invalid:invalid@nonexistent:5432/db".to_string();
    config.connect_timeout = std::time::Duration::from_millis(100); // Shorter timeout for tests
    
    let start = std::time::Instant::now();
    let pool_result = PostgresPool::new(config).await;
    let elapsed = start.elapsed();
    
    // Should fail with invalid connection
    assert!(pool_result.is_err(), "Should fail with invalid connection");
    
    // Should have attempted connection
    assert!(elapsed >= std::time::Duration::from_millis(100), "Should have attempted connection");
}