// Integration tests for JWT token refresh functionality
// DEV-94: Test token refresh with rotation mechanism

mod common;

use chrono::Utc;
use diesel::prelude::*;
use qck_backend_core::{
    db::{create_diesel_pool, DieselDatabaseConfig, DieselPool, RedisConfig},
    JwtConfig, JwtService, RedisPool,
};
use sha2::{Digest, Sha256};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Helper function to create test JWT service with database and Redis
async fn create_test_jwt_service() -> (JwtService, DieselPool, RedisPool) {
    // Set test environment variables (secrets must be at least 32 chars)
    env::set_var(
        "JWT_ACCESS_SECRET",
        "test-access-secret-hs256-minimum-32-characters-long",
    );
    env::set_var(
        "JWT_REFRESH_SECRET",
        "test-refresh-secret-hs256-minimum-32-characters-long",
    );
    env::set_var("JWT_ACCESS_EXPIRY", "3600");
    env::set_var("JWT_REFRESH_EXPIRY", "604800");
    env::set_var("JWT_KEY_VERSION", "1");
    env::set_var("JWT_AUDIENCE", "qck.sh");
    env::set_var("JWT_ISSUER", "qck.sh");

    // Initialize database pool
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://qck_user:qck_password@localhost:15001/qck_test".to_string()
    });

    let db_config = DieselDatabaseConfig {
        url: database_url,
        max_connections: 10,
        min_connections: 2,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(600),
        max_lifetime: Duration::from_secs(1800),
        test_on_checkout: true, // Enabled to better simulate production behavior
    };

    let diesel_pool = create_diesel_pool(db_config)
        .await
        .expect("Failed to create database pool");

    // Initialize Redis pool
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:15002".to_string());

    let redis_config = RedisConfig {
        redis_url,
        pool_size: 10,
        connection_timeout: Duration::from_secs(5),
        command_timeout: Duration::from_secs(5),
        retry_attempts: 3,
        retry_delay: Duration::from_millis(100),
        idle_timeout: Duration::from_secs(60),
        max_lifetime: Duration::from_secs(300),
    };

    let redis_pool = RedisPool::new(redis_config)
        .await
        .expect("Failed to create Redis pool");

    // Create JWT service
    let jwt_config = JwtConfig::from_env().expect("Failed to load JWT config");
    let jwt_service = JwtService::new(jwt_config);

    (jwt_service, diesel_pool, redis_pool)
}

/// Helper to create a test user in the database
async fn create_test_user(pool: &DieselPool) -> Result<Uuid, diesel::result::Error> {
    let mut conn = pool
        .get()
        .await
        .expect("Failed to get connection from pool");

    use qck_backend_core::schema::users;

    let user_id = Uuid::new_v4();
    let now = Utc::now();

    // Use fully qualified syntax to disambiguate
    diesel_async::RunQueryDsl::execute(
        diesel::insert_into(users::table).values((
            users::id.eq(user_id),
            users::email.eq(format!("test{}@example.com", user_id)),
            users::password_hash.eq("test_hash"),
            users::is_active.eq(true),
            users::created_at.eq(now),
            users::updated_at.eq(now),
            users::email_verified.eq(true),
            users::subscription_tier.eq("free"),
            users::full_name.eq("Test User"),
            users::company_name.eq(Some("Test Company")),
            users::onboarding_status.eq("completed"),
        )),
        &mut conn,
    )
    .await?;

    Ok(user_id)
}

