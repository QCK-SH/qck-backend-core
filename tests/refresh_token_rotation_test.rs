// Unit Tests for Refresh Token Rotation
// DEV-107: Comprehensive tests for token rotation, reuse detection, and security

use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
// Removed ipnetwork - using String for IP addresses for compatibility and simplification; avoids extra dependency and serialization issues in tests
use qck_backend::{
    db::{create_diesel_pool, DieselDatabaseConfig},
    models::refresh_token::{DeviceInfo, RefreshToken, RefreshTokenError},
};
use uuid::Uuid;

/// Helper function to setup test database pool
async fn setup_test_pool() -> qck_backend::db::DieselPool {
    dotenv::from_filename(".env.test").ok();
    let config = DieselDatabaseConfig::default();
    create_diesel_pool(config)
        .await
        .expect("Failed to create test pool")
}

/// Helper function to create a test user
async fn create_test_user(pool: &qck_backend::db::DieselPool) -> Uuid {
    use qck_backend::schema::users;

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

/// Helper function to cleanup test data
async fn cleanup_test_data(pool: &qck_backend::db::DieselPool, user_id: Uuid) {
    use qck_backend::schema::{refresh_tokens, users};

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

#[tokio::test]
async fn test_refresh_token_storage_with_device_info() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let jti = "test-jti-123";
    let token_family = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);
    let device_fingerprint = Some("device123".to_string());
    let ip_address: Option<String> = Some("192.168.1.1".to_string());
    let user_agent = Some("Mozilla/5.0 Test Browser".to_string());

    // Store refresh token with device info
    let stored_token = RefreshToken::store(
        &mut conn,
        user_id,
        jti,
        expires_at,
        token_family.clone(),
        DeviceInfo {
            fingerprint: device_fingerprint.clone(),
            ip_address: ip_address.clone(),
            user_agent: user_agent.clone(),
        },
    )
    .await
    .expect("Failed to store refresh token");

    // Verify stored data
    assert_eq!(stored_token.user_id, user_id);
    assert_eq!(stored_token.token_family, token_family);
    assert_eq!(stored_token.device_fingerprint, device_fingerprint);
    assert_eq!(stored_token.ip_address, ip_address);
    assert_eq!(stored_token.user_agent, user_agent);
    assert!(stored_token.revoked_at.is_none());

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_token_family_tracking() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let token_family = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    // Create multiple tokens in the same family
    for i in 0..3 {
        let jti = format!("family-test-{}", i);
        RefreshToken::store(
            &mut conn,
            user_id,
            &jti,
            expires_at,
            token_family.clone(),
            DeviceInfo::default(),
        )
        .await
        .expect("Failed to store token");
    }

    // Revoke entire family
    let revoked_count =
        RefreshToken::revoke_token_family(&mut conn, &token_family, "test_revocation")
            .await
            .expect("Failed to revoke family");

    assert_eq!(revoked_count, 3, "Should revoke all 3 tokens in family");

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_token_reuse_detection() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let jti = "reuse-test-123";
    let token_family = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    // Store and then revoke a token
    RefreshToken::store(
        &mut conn,
        user_id,
        jti,
        expires_at,
        token_family,
        DeviceInfo::default(),
    )
    .await
    .expect("Failed to store token");

    RefreshToken::revoke(&mut conn, jti)
        .await
        .expect("Failed to revoke token");

    // Detect reuse of revoked token
    let is_reused = RefreshToken::detect_token_reuse(&mut conn, jti)
        .await
        .expect("Failed to detect reuse");

    assert!(is_reused, "Should detect token reuse");

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_suspicious_activity_detection() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let expires_at = Utc::now() + Duration::hours(24);

    // Create tokens from different devices/IPs rapidly
    for i in 0..5 {
        let jti = format!("suspicious-{}", i);
        let token_family = Uuid::new_v4().to_string();
        let device_fingerprint = Some(format!("device-{}", i));
        let ip_address: Option<String> = Some(format!("192.168.1.{}", i));

        RefreshToken::store(
            &mut conn,
            user_id,
            &jti,
            expires_at,
            token_family,
            DeviceInfo {
                fingerprint: device_fingerprint,
                ip_address,
                user_agent: None,
            },
        )
        .await
        .expect("Failed to store token");
    }

    // Check for suspicious activity
    let suspicious = RefreshToken::check_suspicious_activity(
        &mut conn,
        user_id,
        Some("new-device"),
        Some("192.168.1.100"),
    )
    .await
    .expect("Failed to check suspicious activity");

    assert!(
        suspicious,
        "Should detect suspicious activity with many different devices"
    );

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_token_expiration_validation() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let jti = "expiry-test";
    let token_family = Uuid::new_v4().to_string();

    // Create a token that will expire soon (not already expired due to DB constraint)
    let expires_at = Utc::now() + Duration::milliseconds(100);

    RefreshToken::store(
        &mut conn,
        user_id,
        jti,
        expires_at,
        token_family,
        DeviceInfo::default(),
    )
    .await
    .expect("Failed to store token");

    // Wait for token to expire with extra buffer
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Try to validate expired token
    let result = RefreshToken::validate(&mut conn, jti).await;

    assert!(
        matches!(result, Err(RefreshTokenError::Expired)),
        "Should return Expired error"
    );

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_active_token_count_limit() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let expires_at = Utc::now() + Duration::hours(24);

    // Create multiple active tokens
    for i in 0..5 {
        let jti = format!("count-test-{}", i);
        let token_family = Uuid::new_v4().to_string();

        RefreshToken::store(
            &mut conn,
            user_id,
            &jti,
            expires_at,
            token_family,
            DeviceInfo::default(),
        )
        .await
        .expect("Failed to store token");
    }

    // Count active tokens
    let count = RefreshToken::count_active_for_user(&mut conn, user_id)
        .await
        .expect("Failed to count tokens");

    assert_eq!(count, 5, "Should have 5 active tokens");

    // Revoke one token
    RefreshToken::revoke(&mut conn, "count-test-0")
        .await
        .expect("Failed to revoke token");

    let count_after = RefreshToken::count_active_for_user(&mut conn, user_id)
        .await
        .expect("Failed to count tokens");

    assert_eq!(
        count_after, 4,
        "Should have 4 active tokens after revocation"
    );

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_mark_token_as_used() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    let jti = "usage-test";
    let token_family = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    let token = RefreshToken::store(
        &mut conn,
        user_id,
        jti,
        expires_at,
        token_family,
        DeviceInfo {
            fingerprint: None,
            ip_address: None,
            user_agent: None,
        },
    )
    .await
    .expect("Failed to store token");

    assert!(
        token.last_used_at.is_none(),
        "New token should not have last_used_at"
    );

    // Mark as used
    let marked = RefreshToken::mark_as_used(&mut conn, jti)
        .await
        .expect("Failed to mark as used");

    assert!(marked, "Should successfully mark token as used");

    // Verify it was updated
    let updated_token = RefreshToken::validate(&mut conn, jti)
        .await
        .expect("Failed to validate token");

    assert!(
        updated_token.last_used_at.is_some(),
        "Token should have last_used_at after being marked as used"
    );

    cleanup_test_data(&pool, user_id).await;
}

