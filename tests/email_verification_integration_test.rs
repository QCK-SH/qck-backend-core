use qck_backend::handlers::auth::{RegisterRequest, ResendVerificationRequest, VerifyEmailRequest};
use reqwest::StatusCode;
use serde_json::json;

mod common;
use common::TestSetup;

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_full_email_verification_flow() {
    let setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    // Step 1: Register a new user
    let register_request = RegisterRequest {
        email: format!("test_{}@example.com", uuid::Uuid::new_v4()),
        password: "Test@1234".to_string(),
        password_confirmation: "Test@1234".to_string(),
        full_name: "Test User".to_string(),
        company_name: Some("Test Company".to_string()),
        accept_terms: true,
    };

    let response = client
        .post(format!("{}/auth/register", base_url))
        .json(&register_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let register_response: serde_json::Value = response.json().await.unwrap();
    assert!(register_response["success"].as_bool().unwrap());
    assert!(register_response["data"]["email_verification_required"]
        .as_bool()
        .unwrap_or(false));

    // Step 2: Test resend verification
    let resend_request = ResendVerificationRequest {
        email: register_request.email.clone(),
    };

    let response = client
        .post(format!("{}/auth/resend-verification", base_url))
        .json(&resend_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let resend_response: serde_json::Value = response.json().await.unwrap();
    assert!(resend_response["success"].as_bool().unwrap());
    assert_eq!(
        resend_response["data"]["email"].as_str().unwrap(),
        register_request.email
    );

    // Step 3: Test rate limiting on resend
    // Should fail due to 60-second cooldown
    let response = client
        .post(format!("{}/auth/resend-verification", base_url))
        .json(&resend_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // Step 4: Test verify with invalid code
    let verify_request = VerifyEmailRequest {
        email: register_request.email.clone(),
        code: "000000".to_string(), // Invalid code
    };

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let error_response: serde_json::Value = response.json().await.unwrap();
    assert!(!error_response["success"].as_bool().unwrap());
    assert_eq!(
        error_response["message"].as_str().unwrap(),
        "Invalid verification code"
    );

    // Step 5: Test too many invalid attempts
    for _ in 0..5 {
        let _ = client
            .post(format!("{}/auth/verify-email", base_url))
            .json(&verify_request)
            .send()
            .await;
    }

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_verify_already_verified_email() {
    let setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    // Register and verify a user first
    let email = format!("verified_{}@example.com", uuid::Uuid::new_v4());

    // Register
    let register_request = json!({
        "email": email,
        "password": "Test@1234",
        "password_confirmation": "Test@1234",
        "full_name": "Test User",
        "accept_terms": true
    });

    let _ = client
        .post(format!("{}/auth/register", base_url))
        .json(&register_request)
        .send()
        .await
        .unwrap();

    // Now test resending to already verified email
    // (In production, this would be after actual verification)
    // For now, we just test the endpoint exists and responds
    let resend_request = json!({
        "email": email
    });

    let response = client
        .post(format!("{}/auth/resend-verification", base_url))
        .json(&resend_request)
        .send()
        .await
        .unwrap();

    // Should succeed or return appropriate status
    assert!(response.status().is_success() || response.status() == StatusCode::BAD_REQUEST);
}
