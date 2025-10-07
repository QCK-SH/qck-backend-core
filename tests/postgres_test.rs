mod common;

use common::{test_table_name, CountRow, TestRow};
use diesel::sql_query;
use qck_backend_core::db::{
    create_diesel_pool, mask_connection_string, DatabaseConfig, DieselDatabaseConfig, DieselPool,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;

// Helper struct for database health check
#[derive(Debug)]
struct DatabaseHealth {
    is_healthy: bool,
    latency_ms: u64,
    active_connections: u32,
    idle_connections: u32,
    max_connections: u32,
    error: Option<String>,
}

// Helper struct for pool metrics
#[derive(Debug)]
struct PoolMetrics {
    active_connections: u32,
    idle_connections: u32,
}

// Helper function to convert DatabaseConfig to DieselDatabaseConfig and create pool
async fn create_test_pool(
    config: DatabaseConfig,
) -> Result<DieselPool, Box<dyn std::error::Error>> {
    let diesel_config = DieselDatabaseConfig {
        url: config.database_url,
        max_connections: config.max_connections,
        min_connections: config.min_connections,
        connection_timeout: config.connect_timeout,
        idle_timeout: config.idle_timeout,
        max_lifetime: config.max_lifetime,
        test_on_checkout: config.test_before_acquire,
    };
    create_diesel_pool(diesel_config).await
}

#[tokio::test]
async fn test_database_config_validation_comprehensive() {
    dotenv::from_filename(".env.test").ok();

    let mut config = DatabaseConfig::from_env();

    // Valid config should pass
    assert!(config.validate().is_ok(), "Valid config should pass");

    // Test all invalid scenarios

    // Invalid: max < min
    let original_max = config.max_connections;
    let original_min = config.min_connections;
    config.max_connections = 1;
    config.min_connections = 5;
    assert!(
        config.validate().is_err(),
        "Should fail when max_connections < min_connections"
    );

    // Reset
    config.max_connections = original_max;
    config.min_connections = original_min;

    // Invalid: max = 0
    config.max_connections = 0;
    config.min_connections = 0;
    assert!(
        config.validate().is_err(),
        "Should fail when max_connections = 0"
    );

    // Reset
    config.max_connections = original_max;
    config.min_connections = original_min;

    // Invalid: empty URL
    let original_url = config.database_url.clone();
    config.database_url = String::new();
    assert!(
        config.validate().is_err(),
        "Should fail with empty database URL"
    );

    // Reset for other tests
    config.database_url = original_url;
}

#[test]
fn test_mask_connection_string_comprehensive() {
    // Test valid PostgreSQL URL with credentials
    let url = "postgresql://user:pass@localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");

    // Test URL with URL-encoded special characters in password
    let url = "postgresql://user:p%40ss%21word@localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");

    // Test URL without credentials
    let url = "postgresql://localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://localhost/mydb");

    // Test URL with only username
    let url = "postgresql://user@localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");

    // Test postgres:// scheme
    let url = "postgres://user:pass@localhost:5432/mydb";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");

    // Test invalid URL
    let invalid = "not a url";
    let masked = mask_connection_string(invalid);
    assert_eq!(masked, "postgresql://***:***@***");

    // Test URL with query parameters
    let url = "postgresql://user:pass@localhost:5432/mydb?sslmode=require&connect_timeout=10";
    let masked = mask_connection_string(url);
    assert_eq!(masked, "postgresql://***:***@localhost/mydb");
}

