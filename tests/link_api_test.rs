// DEV-105: Link Creation API Tests
// Testing the complete link management flow

use qck_backend_core::models::link::{CreateLinkRequest, LinkMetadata, LinkPagination};
use qck_backend_core::utils::url_validator::{SecurityScanner, UrlValidator};

#[tokio::test]
async fn test_url_validation() {
    // Test valid URLs
    let valid_urls = vec![
        "https://example.com",
        "http://subdomain.example.com/path",
        "https://example.com:8080/path?query=value",
        "https://example.com/path#fragment",
    ];

    for url in valid_urls {
        assert!(
            UrlValidator::validate_url(url).is_ok(),
            "URL should be valid: {}",
            url
        );
    }

    // Test invalid URLs
    let invalid_urls = vec![
        "not-a-url",
        "ftp://example.com",   // Wrong protocol
        "javascript:alert(1)", // XSS attempt
        "http://localhost",    // Blacklisted
        "http://192.168.1.1",  // Private network
    ];

    for url in invalid_urls {
        assert!(
            UrlValidator::validate_url(url).is_err(),
            "URL should be invalid: {}",
            url
        );
    }
}

#[tokio::test]
async fn test_security_scanner() {
    // Test safe URLs
    let safe_result = SecurityScanner::scan_url("https://example.com").await;
    assert!(safe_result.is_safe);
    assert_eq!(
        safe_result.risk_level,
        qck_backend_core::utils::url_validator::RiskLevel::Safe
    );

    // Test blocked URLs
    let blocked_result = SecurityScanner::scan_url("javascript:alert(1)").await;
    assert!(!blocked_result.is_safe);
    assert_eq!(
        blocked_result.risk_level,
        qck_backend_core::utils::url_validator::RiskLevel::Blocked
    );
}

#[test]
fn test_create_link_request_validation() {
    use validator::Validate;

    // Valid request
    let valid_request = CreateLinkRequest {
        url: "https://example.com".to_string(),
        custom_alias: Some("my-link".to_string()),
        title: Some("Example Link".to_string()),
        description: Some("A test link".to_string()),
        expires_at: None,
        tags: vec!["test".to_string()],
        is_password_protected: false,
        password: None,
    };

    assert!(valid_request.validate().is_ok());

    // Invalid URL
    let invalid_url = CreateLinkRequest {
        url: "not-a-url".to_string(),
        custom_alias: None,
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    assert!(invalid_url.validate().is_err());

    // Invalid custom alias (too short)
    let short_alias = CreateLinkRequest {
        url: "https://example.com".to_string(),
        custom_alias: Some("ab".to_string()), // Too short
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    assert!(short_alias.validate().is_err());
}

#[test]
fn test_custom_validation() {
    use chrono::{Duration, Utc};

    // Test password protection without password
    let mut request = CreateLinkRequest {
        url: "https://example.com".to_string(),
        custom_alias: None,
        title: None,
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: true,
        password: None,
    };

    assert!(request.validate_custom().is_err());

    // Add password, should pass
    request.password = Some("secret123".to_string());
    assert!(request.validate_custom().is_ok());

    // Test expired date
    request.expires_at = Some(Utc::now() - Duration::days(1));
    assert!(request.validate_custom().is_err());

    // Future date should pass
    request.expires_at = Some(Utc::now() + Duration::days(7));
    assert!(request.validate_custom().is_ok());

    // Test too many tags
    request.tags = (0..15).map(|i| format!("tag{}", i)).collect();
    assert!(request.validate_custom().is_err());

    // 10 tags should be fine
    request.tags = (0..10).map(|i| format!("tag{}", i)).collect();
    assert!(request.validate_custom().is_ok());
}

#[test]
fn test_link_pagination() {
    let pagination = LinkPagination {
        page: 1,
        per_page: 20,
    };
    assert_eq!(pagination.offset(), 0);
    assert_eq!(pagination.limit(), 20);

    let page2 = LinkPagination {
        page: 2,
        per_page: 20,
    };
    assert_eq!(page2.offset(), 20);
    assert_eq!(page2.limit(), 20);

    let page3 = LinkPagination {
        page: 3,
        per_page: 50,
    };
    assert_eq!(page3.offset(), 100);
    assert_eq!(page3.limit(), 50);

    // Test max limit enforcement
    let large = LinkPagination {
        page: 1,
        per_page: 200,
    };
    assert_eq!(large.limit(), 100); // Should cap at 100
}

#[test]
fn test_link_metadata() {
    let request = CreateLinkRequest {
        url: "https://example.com/article".to_string(),
        custom_alias: None,
        title: Some("My Article".to_string()),
        description: Some("An interesting article".to_string()),
        expires_at: None,
        tags: vec!["tech".to_string(), "news".to_string()],
        is_password_protected: false,
        password: None,
    };

    let metadata = LinkMetadata::from_request(&request, None);
    assert_eq!(metadata.title, Some("My Article".to_string()));
    assert_eq!(
        metadata.description,
        Some("An interesting article".to_string())
    );
    assert_eq!(metadata.tags, vec!["tech", "news"]);
    assert_eq!(metadata.domain, "example.com");
    assert!(metadata.password_hash.is_none());
}

#[test]
fn test_sanitization() {
    let mut request = CreateLinkRequest {
        url: "  https://example.com  ".to_string(),
        custom_alias: Some("  my-link  ".to_string()),
        title: Some("  Title  ".to_string()),
        description: Some("  Description  ".to_string()),
        expires_at: None,
        tags: vec!["  tag1  ".to_string(), "  tag2  ".to_string()],
        is_password_protected: false,
        password: None,
    };

    request.sanitize();

    assert_eq!(request.url, "https://example.com");
    assert_eq!(request.custom_alias, Some("my-link".to_string()));
    assert_eq!(request.title, Some("Title".to_string()));
    assert_eq!(request.description, Some("Description".to_string()));
    assert_eq!(request.tags, vec!["tag1", "tag2"]);
}
