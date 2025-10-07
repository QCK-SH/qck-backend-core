// DEV-114: Link CRUD Operations Tests
// Comprehensive tests for Create, Read, Update, Delete operations

use chrono::{Duration, Utc};
use qck_backend_core::{
    app::AppState,
    db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool},
    models::user::User,
};
use std::sync::Arc;
use uuid::Uuid;

// Import types we'll use in tests
type CreateLinkRequest = qck_backend_core::models::link::CreateLinkRequest;
type UpdateLinkRequest = qck_backend_core::models::link::UpdateLinkRequest;
type LinkFilter = qck_backend_core::models::link::LinkFilter;
type LinkPagination = qck_backend_core::models::link::LinkPagination;
type ListLinksParams = qck_backend_core::services::link::ListLinksParams;
type LinkService = qck_backend_core::services::link::LinkService;

async fn setup_test_state() -> AppState {
        config: Arc::new(qck_backend_core::app_config::CONFIG.clone()),
    // Load environment for testing
    dotenv::from_filename(".env.test").ok();

    // Create database pool
    let db_config = DieselDatabaseConfig::default();
    let diesel_pool = create_diesel_pool(db_config).await.unwrap();

    // Create Redis pool
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config).await.unwrap();

    // Create mock services
    let jwt_service = Arc::new(
        qck_backend_core::services::JwtService::from_env_with_diesel(
            diesel_pool.clone(),
            redis_pool.clone(),
        )
        .unwrap(),
    );

    let diesel_pool_clone = diesel_pool.clone();

    AppState {
        config: Arc::new(qck_backend_core::app_config::CONFIG.clone()),
        diesel_pool,
        redis_pool: redis_pool.clone(),
        jwt_service,
        rate_limit_service: Arc::new(qck_backend_core::services::RateLimitService::new(
            redis_pool.clone(),
        )),
        rate_limit_config: Arc::new(qck_backend_core::config::RateLimitingConfig::from_env()),
        password_reset_service: Arc::new(qck_backend_core::services::PasswordResetService::new(
            diesel_pool_clone,
        )),
        email_service: Arc::new(
            qck_backend_core::services::EmailService::new(qck_backend_core::app_config::CONFIG.email.clone())
                .unwrap(),
        ),
        clickhouse_analytics: None, // Disabled for tests
        max_connections: 10,
    }
}