#[tokio::test]
async fn test_cleanup_expired_tokens() {
    let pool = setup_test_pool().await;
    let user_id = create_test_user(&pool).await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    // Create mix of soon-to-expire and active tokens
    for i in 0..3 {
        let jti = format!("cleanup-expired-{}", i);
        let token_family = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(1); // Will expire soon

        RefreshToken::store(
            &mut conn,
            user_id,
            &jti,
            expires_at,
            token_family,
            DeviceInfo::default(),
        )
        .await
        .expect("Failed to store soon-to-expire token");
    }

    // Wait for tokens to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    for i in 0..2 {
        let jti = format!("cleanup-active-{}", i);
        let token_family = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::hours(1); // Active

        RefreshToken::store(
            &mut conn,
            user_id,
            &jti,
            expires_at,
            token_family,
            DeviceInfo::default(),
        )
        .await
        .expect("Failed to store active token");
    }

    // Cleanup expired tokens
    let cleaned = RefreshToken::cleanup_expired(&mut conn)
        .await
        .expect("Failed to cleanup");

    assert!(cleaned >= 3, "Should cleanup at least 3 expired tokens");

    // Verify active tokens remain
    let active_count = RefreshToken::count_active_for_user(&mut conn, user_id)
        .await
        .expect("Failed to count");

    assert_eq!(active_count, 2, "Should still have 2 active tokens");

    cleanup_test_data(&pool, user_id).await;
}
