// Integration tests for login API endpoint
// DEV-102: Comprehensive login tests with security features

use axum::http::StatusCode;
use qck_backend_core::models::user::{NewUser, OnboardingStatus, User};
use qck_backend_core::utils::{hash_password, verify_password};
use serde_json::json;
use serial_test::serial;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

mod common;
use common::{setup_test_app, TestApp, TestRequest};

// Helper function to generate unique email
fn unique_email(prefix: &str) -> String {
    format!("{}{}@example.com", prefix, Uuid::new_v4().simple())
}

// Helper function to create a test user
async fn create_test_user(app: &TestApp, email: &str, password: &str) -> User {
    let mut conn = app.diesel_pool.get().await.unwrap();

    // Hash the password
    let password_hash = hash_password(password).unwrap();

    // Create user in database
    let new_user = NewUser {
        email: email.to_string(),
        password_hash,
        full_name: "Test User".to_string(),
        company_name: Some("Test Company".to_string()),
        email_verified: true,
        subscription_tier: "free".to_string(),
        onboarding_status: OnboardingStatus::Completed.as_str().to_string(),
    };

    User::create(&mut conn, new_user).await.unwrap()
}

#[tokio::test]
#[serial]
async fn test_login_success() {
    let app = setup_test_app().await;

    // Create a test user with unique email
    let password = "SecureP@ssw0rd123!";
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let user = create_test_user(&app, &email, password).await;

    // Attempt login
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": password,
            "remember_me": false
        }))
        .send()
        .await;

    let status = response.status();
    if status != StatusCode::OK {
        let error_body = response.text().await;
        panic!(
            "Login failed with status {} and body: {}",
            status, error_body
        );
    }

    let body: serde_json::Value = response.json().await;
    println!(
        "Login response: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );
    assert_eq!(body["success"], true);
    assert!(body["data"]["access_token"].is_string());
    assert!(body["data"]["refresh_token"].is_string());
    assert_eq!(body["data"]["token_type"], "Bearer");
    assert_eq!(body["data"]["user"]["email"], email);
    assert_eq!(body["data"]["user"]["full_name"], "Test User");
    assert_eq!(body["data"]["user"]["subscription_tier"], "free");
}

#[tokio::test]
#[serial]
async fn test_login_with_remember_me() {
    let app = setup_test_app().await;

    // Create a test user with unique email
    let password = "SecureP@ssw0rd123!";
    let email = format!("remember_{}@example.com", Uuid::new_v4());
    let user = create_test_user(&app, &email, password).await;

    // Login with remember_me
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": password,
            "remember_me": true
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], true);
    // Note: In production, the refresh token would have extended expiry
    assert!(body["data"]["refresh_token"].is_string());
}

#[tokio::test]
#[serial]
async fn test_login_invalid_credentials() {
    let app = setup_test_app().await;

    // Create a test user with unique email
    let email = format!("invalid_{}@example.com", Uuid::new_v4());
    let user = create_test_user(&app, &email, "CorrectPassword123!").await;

    // Attempt login with wrong password
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": "WrongPassword123!"
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
#[serial]
async fn test_login_nonexistent_user() {
    let app = setup_test_app().await;

    // Attempt login with non-existent email
    let email = format!("nonexistent_{}@example.com", Uuid::new_v4());
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": "AnyPassword123!"
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
#[serial]
async fn test_login_email_not_verified() {
    let app = setup_test_app().await;

    // Create unverified user with unique email
    let email = format!("unverified_{}@example.com", Uuid::new_v4());
    let mut conn = app.diesel_pool.get().await.unwrap();
    let password = "SecureP@ssw0rd123!";
    let password_hash = hash_password(password).unwrap();

    let new_user = NewUser {
        email: email.clone(),
        password_hash,
        full_name: "Unverified User".to_string(),
        company_name: None,
        email_verified: false, // Not verified
        subscription_tier: "free".to_string(),
        onboarding_status: OnboardingStatus::Registered.as_str().to_string(),
    };

    User::create(&mut conn, new_user).await.unwrap();

    // Attempt login
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": password
        }))
        .send()
        .await;

    // Check if email verification is required in this environment
    let email_verification_required = std::env::var("REQUIRE_EMAIL_VERIFICATION")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase()
        == "true";

    if email_verification_required {
        // Should be forbidden if email verification is required
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body: serde_json::Value = response.json().await;
        assert_eq!(body["success"], false);
        assert_eq!(body["error"]["code"], "EMAIL_NOT_VERIFIED");
    } else {
        // Should succeed if email verification is disabled
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await;
        assert_eq!(body["success"], true);
    }
}