async fn create_test_user(state: &AppState) -> User {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend_core::schema::users;

    let mut conn = state.diesel_pool.get().await.unwrap();

    // Use the actual NewUser struct fields
    let new_user = qck_backend_core::models::user::NewUser {
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
#[ignore] // Requires database
async fn test_create_link() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    let request = CreateLinkRequest {
        url: "https://example.com/test".to_string(),
        custom_alias: None,
        title: Some("Test Link".to_string()),
        description: Some("A test link for CRUD operations".to_string()),
        expires_at: None,
        tags: vec!["test".to_string()],
        is_password_protected: false,
        password: None,
    };

    let result = service.create_link(&user, request).await;
    assert!(result.is_ok());

    let link_response = result.unwrap();
    assert!(!link_response.short_url.is_empty());
    assert!(link_response.short_url.contains("https://"));
    assert_eq!(link_response.original_url, "https://example.com/test");
    assert_eq!(link_response.title, Some("Test Link".to_string()));
}

#[tokio::test]
#[ignore] // Requires database
async fn test_create_link_with_custom_alias() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    let custom_alias = format!(
        "test-{}",
        Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    let request = CreateLinkRequest {
        url: "https://example.com/custom".to_string(),
        custom_alias: Some(custom_alias.clone()),
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let result = service.create_link(&user, request).await;
    assert!(result.is_ok());

    let link_response = result.unwrap();
    assert!(link_response.short_url.contains(&custom_alias));
}

#[tokio::test]
#[ignore] // Requires database
async fn test_get_link() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // First create a link
    let request = CreateLinkRequest {
        url: "https://example.com/retrieve".to_string(),
        custom_alias: None,
        title: Some("Retrievable Link".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Now retrieve it by short code
    let retrieved = service.get_link(&created.short_code).await;
    assert!(retrieved.is_ok());

    let link = retrieved.unwrap();
    assert_eq!(link.original_url, "https://example.com/retrieve");
    assert_eq!(link.short_code, created.short_code);
}

#[tokio::test]
#[ignore] // Requires database
async fn test_update_link() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/original".to_string(),
        custom_alias: None,
        title: Some("Original Title".to_string()),
        description: Some("Original Description".to_string()),
        expires_at: None,
        tags: vec!["original".to_string()],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Update the link
    let update_request = UpdateLinkRequest {
        url: Some("https://example.com/updated".to_string()),
        title: Some("Updated Title".to_string()),
        description: Some("Updated Description".to_string()),
        expires_at: Some(Some(Utc::now() + Duration::days(30))),
        is_active: Some(true),
        tags: Some(vec!["updated".to_string()]),
        is_password_protected: Some(false),
        password: None,
    };

    let updated = service.update_link(&user, created.id, update_request).await;
    assert!(updated.is_ok());

    let updated_link = updated.unwrap();
    assert_eq!(updated_link.title, Some("Updated Title".to_string()));
    assert_eq!(updated_link.original_url, "https://example.com/updated");
}

#[tokio::test]
#[ignore] // Requires database
async fn test_delete_link() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/delete".to_string(),
        custom_alias: None,
        title: Some("To Be Deleted".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Delete (soft delete) the link
    let result = service.delete_link(&user, created.id).await;
    assert!(result.is_ok());

    // Try to retrieve - should fail or be inactive
    let retrieved = service.get_link(&created.short_code).await;
    // Link should either not be found or be inactive
    assert!(retrieved.is_err() || !retrieved.unwrap().is_active);
}

#[tokio::test]
#[ignore] // Requires database
async fn test_list_user_links() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create multiple links
    for i in 0..5 {
        let request = CreateLinkRequest {
            url: format!("https://example.com/link{}", i),
            custom_alias: None,
            title: Some(format!("Link {}", i)),
            description: None,
            expires_at: None,
            tags: vec![format!("tag{}", i)],
            is_password_protected: false,
            password: None,
        };

        service.create_link(&user, request).await.unwrap();
    }

    // List all links
    let filter = LinkFilter {
        search: None,
        tags: None,
        is_active: Some(true),
        has_password: None,
        domain: None,
        created_after: None,
        created_before: None,
    };

    let pagination = LinkPagination {
        page: 1,
        per_page: 10,
    };

    // Combine filter and pagination into ListLinksParams
    let params = ListLinksParams {
        page: Some(pagination.page),
        per_page: Some(pagination.per_page),
        sort_by: None,
        sort_order: None,
        search: filter.search,
        is_active: filter.is_active,
        domain_id: filter.domain_id,
        has_custom_alias: filter.has_custom_alias,
        created_after: filter.created_after,
        created_before: filter.created_before,
    };
    let result = service.get_user_links(&user, params).await;
    assert!(result.is_ok());

    let list_response = result.unwrap();
    assert!(list_response.total >= 5);
    assert!(list_response.links.len() >= 5);
}

#[tokio::test]
#[ignore] // Requires database
async fn test_link_expiry() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create link with expiry
    let request = CreateLinkRequest {
        url: "https://example.com/expiring".to_string(),
        custom_alias: None,
        title: Some("Expiring Link".to_string()),
        description: None,
        expires_at: Some(Utc::now() + Duration::days(7)),
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();
    assert!(created.expires_at.is_some());

    // Verify expiry is set correctly
    let expires = created.expires_at.unwrap();
    let days_until_expiry = (expires - Utc::now()).num_days();
    assert!(days_until_expiry >= 6 && days_until_expiry <= 7);
}

#[tokio::test]
#[ignore] // Requires database
async fn test_link_search() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create links with specific titles
    let request1 = CreateLinkRequest {
        url: "https://example.com/searchable1".to_string(),
        custom_alias: None,
        title: Some("Searchable Link One".to_string()),
        description: None,
        expires_at: None,
        tags: vec!["searchable".to_string()],
        is_password_protected: false,
        password: None,
    };

    let request2 = CreateLinkRequest {
        url: "https://example.com/searchable2".to_string(),
        custom_alias: None,
        title: Some("Another Test Link".to_string()),
        description: None,
        expires_at: None,
        tags: vec!["test".to_string()],
        is_password_protected: false,
        password: None,
    };

    service.create_link(&user, request1).await.unwrap();
    service.create_link(&user, request2).await.unwrap();

    // Search for "Searchable"
    let filter = LinkFilter {
        search: Some("Searchable".to_string()),
        tags: None,
        is_active: Some(true),
        has_password: None,
        domain: None,
        created_after: None,
        created_before: None,
    };

    let pagination = LinkPagination {
        page: 1,
        per_page: 10,
    };

    // Combine filter and pagination into ListLinksParams
    let params = ListLinksParams {
        page: Some(pagination.page),
        per_page: Some(pagination.per_page),
        sort_by: None,
        sort_order: None,
        search: filter.search,
        is_active: filter.is_active,
        domain_id: filter.domain_id,
        has_custom_alias: filter.has_custom_alias,
        created_after: filter.created_after,
        created_before: filter.created_before,
    };
    let result = service.get_user_links(&user, params).await;
    assert!(result.is_ok());

    let list_response = result.unwrap();
    assert!(list_response
        .links
        .iter()
        .any(|l| l.title.as_ref().map_or(false, |t| t.contains("Searchable"))));
}

#[tokio::test]
async fn test_link_caching_performance() {
    use std::time::Instant;

    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/cached".to_string(),
        custom_alias: None,
        title: Some("Cached Link".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // First retrieval (might hit database)
    let start = Instant::now();
    let _ = service.get_link(&created.short_code).await.unwrap();
    let first_duration = start.elapsed();

    // Second retrieval (should hit cache)
    let start = Instant::now();
    let _ = service.get_link(&created.short_code).await.unwrap();
    let cached_duration = start.elapsed();

    println!("First retrieval: {:?}", first_duration);
    println!("Cached retrieval: {:?}", cached_duration);

    // Cached should be significantly faster (at least 2x)
    assert!(cached_duration < first_duration / 2 || cached_duration.as_millis() < 10);
}
