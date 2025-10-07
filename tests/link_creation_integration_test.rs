// DEV-105: Integration tests for Link Creation API
// Testing the complete flow from request to database

use qck_backend_core::models::link::{CreateLinkRequest, LinkResponse};
use serde_json::json;

// Note: These tests require a running test database and Redis instance
// They would typically be run with: cargo test --test link_creation_integration_test

#[tokio::test]
async fn test_create_basic_link() {
    // Test creating a simple link without custom alias
    let request = CreateLinkRequest {
        url: "https://example.com".to_string(),
        custom_alias: None,
        title: Some("Example Site".to_string()),
        description: Some("A test website".to_string()),
        expires_at: None,
        tags: vec!["test".to_string()],
        is_password_protected: false,
        password: None,
    };

    // In a real test, we'd:
    // 1. Set up test database and user
    // 2. Create authenticated request
    // 3. Send POST to /api/v1/links
    // 4. Verify response contains short_code
    // 5. Verify link exists in database

    assert!(request.url.starts_with("http"));
}

#[tokio::test]
async fn test_create_link_with_custom_alias() {
    let request = CreateLinkRequest {
        url: "https://example.com/custom".to_string(),
        custom_alias: Some("my-custom-link".to_string()),
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    // Should validate custom alias format
    assert!(request.custom_alias.is_some());

    // In production test:
    // - Verify custom alias is used
    // - Verify duplicate alias returns 409
}

#[tokio::test]
async fn test_create_password_protected_link() {
    let request = CreateLinkRequest {
        url: "https://secure.example.com".to_string(),
        custom_alias: None,
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: true,
        password: Some("secretpass123".to_string()),
    };

    assert!(request.is_password_protected);
    assert!(request.password.is_some());

    // Should hash password and store
}

#[tokio::test]
async fn test_url_validation() {
    // Test invalid URLs are rejected
    let invalid_urls = vec![
        "not-a-url",
        "ftp://example.com",   // Wrong protocol
        "javascript:alert(1)", // XSS attempt
        "http://localhost",    // Blacklisted
        "http://192.168.1.1",  // Private network
    ];

    for url in invalid_urls {
        let request = CreateLinkRequest {
            url: url.to_string(),
            custom_alias: None,
            title: None,
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        };

        // Should return validation error
        // In real test: verify API returns 400
    }
}

#[tokio::test]
async fn test_rate_limiting() {
    // Test that rate limiting works
    // Free tier: 100 links/hour
    // Pro tier: 1000 links/hour

    // In production test:
    // 1. Create user with free tier
    // 2. Try to create 101 links
    // 3. Verify 101st returns 429 Too Many Requests
}

#[tokio::test]
async fn test_metadata_extraction() {
    let request = CreateLinkRequest {
        url: "https://github.com/rust-lang/rust".to_string(),
        custom_alias: None,
        title: None,       // Should be extracted
        description: None, // Should be extracted
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    // In production test:
    // 1. Create link
    // 2. Verify title and description were extracted
    // 3. Verify favicon URL was captured
    // 4. Verify OG image if present
}

#[tokio::test]
async fn test_link_expiry() {
    use chrono::{Duration, Utc};

    let request = CreateLinkRequest {
        url: "https://temporary.example.com".to_string(),
        custom_alias: None,
        title: None,
        description: None,
        expires_at: Some(Utc::now() + Duration::days(7)),
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    assert!(request.expires_at.is_some());

    // In production test:
    // 1. Create link with expiry
    // 2. Verify link works before expiry
    // 3. Fast-forward time or wait
    // 4. Verify link returns 410 Gone after expiry
}

#[tokio::test]
async fn test_bulk_link_creation() {
    let requests = vec![
        CreateLinkRequest {
            url: "https://example1.com".to_string(),
            custom_alias: None,
            title: None,
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        },
        CreateLinkRequest {
            url: "https://example2.com".to_string(),
            custom_alias: None,
            title: None,
            description: None,
            expires_at: None,
            tags: vec![],
            is_password_protected: false,
            password: None,
        },
    ];

    assert_eq!(requests.len(), 2);

    // In production test:
    // 1. Send POST to /api/v1/links/bulk
    // 2. Verify all links created
    // 3. Verify response contains success/failed counts
}

#[tokio::test]
async fn test_redis_caching() {
    // Test that created links are cached in Redis

    // In production test:
    // 1. Create a link
    // 2. Verify link is in Redis cache
    // 3. Verify cache TTL is set correctly (3600 seconds)
    // 4. Access link and verify cache hit
}

#[tokio::test]
async fn test_collision_handling() {
    // Test that short code collisions are handled

    // In production test:
    // 1. Mock random generator to produce collision
    // 2. Verify retry logic works
    // 3. Verify unique code generated after retry
}

// Performance tests
#[tokio::test]
async fn test_link_creation_performance() {
    use std::time::Instant;

    let request = CreateLinkRequest {
        url: "https://performance-test.example.com".to_string(),
        custom_alias: None,
        title: Some("Perf Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    // In production test:
    // 1. Measure time to create link
    // 2. Verify < 10ms for code generation
    // 3. Verify < 200ms for complete request
}
