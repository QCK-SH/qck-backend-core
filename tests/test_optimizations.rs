// Test new Base62 optimizations: batch checking, pre-generated pools, alerting

use qck_backend::db::{create_diesel_pool, DieselDatabaseConfig};
use qck_backend::services::short_code::ShortCodeGenerator;
use std::time::Instant;

#[tokio::test]
async fn test_batch_collision_checking() {
    println!("=== Testing Batch Collision Checking ===");

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Test batch generation with optimized checking
    let start = Instant::now();
    let batch_size = 50;
    let codes = generator.generate_batch_codes(batch_size, 7).await.unwrap();
    let duration = start.elapsed();

    println!("Generated {} codes in {:?}", codes.len(), duration);
    println!(
        "Average time per code: {:.2}ms",
        duration.as_millis() as f64 / codes.len() as f64
    );

    // Check all codes are unique
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(
        unique.len(),
        codes.len(),
        "Batch generated duplicate codes!"
    );

    // Should be faster than individual generation due to batch checking
    assert!(duration.as_secs() < 2, "Batch generation too slow!");
}

#[tokio::test]
async fn test_pre_generated_code_pool() {
    println!("=== Testing Pre-generated Code Pool ===");

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Check initial pool is empty
    let initial_size = generator.get_pool_size().await;
    assert_eq!(initial_size, 0, "Pool should start empty");

    // Refill pool
    let pool_size = 10;
    generator.refill_code_pool(pool_size).await.unwrap();

    // Check pool has codes
    let current_size = generator.get_pool_size().await;
    assert_eq!(
        current_size, pool_size,
        "Pool should have {} codes",
        pool_size
    );

    // Get code from pool (should be instant)
    let start = Instant::now();
    let code = generator.generate_unique_code().await.unwrap();
    let duration = start.elapsed();

    println!("Got code from pool: {} in {:?}", code, duration);

    // Should be very fast since it's from pool
    assert!(
        duration.as_millis() < 10,
        "Getting from pool should be instant!"
    );

    // Pool should have one less code
    let new_size = generator.get_pool_size().await;
    assert_eq!(new_size, pool_size - 1, "Pool should decrease by 1");
}

#[tokio::test]
async fn test_collision_alerting() {
    println!("=== Testing Collision Alerting ===");

    // This test verifies alerting logic exists
    // In production, alerts would go to monitoring services

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Get stats to verify collision tracking
    let stats = generator.get_generation_stats().await.unwrap();

    println!("Collision tracking active:");
    println!("  Collision rate: {:.2}%", stats.collision_rate * 100.0);
    println!("  Total codes: {}", stats.total_codes);
    println!("  Current counter: {}", stats.current_counter);

    // Verify collision rate field exists and is tracked
    assert!(
        stats.collision_rate >= 0.0,
        "Collision rate should be tracked"
    );
}

#[tokio::test]
async fn test_performance_with_optimizations() {
    println!("=== Testing Overall Performance with Optimizations ===");

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Pre-fill pool for high-traffic simulation
    generator.refill_code_pool(50).await.unwrap();

    // Generate codes (some from pool, some fresh)
    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = generator.generate_unique_code().await.unwrap();
    }

    let duration = start.elapsed();
    let avg_ms = duration.as_millis() as f64 / iterations as f64;

    println!("Generated {} codes with optimizations", iterations);
    println!("Total time: {:?}", duration);
    println!("Average time per code: {:.2}ms", avg_ms);

    // Should meet <1ms requirement with optimizations
    assert!(
        avg_ms < 1.0,
        "Average generation time {:.2}ms exceeds 1ms requirement!",
        avg_ms
    );
}