#[tokio::test]
async fn test_postgres_pool_creation_and_lifecycle() {
    dotenv::from_filename(".env.test").ok();

    let config = DatabaseConfig::from_env();
    // Convert to DieselDatabaseConfig
    let diesel_config = DieselDatabaseConfig {
        url: config.database_url.clone(),
        max_connections: config.max_connections,
        min_connections: config.min_connections,
        connection_timeout: config.connect_timeout,
        idle_timeout: config.idle_timeout,
        max_lifetime: config.max_lifetime,
        test_on_checkout: config.test_before_acquire,
    };
    let pool_result = create_diesel_pool(diesel_config).await;

    assert!(
        pool_result.is_ok(),
        "Failed to create PostgreSQL pool: {:?}",
        pool_result.err()
    );

    if let Ok(pool) = pool_result {
        // Verify pool is functional
        let pool_ref = &pool;

        // Check pool properties
        let state = pool_ref.state();
        assert!(
            state.connections > 0,
            "Pool should have at least one connection"
        );
        assert!(
            state.connections <= config.max_connections,
            "Pool size should not exceed max_connections"
        );
        assert!(
            state.idle_connections <= state.connections,
            "Idle connections should not exceed total pool size"
        );

        // Test simple query execution using Diesel
        let mut conn = pool_ref
            .get()
            .await
            .expect("Should get connection from pool");

        use diesel_async::RunQueryDsl as _;
        let result: Result<i32, diesel::result::Error> = diesel::sql_query("SELECT 1 as test")
            .get_result::<TestRow>(&mut conn)
            .await
            .map(|row| row.test);

        assert!(result.is_ok(), "Should execute simple query");
        assert_eq!(result.unwrap(), 1, "Query should return expected value");
    }
}

// Helper structs moved to common module

#[tokio::test]
async fn test_postgres_health_check_detailed() {
    dotenv::from_filename(".env.test").ok();

    let config = DatabaseConfig::from_env();
    let max_connections = config.max_connections;

    match create_test_pool(config).await {
        Ok(pool) => {
            let pool_ref = &pool;
            // Perform health check directly on pool
            let start = Instant::now();
            let (is_healthy, error) = match pool_ref.get().await {
                Ok(_conn) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            };
            let latency_ms = start.elapsed().as_millis() as u64;
            let state = pool_ref.state();

            let health = DatabaseHealth {
                is_healthy,
                latency_ms,
                active_connections: state.connections,
                idle_connections: state.idle_connections,
                max_connections,
                error,
            };

            // Comprehensive health assertions
            assert!(health.is_healthy, "PostgreSQL health check should pass");

            assert!(
                health.latency_ms < 100,
                "PostgreSQL latency too high: {}ms (should be <100ms)",
                health.latency_ms
            );

            assert_eq!(
                health.max_connections, max_connections,
                "Max connections should match config"
            );

            assert!(
                health.active_connections <= health.max_connections,
                "Active connections ({}) should not exceed max ({})",
                health.active_connections,
                health.max_connections
            );

            assert!(
                health.idle_connections <= health.max_connections,
                "Idle connections ({}) should not exceed max ({})",
                health.idle_connections,
                health.max_connections
            );

            assert!(
                health.active_connections + health.idle_connections <= health.max_connections,
                "Total connections should not exceed max"
            );

            assert!(
                health.error.is_none(),
                "Health check should not have errors: {:?}",
                health.error
            );
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        },
    }
}

