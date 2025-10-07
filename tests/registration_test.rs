// Integration tests for user registration API
// DEV-101: User Registration API Endpoint tests

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::setup_test_app;

#[tokio::test]
async fn test_successful_registration() {
    let app = setup_test_app().await;

    // Use unique email to avoid conflicts
    let email = format!("newuser_{}@example.com", Uuid::new_v4());

    let registration_data = json!({
        "email": email.clone(),
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "New User",
        "company_name": "New Company",
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    let status = response.status();
    if status != StatusCode::CREATED {
        let error_body = response.text().await;
        eprintln!("Registration failed:");
        eprintln!("  Status: {}", status);
        eprintln!("  Body: {}", error_body);
        panic!("Expected CREATED (201), got {}", status);
    }

    let body: serde_json::Value = response.json().await;
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"]["user_id"].is_string());
    assert_eq!(body["data"]["email"].as_str().unwrap(), email.as_str());
    assert_eq!(body["data"]["full_name"].as_str().unwrap(), "New User");
    assert_eq!(
        body["data"]["company_name"].as_str().unwrap(),
        "New Company"
    );
    // OSS version auto-verifies users, no email verification needed
    // Users should be immediately active and verified
}

#[tokio::test]
async fn test_registration_with_existing_email() {
    let app = setup_test_app().await;

    let email = format!("duplicate_{}@example.com", Uuid::new_v4());

    // First registration
    let registration_data = json!({
        "email": email.clone(),
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "Duplicate User",
        "company_name": null,
        "accept_terms": true
    });

    let _ = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    // Second registration with same email
    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["message"].as_str().unwrap().contains("already exists"));
}

#[tokio::test]
async fn test_registration_with_weak_password() {
    let app = setup_test_app().await;

    let registration_data = json!({
        "email": "weakpass@example.com",
        "password": "weak",  // Too short, no uppercase, no special char
        "password_confirmation": "weak",
        "full_name": "Weak Password User",
        "company_name": null,
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["message"].as_str().unwrap().contains("password"));
}

#[tokio::test]
async fn test_registration_password_mismatch() {
    let app = setup_test_app().await;

    let registration_data = json!({
        "email": "mismatch@example.com",
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "DifferentP@ssw0rd456!",
        "full_name": "Mismatch User",
        "company_name": "Test Company",
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert_eq!(body["message"].as_str().unwrap(), "Passwords do not match");
}

#[tokio::test]
async fn test_registration_invalid_email() {
    let app = setup_test_app().await;

    let registration_data = json!({
        "email": "not-an-email",
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "Invalid Email User",
        "company_name": null,
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["message"].as_str().unwrap().contains("email"));
}

#[tokio::test]
async fn test_registration_terms_not_accepted() {
    let app = setup_test_app().await;

    let registration_data = json!({
        "email": "noterms@example.com",
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "No Terms User",
        "company_name": null,
        "accept_terms": false
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("terms and conditions"));
}

#[tokio::test]
async fn test_registration_rate_limiting() {
    let app = setup_test_app().await;

    // Check if rate limiting is enabled in the current environment
    let rate_limiting_enabled = std::env::var("ENABLE_RATE_LIMITING")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase()
        == "true";

    if !rate_limiting_enabled {
        println!("Rate limiting is disabled in this environment - skipping rate limit test");
        return;
    }

    // Use a fixed IP for all requests in this test to properly test rate limiting
    let fixed_ip = "192.168.1.100:54321";

    println!("Starting rate limiting test with IP: {}", fixed_ip);

    // Make 6 registration attempts (limit is 5 per minute)
    for i in 0..6 {
        let registration_data = json!({
            "email": format!("ratelimit_{}_{}_@example.com", Uuid::new_v4(), i),
            "password": "SecureP@ssw0rd123!",
            "password_confirmation": "SecureP@ssw0rd123!",
            "full_name": format!("Rate Limit User {}", i),
            "company_name": null,
            "accept_terms": true
        });

        let response = app
            .post("/v1/auth/register")
            .json(&registration_data)
            .with_ip(fixed_ip)  // Use fixed IP for rate limiting test
            .send()
            .await;

        let status = response.status();
        println!("Request {} status: {}", i + 1, status);

        if i < 5 {
            // First 5 requests should succeed
            if status != StatusCode::CREATED {
                let body = response.text().await;
                panic!(
                    "Request {} failed with status {} and body: {}",
                    i + 1,
                    status,
                    body
                );
            }
        } else {
            // 6th request should be rate limited
            assert_eq!(
                status,
                StatusCode::TOO_MANY_REQUESTS,
                "Request 6 should be rate limited"
            );

            let body: serde_json::Value = response.json().await;
            assert!(!body["success"].as_bool().unwrap());
            assert!(body["message"]
                .as_str()
                .unwrap()
                .contains("Too many registration attempts"));
        }
    }
}

#[tokio::test]
async fn test_registration_empty_full_name() {
    let app = setup_test_app().await;

    let email = format!("emptyname_{}@example.com", Uuid::new_v4());

    let registration_data = json!({
        "email": email,
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "   ",  // Only whitespace
        "company_name": null,
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Full name cannot be empty"));
}

#[tokio::test]
async fn test_registration_optional_company_name() {
    let app = setup_test_app().await;

    let email = format!("nocompany_{}@example.com", Uuid::new_v4());

    // Test with no company_name field at all
    let registration_data = json!({
        "email": email.clone(),
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "User Without Company",
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: serde_json::Value = response.json().await;
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"]["company_name"].is_null());
}

#[tokio::test]
async fn test_registration_email_case_insensitive() {
    let app = setup_test_app().await;

    let base_email = format!("casetest_{}@example.com", Uuid::new_v4());

    // Register with lowercase email
    let registration_data = json!({
        "email": base_email.clone(),
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "Case Test User",
        "company_name": "Case Test Company",
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Try to register with uppercase version of same email
    let registration_data_upper = json!({
        "email": base_email.to_uppercase(),
        "password": "SecureP@ssw0rd123!",
        "password_confirmation": "SecureP@ssw0rd123!",
        "full_name": "Upper Case User",
        "company_name": null,
        "accept_terms": true
    });

    let response = app
        .post("/v1/auth/register")
        .json(&registration_data_upper)
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body: serde_json::Value = response.json().await;
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["message"].as_str().unwrap().contains("already exists"));
}
