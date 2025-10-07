// Integration test for rate limiting analytics pipeline
// Tests the complete analytics flow with rate limiting service

use qck_backend_core::{RateLimitConfig, RateLimitEvent, RateLimitService};
use std::collections::HashMap;
use uuid::Uuid;

// Note: Full integration tests with Redis require proper environment setup
// These tests focus on validating the analytics data structures and logic

#[test]
fn test_rate_limit_config_consistency() {
    // Test that all predefined configurations are valid
    let auth_config = RateLimitConfig::auth_endpoint();
    assert!(auth_config.max_requests > 0);
    assert!(auth_config.window_seconds > 0);
    assert!(auth_config.block_duration > 0);

    let link_config = RateLimitConfig::link_creation();
    assert!(link_config.max_requests > auth_config.max_requests); // Links should allow more than auth
    assert!(link_config.burst_limit.is_some());

    let redirect_config = RateLimitConfig::redirect_endpoint();
    assert!(redirect_config.max_requests > link_config.max_requests); // Redirects should allow the most

    let default_config = RateLimitConfig::default_api();
    assert!(default_config.max_requests > 0);
    assert!(default_config.distributed); // Should use distributed by default
}

#[test]
fn test_analytics_event_structure() {
    // Test that analytics events have all required fields
    let event = RateLimitEvent {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        key: "test-key".to_string(),
        endpoint: "/api/test".to_string(),
        blocked: true,
        current_count: 150,
        limit: 100,
        user_tier: Some("free".to_string()),
        client_ip: Some("192.168.1.100".to_string()),
        check_latency_ms: 8,
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("source".to_string(), "integration-test".to_string());
            meta
        },
    };

    // Verify event structure
    assert!(!event.id.is_empty());
    assert_eq!(event.endpoint, "/api/test");
    assert_eq!(event.blocked, true);
    assert_eq!(event.current_count, 150);
    assert_eq!(event.limit, 100);
    assert_eq!(event.user_tier, Some("free".to_string()));
    assert_eq!(event.check_latency_ms, 8);
    assert!(event.metadata.contains_key("source"));
}