#[tokio::test]
async fn test_postgres_concurrent_operations() {
    dotenv::from_filename(".env.test").ok();

    let config = DatabaseConfig::from_env();

    match create_test_pool(config).await {
        Ok(pool) => {
            let pool_ref = Arc::new(pool.clone());
            let table_name = test_table_name("concurrent");

            // Create test table
            // NOTE: Dynamic table creation for test purposes only
            // Cannot use Diesel's schema here as tables are created at runtime
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");
                let create_table = format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                        id SERIAL PRIMARY KEY,
                        task_id INT NOT NULL,
                        value TEXT NOT NULL,
                        created_at TIMESTAMPTZ DEFAULT NOW()
                    )",
                    table_name
                );

                use diesel_async::RunQueryDsl as _;
                diesel::sql_query(create_table)
                    .execute(&mut conn)
                    .await
                    .expect("Should create test table");
            }

            // Run concurrent operations
            let num_tasks = 20;
            let barrier = Arc::new(Barrier::new(num_tasks));
            let mut handles = Vec::new();

            for i in 0..num_tasks {
                let pool = pool_ref.clone();
                let barrier = barrier.clone();
                let table = table_name.clone();

                let handle = tokio::spawn(async move {
                    // Synchronize all tasks to start together
                    barrier.wait().await;

                    // Get connection for this task
                    let mut conn = pool.get().await.expect("Should get connection");

                    // Insert data using parameterized query with raw SQL
                    //
                    // NOTE: We use a static test table name to maintain type safety and avoid dynamic SQL identifiers.
                    // While the table name is still dynamically generated for test isolation,
                    // we validate it strictly to ensure it only contains safe characters.
                    // This pattern is ONLY used in tests, never in production code.
                    // Production code always uses Diesel's type-safe table definitions.

                    // Validate table name contains only safe characters (alphanumeric and underscore)
                    assert!(
                        table.chars().all(|c| c.is_alphanumeric() || c == '_'),
                        "Table name contains unsafe characters"
                    );

                    let insert_query =
                        format!("INSERT INTO {} (task_id, value) VALUES ($1, $2)", table);
                    let value = format!("value_{}", i);

                    use diesel_async::RunQueryDsl as _;
                    let result = diesel::sql_query(insert_query)
                        .bind::<diesel::sql_types::Integer, _>(i as i32)
                        .bind::<diesel::sql_types::Text, _>(value)
                        .execute(&mut conn)
                        .await;

                    assert!(
                        result.is_ok(),
                        "Task {} failed to insert: {:?}",
                        i,
                        result.err()
                    );
                });

                handles.push(handle);
            }

            // Wait for all tasks
            for handle in handles {
                handle.await.expect("Task should complete");
            }

            // Verify all inserts succeeded
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");
                let count_query = format!("SELECT COUNT(*) as count FROM {}", table_name);
                use diesel_async::RunQueryDsl as _;
                let row: CountRow = diesel::sql_query(count_query)
                    .get_result(&mut conn)
                    .await
                    .expect("Should count rows");

                assert_eq!(
                    row.count, num_tasks as i64,
                    "Should have inserted all {} rows",
                    num_tasks
                );

                // Cleanup
                let drop_table = format!("DROP TABLE IF EXISTS {}", table_name);
                use diesel_async::RunQueryDsl as _;
                let _ = diesel::sql_query(drop_table).execute(&mut conn).await;
            }
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        },
    }
}

#[tokio::test]
async fn test_postgres_transaction_rollback() {
    dotenv::from_filename(".env.test").ok();

    let config = DatabaseConfig::from_env();

    match create_test_pool(config).await {
        Ok(pool) => {
            let pool_ref = &pool;
            let table_name = test_table_name("rollback");

            // Create test table
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");
                let create_table = format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                        id SERIAL PRIMARY KEY,
                        value TEXT NOT NULL
                    )",
                    table_name
                );

                use diesel_async::RunQueryDsl as _;
                diesel::sql_query(create_table)
                    .execute(&mut conn)
                    .await
                    .expect("Should create test table");
            }

            // Test transaction rollback
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");

                // Start transaction and insert data
                use diesel_async::AsyncConnection;
                let table_name_clone = table_name.clone();
                let result: Result<(), diesel::result::Error> = conn
                    .transaction::<_, diesel::result::Error, _>(|conn| {
                        Box::pin(async move {
                            use diesel_async::RunQueryDsl as _;
                            let insert_query = format!(
                                "INSERT INTO {} (value) VALUES ('test_value')",
                                table_name_clone
                            );
                            diesel::sql_query(insert_query).execute(conn).await?;

                            // Force rollback by returning an error
                            Err(diesel::result::Error::RollbackTransaction)
                        })
                    })
                    .await;

                assert!(result.is_err(), "Transaction should have been rolled back");

                // Verify data was not persisted
                let count_query = format!("SELECT COUNT(*) as count FROM {}", table_name);
                use diesel_async::RunQueryDsl as _;
                let row: CountRow = diesel::sql_query(count_query)
                    .get_result(&mut conn)
                    .await
                    .expect("Should count rows");

                assert_eq!(row.count, 0, "Table should be empty after rollback");
            }

            // Test successful commit
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");

                use diesel_async::AsyncConnection;
                let table_name_clone = table_name.clone();
                let result: Result<(), diesel::result::Error> = conn
                    .transaction::<_, diesel::result::Error, _>(|conn| {
                        Box::pin(async move {
                            use diesel_async::RunQueryDsl as _;
                            let insert_query = format!(
                                "INSERT INTO {} (value) VALUES ('committed_value')",
                                table_name_clone
                            );
                            diesel::sql_query(insert_query).execute(conn).await?;
                            Ok(())
                        })
                    })
                    .await;

                assert!(result.is_ok(), "Transaction should have been committed");

                // Verify data was persisted
                let count_query = format!("SELECT COUNT(*) as count FROM {}", table_name);
                use diesel_async::RunQueryDsl as _;
                let row: CountRow = diesel::sql_query(count_query)
                    .get_result(&mut conn)
                    .await
                    .expect("Should count rows");

                assert_eq!(row.count, 1, "Table should have one row after commit");

                // Cleanup
                let drop_table = format!("DROP TABLE IF EXISTS {}", table_name);
                use diesel_async::RunQueryDsl as _;
                let _ = diesel::sql_query(drop_table).execute(&mut conn).await;
            }
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        },
    }
}