/// Helper to store a refresh token in the database using Diesel
async fn store_refresh_token(
    pool: &DieselPool,
    user_id: Uuid,
    jti: &str,
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), diesel::result::Error> {
    let mut conn = pool
        .get()
        .await
        .expect("Failed to get connection from pool");

    use qck_backend_core::schema::refresh_tokens;

    // Hash the JTI to match the actual schema
    let jti_hash = format!("{:x}", Sha256::digest(jti.as_bytes()));

    // For expired tokens, ensure created_at is before expires_at
    let created_at = if expires_at < Utc::now() {
        // Token is expired, set created_at to 2 hours before expiry
        expires_at - chrono::Duration::hours(2)
    } else {
        // Token is not expired, use current time
        Utc::now()
    };

    // Use Diesel query builder instead of raw SQL
    // Use fully qualified syntax to disambiguate
    diesel_async::RunQueryDsl::execute(
        diesel::insert_into(refresh_tokens::table).values((
            refresh_tokens::id.eq(Uuid::new_v4()),
            refresh_tokens::user_id.eq(user_id),
            refresh_tokens::jti_hash.eq(jti_hash),
            refresh_tokens::expires_at.eq(expires_at),
            refresh_tokens::created_at.eq(created_at),
            refresh_tokens::token_family.eq(Uuid::new_v4().to_string()),
            refresh_tokens::issued_at.eq(created_at),
            refresh_tokens::updated_at.eq(created_at),
        )),
        &mut conn,
    )
    .await?;

    Ok(())
}

/// Helper to validate a refresh token from the database using Diesel
async fn validate_refresh_token(
    pool: &DieselPool,
    jti: &str,
) -> Result<bool, diesel::result::Error> {
    let mut conn = pool
        .get()
        .await
        .expect("Failed to get connection from pool");

    use qck_backend_core::schema::refresh_tokens;

    // Hash the JTI to match the actual schema
    let jti_hash = format!("{:x}", Sha256::digest(jti.as_bytes()));

    // Use Diesel query builder instead of raw SQL
    // Use fully qualified syntax to disambiguate
    let result: Option<RefreshTokenRow> = diesel_async::RunQueryDsl::first(
        refresh_tokens::table
            .select((refresh_tokens::revoked_at, refresh_tokens::expires_at))
            .filter(refresh_tokens::jti_hash.eq(jti_hash)),
        &mut conn,
    )
    .await
    .optional()?;

    match result {
        Some(row) => {
            if row.revoked_at.is_some() {
                Ok(false) // Token is revoked
            } else if row.expires_at < Utc::now() {
                Ok(false) // Token is expired
            } else {
                Ok(true) // Token is valid
            }
        },
        None => Ok(false), // Token not found
    }
}

#[derive(Queryable)]
struct RefreshTokenRow {
    revoked_at: Option<chrono::DateTime<Utc>>,
    expires_at: chrono::DateTime<Utc>,
}

#[tokio::test]
async fn test_token_refresh_with_rotation() {
    let (jwt_service, _postgres_pool, _redis_pool) = create_test_jwt_service().await;

    // Generate initial token pair
    let user_id = Uuid::new_v4().to_string();
    let email = "test@example.com";
    let subscription_tier = "free";
    let permissions = vec!["read".to_string(), "write".to_string()];

    // Generate initial access token
    let initial_access = jwt_service
        .generate_access_token(&user_id, email, subscription_tier, permissions.clone())
        .expect("Failed to generate initial access token");

    let initial_refresh = jwt_service
        .generate_refresh_token(&user_id)
        .await
        .expect("Failed to generate initial refresh token");

    // Validate initial tokens work
    let access_claims = jwt_service
        .validate_access_token(&initial_access)
        .expect("Initial access token should be valid");
    assert_eq!(access_claims.sub, user_id);
    assert_eq!(access_claims.email, email);

    let refresh_claims = jwt_service
        .validate_refresh_token(&initial_refresh)
        .await
        .expect("Initial refresh token should be valid");
    assert_eq!(refresh_claims.sub, user_id);

    // Wait a moment to ensure different timestamps
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Generate new tokens (simulating refresh)
    let new_access = jwt_service
        .generate_access_token(&user_id, email, subscription_tier, permissions.clone())
        .expect("Failed to generate new access token");

    let new_refresh = jwt_service
        .generate_refresh_token(&user_id)
        .await
        .expect("Failed to generate new refresh token");

    // Verify new tokens are different from old ones
    assert_ne!(initial_access, new_access);
    assert_ne!(initial_refresh, new_refresh);

    // Verify new access token is valid
    let new_access_claims = jwt_service
        .validate_access_token(&new_access)
        .expect("New access token should be valid");
    assert_eq!(new_access_claims.sub, user_id);
    assert_eq!(new_access_claims.email, email);

    // Verify new refresh token is valid
    let new_refresh_claims = jwt_service
        .validate_refresh_token(&new_refresh)
        .await
        .expect("New refresh token should be valid");
    assert_eq!(new_refresh_claims.sub, user_id);
}