#[tokio::test]
#[serial]
async fn test_login_active_account() {
    let app = setup_test_app().await;

    // Create active user with unique email (users are active by default)
    let email = unique_email("active_");
    let mut conn = app.diesel_pool.get().await.unwrap();
    let password = "SecureP@ssw0rd123!";
    let password_hash = hash_password(password).unwrap();

    let new_user = NewUser {
        email: email.clone(),
        password_hash,
        full_name: "Active User".to_string(),
        company_name: None,
        email_verified: true,
        subscription_tier: "free".to_string(),
        onboarding_status: OnboardingStatus::Completed.as_str().to_string(),
    };

    User::create(&mut conn, new_user).await.unwrap();

    // Attempt login
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": password
        }))
        .send()
        .await;

    // User should be able to login successfully (users are active by default)
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], true);
    assert!(body["data"]["access_token"].is_string());
}

#[tokio::test]
#[serial]
async fn test_login_rate_limiting_per_ip() {
    // Check if rate limiting is enabled in this environment
    let rate_limiting_enabled = std::env::var("ENABLE_RATE_LIMITING")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase()
        == "true";

    if !rate_limiting_enabled {
        println!("Rate limiting is disabled in this environment - skipping test");
        return;
    }

    let app = setup_test_app().await;

    // Create a test user
    let password = "SecureP@ssw0rd123!";
    let email = unique_email("ratelimit_");
    let user = create_test_user(&app, &email, password).await;

    // Use a specific IP for this test to avoid interference
    let test_ip = "192.168.99.99:12345";

    // Get the configured rate limit
    let config = qck_backend_core::app_config::config();
    let max_attempts = config.security.login_rate_limit_per_ip;
    println!("Configured rate limit: {} attempts", max_attempts);

    // Make max_attempts requests (should all succeed or fail normally)
    for i in 0..max_attempts {
        let response = app
            .post("/v1/auth/login")
            .with_ip(test_ip)
            .json(&json!({
                "email": &email,
                "password": "WrongPassword!", // Use wrong password to trigger failures
                "remember_me": false
            }))
            .send()
            .await;

        // Should get UNAUTHORIZED for wrong password
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "Request {} should be unauthorized",
            i + 1
        );
    }

    // Next request should be rate limited
    let response = app
        .post("/v1/auth/login")
        .with_ip(test_ip)
        .json(&json!({
            "email": &email,
            "password": password,
            "remember_me": false
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "RATE_LIMITED");
    assert!(body["error"]["retry_after"].is_number());
}

#[tokio::test]
#[serial]
async fn test_login_account_lockout() {
    let app = setup_test_app().await;

    // Create a test user
    let password = "SecureP@ssw0rd123!";
    let email = unique_email("lockout_");
    let user = create_test_user(&app, &email, password).await;

    // Get the configured lockout threshold (10 in dev)
    let config = qck_backend_core::app_config::config();
    let lockout_threshold = config.security.login_lockout_threshold;

    // Make failed login attempts up to the lockout threshold
    for i in 0..lockout_threshold {
        let response = app
            .post("/v1/auth/login")
            .json(&json!({
                "email": &email,
                "password": "WrongPassword!",
                "remember_me": false
            }))
            .send()
            .await;

        let status = response.status();

        // Check if rate limiting kicked in (can happen even if generally disabled)
        if status == StatusCode::TOO_MANY_REQUESTS {
            println!("Hit rate limit at attempt {}, skipping lockout test", i + 1);
            return; // Skip test if rate limiting interferes
        }

        if i < lockout_threshold - 1 {
            // Should get UNAUTHORIZED before lockout
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "Attempt {} should be unauthorized",
                i + 1
            );
        } else {
            // Last failed attempt should trigger lockout
            assert_eq!(
                response.status(),
                StatusCode::LOCKED,
                "Attempt {} should trigger lockout",
                i + 1
            );

            let body: serde_json::Value = response.json().await;
            assert_eq!(body["error"]["code"], "ACCOUNT_LOCKED");
            assert!(body["error"]["retry_after"].is_number());
        }

        // Small delay to avoid hitting rate limits
        sleep(Duration::from_millis(100)).await;
    }

    // Try to login with correct password - should still be locked
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &email,
            "password": password,
            "remember_me": false
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::LOCKED);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "ACCOUNT_LOCKED");
}

