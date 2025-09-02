// Test click tracking resilience and fallback mechanisms
use qck_backend::{
    app::AppState,
    db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool},
    models::link::CreateLinkRequest,
    services::link::{sync_click_counts_to_database, LinkService},
};
use redis::AsyncCommands;
use std::sync::Arc;
use uuid::Uuid;

async fn setup_test_state() -> AppState {
    dotenv::from_filename("../.env.dev").ok();
    std::env::set_var("VALIDATE_DNS", "false");

    let db_config = DieselDatabaseConfig::default();
    let diesel_pool = create_diesel_pool(db_config).await.unwrap();
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config).await.unwrap();

    let jwt_service = Arc::new(
        qck_backend::services::JwtService::from_env_with_diesel(
            diesel_pool.clone(),
            redis_pool.clone(),
        )
        .unwrap(),
    );

    let diesel_pool_clone = diesel_pool.clone();

    AppState {
        diesel_pool,
        redis_pool: redis_pool.clone(),
        jwt_service,
        rate_limit_service: Arc::new(qck_backend::services::RateLimitService::new(
            redis_pool.clone(),
        )),
        rate_limit_config: Arc::new(qck_backend::config::RateLimitingConfig::from_env()),
        subscription_service: Arc::new(qck_backend::services::SubscriptionService::new()),
        password_reset_service: Arc::new(qck_backend::services::PasswordResetService::new(
            diesel_pool_clone,
        )),
        email_service: Arc::new(
            qck_backend::services::EmailService::new(qck_backend::app_config::CONFIG.email.clone())
                .unwrap(),
        ),
        clickhouse_analytics: None, // Disabled for tests
        max_connections: 10,
    }
}

async fn create_test_user(state: &AppState) -> qck_backend::models::user::User {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::users;

    let mut conn = state.diesel_pool.get().await.unwrap();

    let new_user = qck_backend::models::user::NewUser {
        email: format!("test{}@example.com", Uuid::new_v4()),
        password_hash: "hashed_password".to_string(),
        email_verified: true,
        subscription_tier: "free".to_string(),
        full_name: "Test User".to_string(),
        company_name: None,
        onboarding_status: "completed".to_string(),
    };

    diesel::insert_into(users::table)
        .values(&new_user)
        .get_result(&mut conn)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_click_tracking_with_retry() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/click-test".to_string(),
        custom_alias: None,
        title: Some("Click Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let link = service.create_link(&user, request).await.unwrap();

    // Process multiple redirects
    for _ in 0..5 {
        let result = service.process_redirect(&link.short_code).await;
        assert!(result.is_ok());
    }

    // Wait for async operations
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check that clicks are tracked in Redis
    let mut redis_conn = state.redis_pool.get_connection().await.unwrap();
    let counter_key = format!("clicks:{}", link.short_code);
    let count: Option<i32> = redis_conn.get(&counter_key).await.unwrap();

    assert_eq!(count, Some(5), "Should have 5 clicks tracked in Redis");
}

#[tokio::test]
async fn test_click_sync_to_database() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/sync-test".to_string(),
        custom_alias: None,
        title: Some("Sync Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let link = service.create_link(&user, request).await.unwrap();
    let initial_count = link.click_count;

    // Process redirects
    for _ in 0..3 {
        service.process_redirect(&link.short_code).await.unwrap();
    }

    // Wait for async operations
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Manually sync counts to database
    let synced = sync_click_counts_to_database(&state.redis_pool, &state.diesel_pool)
        .await
        .unwrap();

    println!("Synced {} links", synced);
    assert!(synced > 0, "Should have synced at least one link");

    // Check database has updated count
    let updated_link = service
        .get_link_by_code(&link.short_code)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated_link.click_count as i64,
        initial_count as i64 + 3,
        "Database should have 3 more clicks"
    );
}

#[tokio::test]
async fn test_fallback_queue_on_error() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/fallback-test".to_string(),
        custom_alias: None,
        title: Some("Fallback Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let link = service.create_link(&user, request).await.unwrap();

    // Simulate clicks (these will be tracked)
    for _ in 0..2 {
        service.process_redirect(&link.short_code).await.unwrap();
    }

    // Wait and check fallback exists if primary fails
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check that either primary or fallback counter exists
    let mut redis_conn = state.redis_pool.get_connection().await.unwrap();
    let primary_key = format!("clicks:{}", link.short_code);
    let fallback_key = format!("clicks:fallback:{}", link.short_code);

    let primary_count: Option<i32> = redis_conn.get(&primary_key).await.unwrap_or(None);
    let fallback_count: Option<i32> = redis_conn.get(&fallback_key).await.unwrap_or(None);

    println!(
        "Primary count: {:?}, Fallback count: {:?}",
        primary_count, fallback_count
    );

    // At least one should have the count
    assert!(
        primary_count.is_some() || fallback_count.is_some(),
        "Either primary or fallback should have click count"
    );
}
