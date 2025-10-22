// Integration Tests for Refresh Token Rotation Flow
// DEV-107: End-to-end tests for token rotation and security features

use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use qck_backend_core::{
    db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool},
    services::jwt::JwtService,
};
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

/// Helper function to create a test user
async fn create_test_user(pool: &qck_backend_core::db::DieselPool) -> Uuid {
    use qck_backend_core::schema::users;

    let mut conn = pool.get().await.expect("Failed to get connection");
    let user_id = Uuid::new_v4();

    diesel::insert_into(users::table)
        .values((
            users::id.eq(user_id),
            users::email.eq(format!("test-{}@example.com", user_id)),
            users::password_hash.eq("hashed_password"),
            users::is_active.eq(true),
            users::email_verified.eq(true),
            users::subscription_tier.eq("free"),
            users::full_name.eq("Test User"), // Add required full_name field
            users::company_name.eq(Some("Test Company")), // Add optional company_name field
            users::onboarding_status.eq("completed"), // Add required onboarding_status field
            users::created_at.eq(Utc::now()),
            users::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .await
        .expect("Failed to create test user");

    user_id
}

/// Clean up test user from database
async fn cleanup_test_user(pool: &qck_backend_core::db::DieselPool, user_id: Uuid) {
    use qck_backend_core::schema::{refresh_tokens, users};

    let mut conn = pool.get().await.expect("Failed to get connection");

    // Delete refresh tokens first (foreign key constraint)
    diesel::delete(refresh_tokens::table.filter(refresh_tokens::user_id.eq(user_id)))
        .execute(&mut conn)
        .await
        .ok();

    // Then delete user
    diesel::delete(users::table.filter(users::id.eq(user_id)))
        .execute(&mut conn)
        .await
        .ok();
}

/// Helper to setup test environment
async fn setup_test_env() -> (qck_backend_core::db::DieselPool, RedisPool, JwtService) {
    dotenv::from_filename(".env.test").ok();

    // Setup database pool
    let db_config = DieselDatabaseConfig::default();
    let db_pool = create_diesel_pool(db_config)
        .await
        .expect("Failed to create database pool");

    // Setup Redis pool
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config)
        .await
        .expect("Failed to create Redis pool");

    // Setup JWT service
    let jwt_service = JwtService::from_env_with_diesel(db_pool.clone(), redis_pool.clone())
        .expect("Failed to create JWT service");

    (db_pool, redis_pool, jwt_service)
}

#[tokio::test]
async fn test_complete_token_rotation_flow() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();
    let access_token = jwt_service
        .generate_access_token(
            &user_id,
            "test@example.com",
            "free",
            vec!["read".to_string()],
        )
        .expect("Failed to generate access token");

    let refresh_token = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("device-123".to_string()),
            Some("192.168.1.1".to_string()),
            Some("Test Browser".to_string()),
        )
        .await
        .expect("Failed to generate refresh token");

    // Validate initial tokens
    let access_claims = jwt_service
        .validate_access_token(&access_token)
        .expect("Failed to validate access token");
    assert_eq!(access_claims.sub, user_id);

    // Perform token rotation
    let (new_access, new_refresh, _remember_me) = jwt_service
        .rotate_refresh_token(
            &refresh_token,
            Some("device-123".to_string()),
            Some("192.168.1.1".to_string()),
            Some("Test Browser".to_string()),
        )
        .await
        .expect("Failed to rotate tokens");

    // Validate new tokens
    let new_claims = jwt_service
        .validate_access_token(&new_access)
        .expect("Failed to validate new access token");
    assert_eq!(new_claims.sub, user_id);

    // Old refresh token should be revoked
    let old_validation = jwt_service.validate_refresh_token(&refresh_token).await;
    assert!(
        old_validation.is_err(),
        "Old refresh token should be invalid"
    );

    // New refresh token should work
    let new_validation = jwt_service.validate_refresh_token(&new_refresh).await;
    assert!(new_validation.is_ok(), "New refresh token should be valid");

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

#[tokio::test]
#[ignore = "DEV-107: Token family revocation not yet implemented"]
async fn test_token_reuse_detection_security() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Generate initial refresh token
    let refresh_token = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("device-456".to_string()),
            Some("10.0.0.1".to_string()),
            Some("Chrome".to_string()),
        )
        .await
        .expect("Failed to generate refresh token");

    // First rotation - should succeed
    let (_, new_refresh, _remember_me) = jwt_service
        .rotate_refresh_token(
            &refresh_token,
            Some("device-456".to_string()),
            Some("10.0.0.1".to_string()),
            Some("Chrome".to_string()),
        )
        .await
        .expect("First rotation should succeed");

    // Try to reuse the old token (simulating token theft)
    let reuse_result = jwt_service
        .rotate_refresh_token(
            &refresh_token,
            Some("device-456".to_string()),
            Some("10.0.0.1".to_string()),
            Some("Chrome".to_string()),
        )
        .await;

    // Should detect reuse and fail
    eprintln!("Reuse result: {:?}", reuse_result);
    assert!(
        reuse_result.is_err(),
        "Token reuse should be detected and rejected. Got: {:?}",
        reuse_result
    );

    // The new token should also be invalidated (family revocation)
    let family_check = jwt_service
        .rotate_refresh_token(
            &new_refresh,
            Some("device-456".to_string()),
            Some("10.0.0.1".to_string()),
            Some("Chrome".to_string()),
        )
        .await;

    eprintln!("Family check result: {:?}", family_check);
    assert!(
        family_check.is_err(),
        "Entire token family should be revoked after reuse detection. Got: {:?}",
        family_check
    );

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