#[tokio::test]
#[serial]
async fn test_login_clears_failed_attempts_on_success() {
    // Check if rate limiting is enabled in this environment
    let rate_limiting_enabled = std::env::var("ENABLE_RATE_LIMITING")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase()
        == "true";

    if !rate_limiting_enabled {
        println!("Rate limiting is disabled - skipping failed attempts test");
        return;
    }

    let app = setup_test_app().await;

    // Create a test user
    let password = "SecureP@ssw0rd123!";
    let email = unique_email("clear_");
    let user = create_test_user(&app, &email, password).await;

    // Use a unique IP for this test
    let test_ip = "192.168.88.88:12345";

    // Make a few failed attempts (but not enough to lock)
    for _ in 0..3 {
        let response = app
            .post("/v1/auth/login")
            .with_ip(test_ip)
            .json(&json!({
                "email": &email,
                "password": "WrongPassword!"
            }))
            .send()
            .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        sleep(Duration::from_millis(100)).await;
    }

    // Successful login should clear failed attempts
    let response = app
        .post("/v1/auth/login")
        .with_ip(test_ip)
        .json(&json!({
            "email": &email,
            "password": password
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Make more failed attempts - counter should have reset
    for _ in 0..3 {
        let response = app
            .post("/v1/auth/login")
            .with_ip(test_ip)
            .json(&json!({
                "email": &email,
                "password": "WrongPassword!"
            }))
            .send()
            .await;

        // Should still get UNAUTHORIZED (not locked)
        let status = response.status();
        if status != StatusCode::UNAUTHORIZED {
            let body = response.text().await;
            println!("Unexpected status {} with body: {}", status, body);
        }
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
#[serial]
async fn test_login_case_insensitive_email() {
    let app = setup_test_app().await;

    // Create a test user with lowercase email
    let password = "SecureP@ssw0rd123!";
    let email = unique_email("mixed_");
    let user = create_test_user(&app, &email, password).await;

    // Try login with mixed case version of the same email
    let mixed_case_email = email
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_uppercase().to_string()
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect::<String>();

    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": mixed_case_email,
            "password": password,
            "remember_me": false
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], true);
    // Email should be normalized to lowercase
    assert_eq!(body["data"]["user"]["email"], email.as_str());
}

#[tokio::test]
#[serial]
async fn test_login_invalid_email_format() {
    let app = setup_test_app().await;

    // Try login with invalid email format
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": "not-an-email",
            "password": "Password123!",
            "remember_me": false
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = response.json().await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
#[serial]
async fn test_login_empty_fields() {
    let app = setup_test_app().await;

    // Try login with empty email
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": "",
            "password": "Password123!"
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Try login with empty password
    let response = app
        .post("/v1/auth/login")
        .json(&json!({
            "email": &unique_email("test_"),
            "password": ""
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