#[tokio::test]
async fn test_refresh_token_expiry() {
    let (_jwt_service, postgres_pool, _redis_pool) = create_test_jwt_service().await;

    // Create a test user first
    let user_id = create_test_user(&postgres_pool).await.unwrap();
    let jti = Uuid::new_v4().to_string();

    // Create a refresh token that expires in 1 second
    let expires_soon = Utc::now() + chrono::Duration::seconds(1);

    store_refresh_token(&postgres_pool, user_id, &jti, expires_soon)
        .await
        .expect("Should store token");

    // Wait for token to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Validate through the database which will detect expiry
    let validation_result = validate_refresh_token(&postgres_pool, &jti).await;

    assert_eq!(
        validation_result.unwrap(),
        false,
        "Expired token validation should fail"
    );
}

#[tokio::test]
async fn test_concurrent_token_generation() {
    let (jwt_service, _postgres_pool, _redis_pool) = create_test_jwt_service().await;

    let user_id = Uuid::new_v4().to_string();
    let email = "concurrent@example.com";
    let subscription_tier = "free";
    let permissions = vec!["read".to_string(), "write".to_string()];

    // Use Arc to share jwt_service across threads
    let jwt_service = Arc::new(jwt_service);

    // Create futures for concurrent token generation
    let mut handles = Vec::new();

    for i in 0..3 {
        let jwt_service_clone = jwt_service.clone();
        let user_id_clone = format!("{}-{}", user_id, i);
        let email_clone = format!("user{}@example.com", i);
        let tier_clone = subscription_tier.to_string();
        let permissions_clone = permissions.clone();

        let handle = tokio::spawn(async move {
            jwt_service_clone.generate_access_token(
                &user_id_clone,
                &email_clone,
                &tier_clone,
                permissions_clone,
            )
        });

        handles.push(handle);
    }

    let mut success_count = 0;

    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => success_count += 1,
            Err(e) => panic!("Token generation failed: {:?}", e),
        }
    }

    assert_eq!(
        success_count, 3,
        "All concurrent token generations should succeed"
    );
}

#[tokio::test]
async fn test_token_metadata_in_claims() {
    let (jwt_service, _postgres_pool, _redis_pool) = create_test_jwt_service().await;

    let user_id = Uuid::new_v4().to_string();
    let email = "metadata@example.com";
    let subscription_tier = "free";
    let permissions = vec!["read".to_string(), "write".to_string(), "admin".to_string()];

    // Generate token with metadata
    let access_token = jwt_service
        .generate_access_token(&user_id, email, subscription_tier, permissions.clone())
        .expect("Failed to generate access token");

    // Validate token and check metadata
    let claims = jwt_service
        .validate_access_token(&access_token)
        .expect("Access token should be valid");

    assert_eq!(claims.email, email);
    assert_eq!(claims.tier, subscription_tier);
    assert_eq!(claims.scope, permissions);
    assert_eq!(claims.sub, user_id);
}

#[tokio::test]
async fn test_token_audience_and_issuer() {
    let (jwt_service, _postgres_pool, _redis_pool) = create_test_jwt_service().await;

    let user_id = Uuid::new_v4().to_string();
    let email = "audience@example.com";
    let subscription_tier = "free";
    let permissions = vec!["read".to_string()];

    // Generate token
    let access_token = jwt_service
        .generate_access_token(&user_id, email, subscription_tier, permissions)
        .expect("Failed to generate access token");

    // Validate token and check audience/issuer
    let claims = jwt_service
        .validate_access_token(&access_token)
        .expect("Access token should be valid");

    assert_eq!(claims.aud, "qck.sh", "Audience should match");
    assert_eq!(claims.iss, "qck.sh", "Issuer should match");
}