#[tokio::test]
async fn test_postgres_pool_metrics_accuracy() {
    dotenv::from_filename(".env.test").ok();

    let config = DatabaseConfig::from_env();

    match create_test_pool(config.clone()).await {
        Ok(pool) => {
            let state = pool.state();
            let initial_metrics = PoolMetrics {
                active_connections: state.connections,
                idle_connections: state.idle_connections,
            };

            // Meaningful assertions for metrics
            assert!(
                initial_metrics.active_connections <= config.max_connections,
                "Active connections ({}) should not exceed max ({})",
                initial_metrics.active_connections,
                config.max_connections
            );

            assert!(
                initial_metrics.idle_connections <= config.max_connections,
                "Idle connections ({}) should not exceed max ({})",
                initial_metrics.idle_connections,
                config.max_connections
            );

            assert!(
                initial_metrics.active_connections + initial_metrics.idle_connections
                    <= config.max_connections,
                "Total connections should not exceed max_connections"
            );

            // Perform some operations to change metrics
            let pool_ref = &pool;
            for i in 0..5 {
                let mut conn = pool_ref.get().await.expect("Should get connection");
                let query = format!("SELECT {} as num", i);
                use diesel_async::RunQueryDsl as _;
                let _: Result<TestRow, _> = diesel::sql_query(query).get_result(&mut conn).await;
            }

            let state = pool.state();
            let final_metrics = PoolMetrics {
                active_connections: state.connections,
                idle_connections: state.idle_connections,
            };

            // Verify metrics are within bounds
            assert!(
                final_metrics.active_connections <= config.max_connections,
                "Active connections should remain within bounds"
            );

            assert!(
                final_metrics.idle_connections <= config.max_connections,
                "Idle connections should remain within bounds"
            );
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        },
    }
}

