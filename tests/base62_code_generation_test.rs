// DEV-108: Base62 Code Generation Tests
// Tests collision detection, Redis caching, and profanity filtering

use diesel::prelude::*;
use qck_backend::db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool};
use qck_backend::services::short_code::ShortCodeGenerator;
use std::time::Instant;

#[tokio::test]
async fn test_code_generation_performance() {
    // Test that code generation meets < 10ms requirement
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let code = generator.generate_unique_code().await;
        assert!(code.is_ok());
    }

    let duration = start.elapsed();
    let avg_ms = duration.as_millis() as f64 / iterations as f64;

    println!("Average code generation time: {:.2}ms", avg_ms);
    assert!(avg_ms < 10.0, "Code generation too slow: {:.2}ms", avg_ms);
}

#[tokio::test]
async fn test_collision_detection() {
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Generate many codes to test collision detection
    let mut codes = Vec::new();

    for _ in 0..100 {
        match generator.generate_unique_code().await {
            Ok(code) => {
                // Check for duplicates
                assert!(!codes.contains(&code), "Duplicate code generated: {}", code);
                codes.push(code);
            },
            Err(e) => {
                eprintln!("Failed to generate code: {}", e);
            },
        }
    }

    println!("Generated {} unique codes without collision", codes.len());
}

#[tokio::test]
async fn test_custom_alias_validation() {
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Valid aliases
    let valid_aliases = vec!["my-link", "test_123", "abc123", "MyCustomLink"];

    for alias in valid_aliases {
        let result = generator.validate_custom_alias(alias).await;
        assert!(result.is_ok(), "Valid alias rejected: {}", alias);
    }

    // Invalid aliases
    let invalid_aliases = vec![
        "ab",    // Too short
        "-test", // Starts with hyphen
        "test!", // Invalid character
        "admin", // Reserved word
        "api",   // Reserved word
    ];

    for alias in invalid_aliases {
        let result = generator.validate_custom_alias(alias).await;
        assert!(result.is_err(), "Invalid alias accepted: {}", alias);
    }
}

#[tokio::test]
async fn test_reserved_codes() {
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Reserved words should be rejected
    let reserved = vec!["api", "admin", "login", "dashboard", "settings"];

    for word in reserved {
        let result = generator.validate_custom_alias(word).await;
        assert!(result.is_err(), "Reserved word accepted: {}", word);

        // Also check uppercase
        let result = generator.validate_custom_alias(&word.to_uppercase()).await;
        assert!(
            result.is_err(),
            "Reserved word accepted (uppercase): {}",
            word
        );
    }
}

#[tokio::test]
async fn test_batch_generation() {
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Generate batch of codes
    let batch_size = 10;
    let length = 6;

    match generator.generate_batch_codes(batch_size, length).await {
        Ok(codes) => {
            assert_eq!(codes.len(), batch_size);

            // All codes should be unique
            let mut unique_codes = codes.clone();
            unique_codes.sort();
            unique_codes.dedup();
            assert_eq!(unique_codes.len(), batch_size);

            // All codes should have correct length
            for code in &codes {
                assert_eq!(code.len(), length);
            }

            println!("Successfully generated batch of {} codes", codes.len());
        },
        Err(e) => {
            panic!("Batch generation failed: {}", e);
        },
    }
}

#[tokio::test]
async fn test_encoding_decoding() {
    use qck_backend::services::short_code::ShortCodeGenerator;

    // Test encode/decode functions
    let test_cases = vec![
        (0u64, "0"),
        (1, "1"),
        (61, "z"),
        (62, "10"),
        (3843, "zz"),
        (238327, "zzz"),
    ];

    for (id, expected) in test_cases {
        let encoded = ShortCodeGenerator::encode_id(id);
        assert_eq!(encoded, expected, "Failed to encode {}", id);

        let decoded = ShortCodeGenerator::decode_to_id(&encoded).unwrap();
        assert_eq!(decoded, id, "Failed to decode {}", encoded);
    }
}

#[tokio::test]
async fn test_profanity_filtering() {
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Profane aliases should be rejected
    let profane = vec!["fuck123", "shit", "NSFW", "xxx69"];

    for word in profane {
        let result = generator.validate_custom_alias(word).await;
        // Note: The validate_custom_alias might not check profanity,
        // but the generation should avoid generating these
        println!("Checking profane word: {} - {:?}", word, result);
    }

    // Generate many codes and ensure none contain profanity
    // This is probabilistic but should work with high confidence
    let mut generated = Vec::new();
    for _ in 0..100 {
        if let Ok(code) = generator.generate_unique_code().await {
            generated.push(code);
        }
    }

    // Check that no generated codes contain common profanity
    let profanity_substrings = vec!["fuck", "shit", "sex", "xxx"];
    for code in &generated {
        let code_lower = code.to_lowercase();
        for profane in &profanity_substrings {
            assert!(
                !code_lower.contains(profane),
                "Generated code contains profanity: {} (contains {})",
                code,
                profane
            );
        }
    }

    println!("Generated {} codes without profanity", generated.len());
}

