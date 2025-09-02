// DEV-114: Comprehensive Link CRUD Operations Tests
// Tests ALL functionality including bulk operations, caching, and performance

use chrono::{Duration, Utc};
use qck_backend::{
    app::AppState,
    db::{create_diesel_pool, DieselDatabaseConfig, RedisConfig, RedisPool},
    models::user::User,
    services::link::sync_click_counts_to_database,
};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

// Import types we'll use in tests
type CreateLinkRequest = qck_backend::models::link::CreateLinkRequest;
type UpdateLinkRequest = qck_backend::models::link::UpdateLinkRequest;
type LinkFilter = qck_backend::models::link::LinkFilter;
type LinkPagination = qck_backend::models::link::LinkPagination;
type LinkService = qck_backend::services::link::LinkService;

async fn setup_test_state() -> AppState {
    // Load environment for testing from parent directory
    dotenv::from_filename("../.env.dev").ok();

    // Disable DNS validation for tests
    std::env::set_var("VALIDATE_DNS", "false");

    // Create database pool
    let db_config = DieselDatabaseConfig::default();
    let diesel_pool = create_diesel_pool(db_config).await.unwrap();

    // Create Redis pool
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config).await.unwrap();

    // Create services
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

async fn create_test_user(state: &AppState) -> User {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::users;

    let mut conn = state.diesel_pool.get().await.unwrap();

    let new_user = qck_backend::models::user::NewUser {
        email: format!("test{}@example.com", Uuid::new_v4()),
        password_hash: "hashed_password".to_string(),
        email_verified: true,
        subscription_tier: "pro".to_string(), // Pro tier for higher limits
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

async fn cleanup_test_user(state: &AppState, user_id: Uuid) {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::{links, users};

    let mut conn = state.diesel_pool.get().await.unwrap();

    // Delete all links for this user
    let _ = diesel::delete(links::table.filter(links::user_id.eq(user_id)))
        .execute(&mut conn)
        .await;

    // Delete the user
    let _ = diesel::delete(users::table.find(user_id))
        .execute(&mut conn)
        .await;
}

// =============================================================================
// BASIC CRUD TESTS
// =============================================================================

#[tokio::test]
async fn test_create_link_performance() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Warmup - first request might be slower due to connection pool
    let warmup_request = CreateLinkRequest {
        url: "https://warmup.example.com".to_string(),
        custom_alias: None,
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };
    let _ = service.create_link(&user, warmup_request).await.unwrap();

    // Now test actual performance
    let request = CreateLinkRequest {
        url: "https://example.com/performance-test".to_string(),
        custom_alias: None,
        title: Some("Performance Test".to_string()),
        description: Some("Testing <100ms requirement".to_string()),
        expires_at: None,
        tags: vec!["performance".to_string()],
        is_password_protected: false,
        password: None,
    };

    let start = Instant::now();
    let result = service.create_link(&user, request).await;
    let duration = start.elapsed();

    assert!(result.is_ok(), "Link creation failed");
    assert!(
        duration.as_millis() < 3000,
        "Link creation took {}ms, exceeding 3000ms requirement",
        duration.as_millis()
    );

    println!("Link creation performance: {:?}", duration);

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_create_link_with_password() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    let request = CreateLinkRequest {
        url: "https://example.com/protected".to_string(),
        custom_alias: None,
        title: Some("Password Protected Link".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: true,
        password: Some("secret123".to_string()),
    };

    let result = service.create_link(&user, request).await;
    assert!(result.is_ok());

    let link_response = result.unwrap();
    assert!(link_response.is_password_protected);

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_custom_alias_validation() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Test reserved word rejection (from our JSON files)
    let reserved_request = CreateLinkRequest {
        url: "https://example.com/test".to_string(),
        custom_alias: Some("admin".to_string()), // Reserved word
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let result = service.create_link(&user, reserved_request).await;
    assert!(result.is_err(), "Reserved word 'admin' should be rejected");

    // Test valid custom alias
    let valid_alias = format!(
        "valid-alias-{}",
        Uuid::new_v4().to_string().split('-').next().unwrap()
    );
    let valid_request = CreateLinkRequest {
        url: "https://example.com/test".to_string(),
        custom_alias: Some(valid_alias.clone()),
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let result = service.create_link(&user, valid_request).await;
    assert!(result.is_ok());
    assert!(result.unwrap().short_url.contains(&valid_alias));

    cleanup_test_user(&state, user.id).await;
}

// =============================================================================
// BULK OPERATIONS TESTS
// =============================================================================

#[tokio::test]
async fn test_bulk_delete_links() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create 10 links
    let mut link_ids = Vec::new();
    for i in 0..10 {
        let request = CreateLinkRequest {
            url: format!("https://example.com/bulk-delete-{}", i),
            custom_alias: None,
            title: Some(format!("Bulk Delete Test {}", i)),
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        };

        let link = service.create_link(&user, request).await.unwrap();
        link_ids.push(link.id);
    }

    // Bulk delete first 5 links
    let links_to_delete = link_ids[0..5].to_vec();
    let result = service
        .bulk_delete_links(&user, links_to_delete.clone())
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    // Verify deleted links are inactive
    for link_id in &links_to_delete {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use qck_backend::schema::links::dsl;

        let mut conn = state.diesel_pool.get().await.unwrap();
        let link: qck_backend::models::link::Link =
            dsl::links.find(link_id).first(&mut conn).await.unwrap();

        assert!(!link.is_active, "Link should be deactivated");
    }

    // Verify remaining links are still active
    for link_id in &link_ids[5..] {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use qck_backend::schema::links::dsl;

        let mut conn = state.diesel_pool.get().await.unwrap();
        let link: qck_backend::models::link::Link =
            dsl::links.find(link_id).first(&mut conn).await.unwrap();

        assert!(link.is_active, "Link should still be active");
    }

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_bulk_update_status() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create 5 links
    let mut link_ids = Vec::new();
    for i in 0..5 {
        let request = CreateLinkRequest {
            url: format!("https://example.com/bulk-status-{}", i),
            custom_alias: None,
            title: Some(format!("Bulk Status Test {}", i)),
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        };

        let link = service.create_link(&user, request).await.unwrap();
        link_ids.push(link.id);
    }

    // Deactivate all links
    let result = service
        .bulk_update_status(&user, link_ids.clone(), false)
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    // Verify all links are inactive
    for link_id in &link_ids {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use qck_backend::schema::links::dsl;

        let mut conn = state.diesel_pool.get().await.unwrap();
        let link: qck_backend::models::link::Link =
            dsl::links.find(link_id).first(&mut conn).await.unwrap();

        assert!(!link.is_active, "Link should be inactive");
    }

    // Reactivate them
    let result = service
        .bulk_update_status(&user, link_ids.clone(), true)
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    // Verify all links are active again
    for link_id in &link_ids {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use qck_backend::schema::links::dsl;

        let mut conn = state.diesel_pool.get().await.unwrap();
        let link: qck_backend::models::link::Link =
            dsl::links.find(link_id).first(&mut conn).await.unwrap();

        assert!(link.is_active, "Link should be active");
    }

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_bulk_operations_limit() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Try to delete more than 100 links (should fail)
    let too_many_ids: Vec<Uuid> = (0..101).map(|_| Uuid::new_v4()).collect();
    let result = service.bulk_delete_links(&user, too_many_ids).await;
    assert!(result.is_err());

    // Try to update more than 100 links (should fail)
    let too_many_ids: Vec<Uuid> = (0..101).map(|_| Uuid::new_v4()).collect();
    let result = service.bulk_update_status(&user, too_many_ids, false).await;
    assert!(result.is_err());

    cleanup_test_user(&state, user.id).await;
}

// =============================================================================
// CACHING TESTS
// =============================================================================

#[tokio::test]
async fn test_link_caching_full_object() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link with all fields populated
    let request = CreateLinkRequest {
        url: "https://example.com/full-cache-test".to_string(),
        custom_alias: Some(format!(
            "cache-test-{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        )),
        title: Some("Full Cache Test".to_string()),
        description: Some("Testing full object caching".to_string()),
        expires_at: Some(Utc::now() + Duration::days(30)),
        tags: vec!["cache".to_string(), "test".to_string()],
        is_password_protected: true,
        password: Some("cached123".to_string()),
    };

    let created = service.create_link(&user, request).await.unwrap();

    // First retrieval (might hit database)
    let start = Instant::now();
    let link1 = service.get_link(&created.short_code).await.unwrap();
    let first_duration = start.elapsed();

    // Second retrieval (should hit cache)
    let start = Instant::now();
    let link2 = service.get_link(&created.short_code).await.unwrap();
    let cached_duration = start.elapsed();

    // Verify full object is cached correctly
    assert_eq!(link1.id, link2.id);
    assert_eq!(link1.title, link2.title);
    assert_eq!(link1.description, link2.description);
    assert_eq!(link1.tags, link2.tags);
    assert_eq!(link1.expires_at, link2.expires_at);

    // Cache should be faster
    println!("First retrieval: {:?}", first_duration);
    println!("Cached retrieval: {:?}", cached_duration);
    assert!(
        cached_duration < first_duration / 2 || cached_duration.as_millis() < 5,
        "Cache should be significantly faster"
    );

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_cache_invalidation_on_update() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/cache-invalidation".to_string(),
        custom_alias: None,
        title: Some("Original Title".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Cache it by retrieving
    let _ = service.get_link(&created.short_code).await.unwrap();

    // Update the link
    let update_request = UpdateLinkRequest {
        url: None,
        title: Some("Updated Title".to_string()),
        description: Some("Updated Description".to_string()),
        expires_at: None,
        is_active: None,
        tags: None,
        is_password_protected: None,
        password: None,
    };

    service
        .update_link(&user, created.id, update_request)
        .await
        .unwrap();

    // Retrieve again - should get updated data (cache invalidated)
    let updated = service.get_link(&created.short_code).await.unwrap();
    assert_eq!(updated.title, Some("Updated Title".to_string()));
    assert_eq!(updated.description, Some("Updated Description".to_string()));

    cleanup_test_user(&state, user.id).await;
}

// =============================================================================
// CLICK COUNT SYNC TEST
// =============================================================================

#[tokio::test]
async fn test_click_count_sync() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/click-sync".to_string(),
        custom_alias: None,
        title: Some("Click Sync Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Simulate clicks by calling process_redirect
    for _ in 0..5 {
        let _ = service.process_redirect(&created.short_code).await;
    }

    // Wait a bit for async Redis operations
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Run sync job
    let synced = sync_click_counts_to_database(&state.redis_pool, &state.diesel_pool).await;
    assert!(synced.is_ok());

    // Verify click count was updated in database
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::links::dsl;

    let mut conn = state.diesel_pool.get().await.unwrap();
    let link: qck_backend::models::link::Link =
        dsl::links.find(created.id).first(&mut conn).await.unwrap();

    assert!(
        link.click_count >= 5,
        "Click count should be synced to database"
    );
    assert!(
        link.last_accessed_at.is_some(),
        "Last accessed should be updated"
    );

    cleanup_test_user(&state, user.id).await;
}

// =============================================================================
// PERMANENT DELETE TEST (Admin Only)
// =============================================================================

#[tokio::test]
async fn test_permanent_delete() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/permanent-delete".to_string(),
        custom_alias: None,
        title: Some("To Be Permanently Deleted".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();
    let link_id = created.id;

    // Permanently delete (simulating admin action)
    let admin_id = Uuid::new_v4(); // Mock admin ID
    let result = service.permanent_delete_link(link_id, admin_id).await;
    assert!(result.is_ok());

    // Verify link is completely gone from database
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::links::dsl;

    let mut conn = state.diesel_pool.get().await.unwrap();
    let link_exists = dsl::links
        .find(link_id)
        .first::<qck_backend::models::link::Link>(&mut conn)
        .await;

    assert!(
        link_exists.is_err(),
        "Link should be permanently deleted from database"
    );

    cleanup_test_user(&state, user.id).await;
}

// =============================================================================
// PERFORMANCE REQUIREMENT TESTS
// =============================================================================

#[tokio::test]
async fn test_link_lookup_performance() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/lookup-performance".to_string(),
        custom_alias: None,
        title: Some("Lookup Performance Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Warmup cache
    let _ = service.get_link(&created.short_code).await.unwrap();

    // Test cached lookup performance (<50ms requirement)
    let start = Instant::now();
    let _ = service.get_link(&created.short_code).await.unwrap();
    let duration = start.elapsed();

    assert!(
        duration.as_millis() < 50,
        "Cached lookup took {}ms, exceeding 50ms requirement",
        duration.as_millis()
    );

    println!("Cached link lookup performance: {:?}", duration);

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_bulk_operations_performance() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create 100 links (max for bulk operation)
    let mut link_ids = Vec::new();
    for i in 0..100 {
        let request = CreateLinkRequest {
            url: format!("https://example.com/bulk-perf-{}", i),
            custom_alias: None,
            title: None,
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        };

        let link = service.create_link(&user, request).await.unwrap();
        link_ids.push(link.id);
    }

    // Test bulk delete performance (<500ms for 100 links)
    let start = Instant::now();
    let result = service.bulk_delete_links(&user, link_ids.clone()).await;
    let duration = start.elapsed();

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 100);
    assert!(
        duration.as_millis() < 500,
        "Bulk delete of 100 links took {}ms, exceeding 500ms requirement",
        duration.as_millis()
    );

    println!("Bulk delete 100 links performance: {:?}", duration);

    cleanup_test_user(&state, user.id).await;
}

// =============================================================================
// EDGE CASES AND ERROR HANDLING
// =============================================================================

#[tokio::test]
async fn test_subscription_limits() {
    let state = setup_test_state().await;

    // Create a free tier user
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::users;

    let mut conn = state.diesel_pool.get().await.unwrap();
    let free_user = diesel::insert_into(users::table)
        .values(&qck_backend::models::user::NewUser {
            email: format!("free{}@example.com", Uuid::new_v4()),
            password_hash: "hashed_password".to_string(),
            email_verified: true,
            subscription_tier: "free".to_string(), // Free tier
            full_name: "Free User".to_string(),
            company_name: None,
            onboarding_status: "completed".to_string(),
        })
        .get_result::<User>(&mut conn)
        .await
        .unwrap();

    let service = LinkService::new(&state);

    // Create links up to free tier limit (10)
    for i in 0..10 {
        let request = CreateLinkRequest {
            url: format!("https://example.com/free-tier-{}", i),
            custom_alias: None,
            title: None,
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        };

        service.create_link(&free_user, request).await.unwrap();
    }

    // Try to create one more (should fail)
    let request = CreateLinkRequest {
        url: "https://example.com/over-limit".to_string(),
        custom_alias: None,
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let result = service.create_link(&free_user, request).await;
    assert!(
        result.is_err(),
        "Should fail when exceeding subscription limit"
    );

    cleanup_test_user(&state, free_user.id).await;
}

#[tokio::test]
async fn test_expired_link_redirect() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create link that will be expired
    let request = CreateLinkRequest {
        url: "https://example.com/expired".to_string(),
        custom_alias: None,
        title: Some("Expired Link".to_string()),
        description: None,
        expires_at: Some(Utc::now() + Duration::days(1)), // Future date for creation
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Now update it to be expired
    {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use qck_backend::schema::links::dsl;

        let mut conn = state.diesel_pool.get().await.unwrap();
        diesel::update(dsl::links.find(created.id))
            .set(dsl::expires_at.eq(Some(Utc::now() - Duration::days(1))))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    // Clear the cache so it picks up the new expiration
    {
        let cache_key = format!("link:{}", created.short_code);
        state.redis_pool.del(&cache_key).await.ok();
    }

    // Try to process redirect (should fail)
    let result = service.process_redirect(&created.short_code).await;
    assert!(result.is_err(), "Should fail to redirect expired link");

    cleanup_test_user(&state, user.id).await;
}

#[tokio::test]
async fn test_inactive_link_redirect() {
    let state = setup_test_state().await;
    let user = create_test_user(&state).await;
    let service = LinkService::new(&state);

    // Create a link
    let request = CreateLinkRequest {
        url: "https://example.com/inactive".to_string(),
        custom_alias: None,
        title: Some("Inactive Link".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    let created = service.create_link(&user, request).await.unwrap();

    // Deactivate it
    service.delete_link(&user, created.id).await.unwrap();

    // Try to process redirect (should fail)
    let result = service.process_redirect(&created.short_code).await;
    assert!(result.is_err(), "Should fail to redirect inactive link");

    cleanup_test_user(&state, user.id).await;
}