#[tokio::test]
async fn test_postgres_connection_retry_with_invalid_url() {
    dotenv::from_filename(".env.test").ok();

    let mut config = DatabaseConfig::from_env();

    // Use invalid host to test retry logic
    config.database_url = "postgresql://user:pass@nonexistent-host:5432/db".to_string();
    config.connect_timeout = Duration::from_millis(100); // Very short timeout for faster test

    let start = Instant::now();
    let pool_result = create_test_pool(config).await;
    let elapsed = start.elapsed();

    // Should fail after retries
    assert!(
        pool_result.is_err(),
        "Should fail with invalid connection URL"
    );

    // With connection timeout and retry logic, expect some delay
    assert!(
        elapsed >= Duration::from_millis(100), // At least connection timeout duration
        "Should have attempted connection, took {:?}",
        elapsed
    );

    assert!(
        elapsed < Duration::from_secs(15), // Reasonable timeout expectation
        "Should not take too long to fail, took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_postgres_production_query_performance() {
    dotenv::from_filename(".env.test").ok();

    let config = DatabaseConfig::from_env();

    match create_test_pool(config).await {
        Ok(pool) => {
            let pool_ref = &pool;
            let table_name = test_table_name("performance");

            // Create test table with index
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");

                let create_table = format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                        id SERIAL PRIMARY KEY,
                        key VARCHAR(50) NOT NULL,
                        value TEXT NOT NULL,
                        created_at TIMESTAMPTZ DEFAULT NOW()
                    )",
                    table_name
                );

                use diesel_async::RunQueryDsl as _;
                diesel::sql_query(create_table)
                    .execute(&mut conn)
                    .await
                    .expect("Should create test table");

                // Create index for performance
                let create_index = format!(
                    "CREATE INDEX idx_{}_key ON {} (key)",
                    table_name, table_name
                );

                use diesel_async::RunQueryDsl as _;
                diesel::sql_query(create_index)
                    .execute(&mut conn)
                    .await
                    .expect("Should create index");

                // Insert test data
                for i in 0..100 {
                    let insert_query = format!(
                        "INSERT INTO {} (key, value) VALUES ('key_{}', 'value_{}')",
                        table_name, i, i
                    );
                    use diesel_async::RunQueryDsl as _;
                    diesel::sql_query(insert_query)
                        .execute(&mut conn)
                        .await
                        .expect("Should insert test data");
                }
            }

            // Measure query performance
            let mut latencies = Vec::with_capacity(1000);

            for i in 0..1000 {
                let key = format!("key_{}", i % 100);
                let start = Instant::now();

                {
                    let mut conn = pool_ref.get().await.expect("Should get connection");
                    let select_query =
                        format!("SELECT * FROM {} WHERE key = '{}'", table_name, key);
                    use diesel_async::RunQueryDsl as _;
                    let _: Result<Vec<TestRow>, _> =
                        diesel::sql_query(select_query).load(&mut conn).await;
                }

                let latency = start.elapsed();
                latencies.push(latency);
            }

            // Calculate percentiles
            latencies.sort();
            let p50 = latencies[500];
            let p95 = latencies[950];
            let p99 = latencies[990];

            println!("PostgreSQL Query Performance:");
            println!("  P50: {:?}", p50);
            println!("  P95: {:?}", p95);
            println!("  P99: {:?}", p99);

            // Production requirements (relaxed for CI environments)
            let (p50_limit, p95_limit, p99_limit) = if std::env::var("CI").is_ok() {
                // Relaxed limits for CI environments
                (
                    Duration::from_millis(50),
                    Duration::from_millis(200),
                    Duration::from_millis(500),
                )
            } else {
                // Production limits
                (
                    Duration::from_millis(10),
                    Duration::from_millis(50),
                    Duration::from_millis(100),
                )
            };

            assert!(
                p50 < p50_limit,
                "P50 query latency too high: {:?} (need <{:?})",
                p50,
                p50_limit
            );

            assert!(
                p95 < p95_limit,
                "P95 query latency too high: {:?} (need <{:?})",
                p95,
                p95_limit
            );

            assert!(
                p99 < p99_limit,
                "P99 query latency too high: {:?} (need <{:?})",
                p99,
                p99_limit
            );

            // Cleanup
            {
                let mut conn = pool_ref.get().await.expect("Should get connection");
                let drop_table = format!("DROP TABLE IF EXISTS {}", table_name);
                use diesel_async::RunQueryDsl as _;
                let _ = diesel::sql_query(drop_table).execute(&mut conn).await;
            }
        },
        Err(e) => {
            panic!("Failed to create PostgreSQL pool: {}", e);
        },
    }
}
