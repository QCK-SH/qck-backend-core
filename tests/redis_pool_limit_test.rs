// Test to verify Redis pool connection limiting
use qck_backend_core::db::{RedisConfig, RedisPool};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_redis_pool_connection_limit() {
    // Load environment
    dotenv::from_filename("../.env.dev").ok();

    // Create a small pool for testing
    let mut config = RedisConfig::from_env();
    config.pool_size = 5; // Small pool size

    let pool = RedisPool::new(config.clone()).await.unwrap();

    // Hold connections to exhaust the pool
    let held_connections = Arc::new(Mutex::new(Vec::new()));

    // Get connections up to 2x the pool size (our hard limit)
    let max_allowed = config.pool_size * 2;

    // Acquire max_allowed connections
    for i in 0..max_allowed {
        match pool.get_connection().await {
            Ok(conn) => {
                println!("Got connection {}/{}", i + 1, max_allowed);
                held_connections.lock().await.push(conn);
            },
            Err(e) => {
                panic!(
                    "Should be able to get {} connections, failed at {}: {}",
                    max_allowed,
                    i + 1,
                    e
                );
            },
        }
    }

    // Now try to get one more - this should fail
    match pool.get_connection().await {
        Ok(_) => {
            panic!(
                "Should not be able to get more than {} connections",
                max_allowed
            );
        },
        Err(e) => {
            println!("Correctly rejected connection beyond limit: {}", e);
            assert!(e.to_string().contains("exhausted") || e.to_string().contains("limit"));
        },
    }

    // Return one connection
    {
        let mut conns = held_connections.lock().await;
        if let Some(conn) = conns.pop() {
            pool.return_connection(conn).await;
        }
    }

    // Now we should be able to get another connection
    match pool.get_connection().await {
        Ok(_) => {
            println!("Successfully got connection after returning one");
        },
        Err(e) => {
            panic!(
                "Should be able to get connection after returning one: {}",
                e
            );
        },
    }
}

#[tokio::test]
async fn test_redis_pool_metrics_accuracy() {
    // Load environment
    dotenv::from_filename("../.env.dev").ok();

    let mut config = RedisConfig::from_env();
    config.pool_size = 10;

    let pool = RedisPool::new(config).await.unwrap();

    // Get initial metrics
    let initial_metrics = pool.get_metrics().await;
    assert_eq!(initial_metrics.connections_active, 0);
    assert_eq!(initial_metrics.connections_idle, 10);

    // Get 5 connections
    let mut connections = Vec::new();
    for _ in 0..5 {
        connections.push(pool.get_connection().await.unwrap());
    }

    // Check metrics
    let metrics = pool.get_metrics().await;
    assert_eq!(metrics.connections_active, 5);
    assert_eq!(metrics.connections_idle, 5);

    // Return all connections
    for conn in connections {
        pool.return_connection(conn).await;
    }

    // Check final metrics
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let final_metrics = pool.get_metrics().await;
    assert_eq!(final_metrics.connections_active, 0);
    assert_eq!(final_metrics.connections_idle, 10);
}