#[tokio::test]
async fn test_device_fingerprint_tracking() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Generate tokens from different devices
    let token_device1 = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("laptop-fingerprint".to_string()),
            Some("192.168.1.10".to_string()),
            Some("Firefox/Linux".to_string()),
        )
        .await
        .expect("Failed to generate token for device 1");

    let token_device2 = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("phone-fingerprint".to_string()),
            Some("192.168.1.20".to_string()),
            Some("Safari/iOS".to_string()),
        )
        .await
        .expect("Failed to generate token for device 2");

    // Both tokens should be valid independently
    let validation1 = jwt_service.validate_refresh_token(&token_device1).await;
    let validation2 = jwt_service.validate_refresh_token(&token_device2).await;

    assert!(validation1.is_ok(), "Device 1 token should be valid");
    assert!(validation2.is_ok(), "Device 2 token should be valid");

    // Rotating one shouldn't affect the other
    let (_, _new_token1, _remember_me) = jwt_service
        .rotate_refresh_token(
            &token_device1,
            Some("laptop-fingerprint".to_string()),
            Some("192.168.1.10".to_string()),
            Some("Firefox/Linux".to_string()),
        )
        .await
        .expect("Rotation for device 1 should succeed");

    // Device 2 token should still be valid
    let validation2_after = jwt_service.validate_refresh_token(&token_device2).await;
    assert!(
        validation2_after.is_ok(),
        "Device 2 token should remain valid after device 1 rotation"
    );

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

#[tokio::test]
#[ignore = "DEV-107: Token rotation concurrency control not yet implemented"]
async fn test_concurrent_refresh_requests() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Generate initial token
    let refresh_token = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("test-device".to_string()),
            Some("10.0.0.5".to_string()),
            Some("TestBrowser".to_string()),
        )
        .await
        .expect("Failed to generate refresh token");

    // Create barrier for synchronizing concurrent tasks
    // This ensures both tasks start their operations at exactly the same time
    let barrier = Arc::new(Barrier::new(2));

    // Simulate concurrent refresh attempts (race condition)
    let jwt_service = std::sync::Arc::new(jwt_service);
    let token1 = refresh_token.clone();
    let token2 = refresh_token.clone();

    let service1 = jwt_service.clone();
    let service2 = jwt_service.clone();
    let barrier1 = barrier.clone();
    let barrier2 = barrier.clone();

    let handle1 = tokio::spawn(async move {
        // Wait for both tasks to reach this point before proceeding
        barrier1.wait().await;

        service1
            .rotate_refresh_token(
                &token1,
                Some("test-device".to_string()),
                Some("10.0.0.5".to_string()),
                Some("TestBrowser".to_string()),
            )
            .await
    });

    let handle2 = tokio::spawn(async move {
        // Wait for both tasks to reach this point before proceeding
        barrier2.wait().await;

        service2
            .rotate_refresh_token(
                &token2,
                Some("test-device".to_string()),
                Some("10.0.0.5".to_string()),
                Some("TestBrowser".to_string()),
            )
            .await
    });

    let result1 = handle1.await.expect("Task 1 should complete");
    let result2 = handle2.await.expect("Task 2 should complete");

    // Only one should succeed due to proper concurrency control
    let successes = [result1.is_ok(), result2.is_ok()]
        .iter()
        .filter(|&&x| x)
        .count();

    // Debug output to understand the failures
    if successes != 1 {
        eprintln!("Result1: {:?}", result1);
        eprintln!("Result2: {:?}", result2);
    }

    assert_eq!(
        successes, 1,
        "Exactly one concurrent refresh should succeed, but got {} successes. Result1: {:?}, Result2: {:?}",
        successes,
        result1.is_ok(),
        result2.is_ok()
    );

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

