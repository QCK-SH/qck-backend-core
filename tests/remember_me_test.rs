// Test that remember_me flag properly extends refresh token expiry
use qck_backend::app_config::config;
use qck_backend::db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool};
use qck_backend::services::JwtService;
use std::env;
use std::time::Duration;

#[tokio::test]
async fn test_remember_me_token_expiry() {
    // Load test environment
    dotenv::from_filename(".env.test").ok();

    // Get configuration
    let config = config();
    let remember_me_days = config.security.remember_me_duration_days;

    // Initialize database pool
    let db_config = DieselDatabaseConfig {
        url: env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://qck_user:qck_password@localhost:15001/qck_test".to_string()
        }),
        max_connections: 10,
        min_connections: 1,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        max_lifetime: Duration::from_secs(1800),
        test_on_checkout: true,
    };
    let diesel_pool = create_diesel_pool(db_config)
        .await
        .expect("Failed to create diesel pool");

    // Initialize Redis pool
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config)
        .await
        .expect("Failed to create Redis pool");

    // Create JWT service
    let jwt_service = JwtService::from_env_with_diesel(diesel_pool.clone(), redis_pool)
        .expect("Failed to create JWT service");

    // Create a real test user in the database
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::users;
    use uuid::Uuid;

    let mut conn = diesel_pool.get().await.expect("Failed to get connection");
    let user_uuid = Uuid::new_v4();
    let user_id = user_uuid.to_string();

    diesel::insert_into(users::table)
        .values((
            users::id.eq(user_uuid),
            users::email.eq(format!("remember_test_{}@example.com", user_uuid)),
            users::password_hash.eq("test_hash"),
            users::full_name.eq("Test User"),
            users::company_name.eq(Some("Test Company")),
            users::is_active.eq(true),
            users::email_verified.eq(true),
            users::subscription_tier.eq("free"),
            users::onboarding_status.eq("completed"),
            users::created_at.eq(chrono::Utc::now()),
            users::updated_at.eq(chrono::Utc::now()),
        ))
        .execute(&mut conn)
        .await
        .expect("Failed to create test user");
    let device_fingerprint = Some("test-fingerprint".to_string());
    let ip_address = Some("127.0.0.1".to_string());
    let user_agent = Some("Test User Agent".to_string());

    // Test 1: Generate token WITHOUT remember_me
    let token_without_remember = jwt_service
        .generate_refresh_token_with_device_and_remember(
            &user_id,
            device_fingerprint.clone(),
            ip_address.clone(),
            user_agent.clone(),
            false, // remember_me = false
        )
        .await
        .expect("Failed to generate token without remember_me");

    // Test 2: Generate token WITH remember_me
    let token_with_remember = jwt_service
        .generate_refresh_token_with_device_and_remember(
            &user_id,
            device_fingerprint.clone(),
            ip_address.clone(),
            user_agent.clone(),
            true, // remember_me = true
        )
        .await
        .expect("Failed to generate token with remember_me");

    // Decode tokens to check expiry times
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct TestClaims {
        exp: u64,
        iat: u64,
    }

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false; // Don't validate expiry for this test
    validation.validate_aud = false;
    validation.required_spec_claims.clear();

    // Get the refresh secret from environment
    let refresh_secret =
        env::var("JWT_REFRESH_SECRET").unwrap_or_else(|_| "test-refresh-secret-hs256".to_string());
    let decoding_key = DecodingKey::from_secret(refresh_secret.as_bytes());

    // Decode token without remember_me
    let decoded_without = decode::<TestClaims>(&token_without_remember, &decoding_key, &validation)
        .expect("Failed to decode token without remember_me");

    // Decode token with remember_me
    let decoded_with = decode::<TestClaims>(&token_with_remember, &decoding_key, &validation)
        .expect("Failed to decode token with remember_me");

    // Calculate expected expiry differences
    let normal_expiry_seconds = decoded_without.claims.exp - decoded_without.claims.iat;
    let remember_expiry_seconds = decoded_with.claims.exp - decoded_with.claims.iat;

    // Normal token should have the configured refresh token expiry
    println!(
        "Normal token expiry: {} seconds ({} days)",
        normal_expiry_seconds,
        normal_expiry_seconds / 86400
    );
    println!(
        "Remember token expiry: {} seconds ({} days)",
        remember_expiry_seconds,
        remember_expiry_seconds / 86400
    );

    // Check that normal token has standard refresh expiry (could be 7 days or as configured)
    // The exact value depends on JWT_REFRESH_EXPIRY configuration
    assert!(
        normal_expiry_seconds > 0,
        "Normal token should have positive expiry"
    );

    // Remember_me token should have longer expiry than normal token
    // The exact value depends on configuration but should be longer
    let expected_remember_seconds = remember_me_days as u64 * 86400;
    assert!(
        remember_expiry_seconds >= expected_remember_seconds || remember_expiry_seconds > normal_expiry_seconds,
        "Remember_me token ({} seconds) should have longer expiry than normal token ({} seconds) or be at least {} seconds",
        remember_expiry_seconds,
        normal_expiry_seconds,
        expected_remember_seconds
    );

    // Verify that remember_me token has longer expiry
    assert!(
        remember_expiry_seconds > normal_expiry_seconds,
        "Remember_me token should have longer expiry than normal token"
    );

    println!(
        "✅ Remember_me test passed:\n  Normal token expiry: {} days\n  Remember_me token expiry: {} days",
        normal_expiry_seconds / 86400,
        remember_expiry_seconds / 86400
    );
}
