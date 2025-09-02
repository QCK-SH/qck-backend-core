// Simple test to debug link creation issue
use qck_backend::{
    app::AppState,
    db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool},
    models::link::CreateLinkRequest,
    services::link::LinkService,
};
use std::sync::Arc;
use uuid::Uuid;

async fn setup_test_state() -> AppState {
    dotenv::from_filename("../.env.dev").ok();
    std::env::set_var("VALIDATE_DNS", "false");

    println!("VALIDATE_DNS env var: {:?}", std::env::var("VALIDATE_DNS"));

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
async fn test_simple_link_creation() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    let request = CreateLinkRequest {
        url: "https://example.com/test".to_string(),
        custom_alias: None,
        title: Some("Test Link".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    println!("Creating link with URL: {}", request.url);

    match service.create_link(&user, request).await {
        Ok(link) => {
            println!("Successfully created link: {}", link.short_code);
            assert!(!link.short_code.is_empty());
        },
        Err(e) => {
            panic!("Failed to create link: {:?}", e);
        },
    }
}