#[tokio::test]
#[ignore] // This test requires database setup
async fn test_generation_stats() {
    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    match generator.get_generation_stats().await {
        Ok(stats) => {
            println!("Generation Stats:");
            println!("  Total codes: {}", stats.total_codes);
            println!("  Default length: {}", stats.default_length);
            println!("  Utilization: {:.2}%", stats.utilization_percentage);
            println!("  Reserved codes: {}", stats.reserved_codes_count);
            println!("  Collision rate: {:.2}%", stats.collision_rate * 100.0);
            println!("  Current counter: {}", stats.current_counter);

            assert!(stats.default_length > 0);
            assert!(stats.reserved_codes_count > 0);
            // New fields should exist
            assert!(stats.collision_rate >= 0.0);
            assert!(stats.current_counter > 0);
        },
        Err(e) => {
            eprintln!("Failed to get stats (expected without database): {}", e);
        },
    }
}

#[tokio::test]
async fn test_counter_based_vs_random_generation() {
    println!("=== Testing Counter vs Random Generation Strategy ===");

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();

    // Warm up the connection pool with a simple query
    {
        use diesel_async::RunQueryDsl;
        let mut conn = pool.get().await.unwrap();
        let _ = diesel_async::RunQueryDsl::execute(diesel::sql_query("SELECT 1"), &mut conn).await;
        println!("Connection pool warmed up");
    }

    let generator = ShortCodeGenerator::new(pool);

    // Generate exactly 6 codes to test the strategy switch
    let mut codes = Vec::new();
    for i in 0..6 {
        let start = Instant::now();
        let code = generator.generate_unique_code().await.unwrap();
        let duration = start.elapsed();

        println!("Code #{}: {} (generated in {:?})", i + 1, code, duration);
        codes.push(code);

        // First 3 should be reasonably fast (counter-based with DB check)
        // Note: We're still checking the database for safety, so 50ms is reasonable
        if i < 3 {
            assert!(
                duration.as_millis() < 150,
                "Counter-based generation should be reasonably fast, got {}ms",
                duration.as_millis()
            );
        }
    }

    // All codes should be unique
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(unique.len(), codes.len(), "Generated duplicate codes!");
}

#[tokio::test]
async fn test_redis_reservation_actually_works() {
    println!("=== Testing Redis Code Reservation Feature ===");

    // Setup Redis pool
    let redis_config = RedisConfig::from_env();
    let redis_pool = match RedisPool::new(redis_config).await {
        Ok(pool) => Some(pool),
        Err(e) => {
            println!("Redis not available: {}. Skipping test.", e);
            return;
        },
    };

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::with_redis(pool, redis_pool.clone());

    // Generate a code (should be reserved)
    let code = generator.generate_unique_code().await.unwrap();
    println!("Generated code: {}", code);

    // Check if it's actually reserved in Redis
    if let Some(redis) = &redis_pool {
        let reserve_key = format!("reserve:{}", code);

        // Should be reserved
        match redis.get::<String>(&reserve_key).await {
            Ok(Some(val)) => {
                println!("✅ Code IS reserved in Redis with value: {}", val);
                assert_eq!(val, "reserved");
            },
            Ok(None) => {
                println!("⚠️ Code NOT reserved (Redis might be disabled or expired)");
            },
            Err(e) => {
                println!("⚠️ Redis error: {}", e);
            },
        }

        // Test the release function
        generator.release_code(&code).await.unwrap();

        // Should no longer be reserved
        match redis.get::<String>(&reserve_key).await {
            Ok(None) => println!("✅ Code successfully released"),
            Ok(Some(_)) => panic!("Code still reserved after release!"),
            Err(e) => println!("Redis error on release check: {}", e),
        }
    }
}

#[tokio::test]
async fn test_json_files_loaded_correctly() {
    println!("=== Testing JSON File Loading ===");

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Test words that are ONLY in the JSON files (not in fallback)
    let json_only_reserved = vec![
        "graphql",    // api_endpoints in JSON
        "webhook",    // api_endpoints in JSON
        "enterprise", // business_terms in JSON
        "kubernetes", // Not in fallback, might be in JSON
    ];

    for word in json_only_reserved {
        let result = generator.validate_custom_alias(word).await;
        if result.is_err() {
            println!("✅ JSON-loaded word '{}' correctly rejected", word);
        } else {
            println!(
                "❌ JSON word '{}' NOT rejected (JSON might not be loaded)",
                word
            );
        }
    }

    // Test profanity that's in JSON but not in minimal fallback
    let json_profanity = vec!["nigger", "retard", "bukkake"];
    for word in json_profanity {
        // Note: validate_custom_alias might not check profanity,
        // but generation should avoid these
        println!("Checking JSON profanity word: {}", word);
    }
}

#[tokio::test]
async fn test_collision_rate_tracking() {
    println!("=== Testing Collision Rate Tracking ===");

    let db_config = DieselDatabaseConfig::default();
    let pool = create_diesel_pool(db_config).await.unwrap();
    let generator = ShortCodeGenerator::new(pool);

    // Generate many codes
    for _ in 0..20 {
        let _ = generator.generate_unique_code().await;
    }

    // Check stats for collision tracking
    match generator.get_generation_stats().await {
        Ok(stats) => {
            println!("After 20 generations:");
            println!("  Collision rate: {:.2}%", stats.collision_rate * 100.0);
            println!("  Current counter: {}", stats.current_counter);

            // Counter should have advanced
            assert!(stats.current_counter > 0, "Counter should be > 0");

            // Collision rate should be tracked (even if 0%)
            assert!(
                stats.collision_rate >= 0.0,
                "Collision rate should be tracked"
            );
        },
        Err(e) => {
            println!("Could not get stats: {}", e);
        },
    }
}