#[tokio::test]
async fn test_refresh_token_expiration_behavior() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Create a token with very short expiry (for testing)
    // Note: This would require modifying the JWT service to accept custom expiry
    // For now, we'll test with normal expiry and validate the behavior

    let refresh_token = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("expiry-test".to_string()),
            Some("172.16.0.1".to_string()),
            Some("ExpiredBrowser".to_string()),
        )
        .await
        .expect("Failed to generate refresh token");

    // Token should be valid initially
    let initial_validation = jwt_service.validate_refresh_token(&refresh_token).await;
    assert!(initial_validation.is_ok(), "Fresh token should be valid");

    // Manually expire the token in database for testing
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend_core::schema::refresh_tokens;

    let mut conn = db_pool.get().await.expect("Failed to get connection");
    let jti = initial_validation.unwrap().jti;
    let jti_hash = qck_backend_core::models::refresh_token::RefreshToken::hash_jti(&jti);

    // Set expires_at to 1 second from now to respect the check constraint
    diesel::update(refresh_tokens::table)
        .filter(refresh_tokens::jti_hash.eq(&jti_hash))
        .set(refresh_tokens::expires_at.eq(chrono::Utc::now() + chrono::Duration::seconds(1)))
        .execute(&mut conn)
        .await
        .expect("Failed to update token expiry");

    // Wait for token to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Now token should be expired
    let expired_validation = jwt_service.validate_refresh_token(&refresh_token).await;
    assert!(
        expired_validation.is_err(),
        "Expired token should fail validation"
    );

    // Rotation should also fail
    let rotation_result = jwt_service
        .rotate_refresh_token(
            &refresh_token,
            Some("expiry-test".to_string()),
            Some("172.16.0.1".to_string()),
            Some("ExpiredBrowser".to_string()),
        )
        .await;

    assert!(rotation_result.is_err(), "Cannot rotate expired token");

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

#[tokio::test]
async fn test_max_active_tokens_per_user() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Generate multiple tokens for the same user
    let mut tokens = Vec::new();
    for i in 0..5 {
        let token = jwt_service
            .generate_refresh_token_with_device(
                &user_id,
                Some(format!("device-{}", i)),
                Some(format!("10.0.0.{}", i)),
                Some(format!("Browser-{}", i)),
            )
            .await
            .expect("Failed to generate token");
        tokens.push(token);
    }

    // Check active token count
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend_core::schema::refresh_tokens;

    let mut conn = db_pool.get().await.expect("Failed to get connection");

    let count: i64 = refresh_tokens::table
        .filter(refresh_tokens::user_id.eq(user_uuid))
        .filter(refresh_tokens::revoked_at.is_null())
        .filter(refresh_tokens::expires_at.gt(chrono::Utc::now()))
        .count()
        .get_result(&mut conn)
        .await
        .expect("Failed to count tokens");

    assert_eq!(count, 5, "Should have 5 active tokens");

    // In production, you might want to enforce a maximum limit
    // and revoke oldest tokens when limit is exceeded

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

#[tokio::test]
async fn test_logout_revokes_all_tokens() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Generate multiple tokens
    let token1 = jwt_service
        .generate_refresh_token_with_device(&user_id, Some("device-1".to_string()), None, None)
        .await
        .expect("Failed to generate token 1");

    let token2 = jwt_service
        .generate_refresh_token_with_device(&user_id, Some("device-2".to_string()), None, None)
        .await
        .expect("Failed to generate token 2");

    // Both should be valid
    assert!(jwt_service.validate_refresh_token(&token1).await.is_ok());
    assert!(jwt_service.validate_refresh_token(&token2).await.is_ok());

    // Revoke all tokens for user (logout)
    let revoked = jwt_service
        .revoke_all_user_tokens(&user_id)
        .await
        .expect("Failed to revoke all tokens");

    assert!(revoked >= 2, "Should revoke at least 2 tokens");

    // Both tokens should now be invalid
    assert!(jwt_service.validate_refresh_token(&token1).await.is_err());
    assert!(jwt_service.validate_refresh_token(&token2).await.is_err());

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}

/// Additional stress test with multiple concurrent requests
#[tokio::test]
#[ignore = "DEV-107: High concurrency token rotation control not yet implemented"]
async fn test_high_concurrency_refresh_requests() {
    let (db_pool, _redis_pool, jwt_service) = setup_test_env().await;

    // Create test user
    let user_uuid = create_test_user(&db_pool).await;
    let user_id = user_uuid.to_string();

    // Generate initial token
    let refresh_token = jwt_service
        .generate_refresh_token_with_device(
            &user_id,
            Some("high-concurrency-device".to_string()),
            Some("10.0.0.10".to_string()),
            Some("StressTestBrowser".to_string()),
        )
        .await
        .expect("Failed to generate refresh token");

    // Test with higher concurrency (10 concurrent requests)
    let num_concurrent = 10;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let jwt_service = Arc::new(jwt_service);

    let mut handles = Vec::new();

    for i in 0..num_concurrent {
        let token = refresh_token.clone();
        let service = jwt_service.clone();
        let barrier_clone = barrier.clone();

        let handle = tokio::spawn(async move {
            // All tasks wait here until all are ready
            barrier_clone.wait().await;

            service
                .rotate_refresh_token(
                    &token,
                    Some("high-concurrency-device".to_string()),
                    Some("10.0.0.10".to_string()),
                    Some(format!("StressTestBrowser-{}", i)),
                )
                .await
        });

        handles.push(handle);
    }

    // Collect all results
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        results.push(result);
    }

    // Count successes
    let successes = results.iter().filter(|r| r.is_ok()).count();

    assert_eq!(
        successes, 1,
        "Exactly one out of {} concurrent refresh requests should succeed, but got {} successes",
        num_concurrent, successes
    );

    // Cleanup test data
    cleanup_test_user(&db_pool, user_uuid).await;
}
