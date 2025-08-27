use qck_backend::handlers::auth::{
    LoginRequest, RegisterRequest, ResendVerificationRequest, VerifyEmailRequest,
};
use reqwest::StatusCode;
use serde_json::json;

mod common;
use common::TestSetup;

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_complete_email_verification_flow_with_login_states() {
    let setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    // Generate unique email for this test
    let test_email = format!("test_complete_{}@example.com", uuid::Uuid::new_v4());

    println!("\n=== COMPLETE EMAIL VERIFICATION FLOW TEST ===");
    println!("Testing with email: {}", test_email);

    // Step 1: Register a new user
    println!("\n1. Registering new user...");
    let register_request = RegisterRequest {
        email: test_email.clone(),
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
    println!(
        "Registration response: {}",
        serde_json::to_string_pretty(&register_response).unwrap()
    );

    assert!(register_response["success"].as_bool().unwrap());
    assert!(register_response["data"]["email_verification_required"]
        .as_bool()
        .unwrap_or(false));

    // Step 2: Test login with unverified email (when verification is NOT required)
    println!("\n2. Testing login with unverified email (verification not required)...");
    let login_request = LoginRequest {
        email: test_email.clone(),
        password: "Test@1234".to_string(),
        remember_me: false,
    };

    let response = client
        .post(format!("{}/auth/login", base_url))
        .json(&login_request)
        .send()
        .await
        .unwrap();

    // Should succeed if REQUIRE_EMAIL_VERIFICATION=false
    let status = response.status();
    let login_response: serde_json::Value = response.json().await.unwrap();
    println!(
        "Login response (unverified): {}",
        serde_json::to_string_pretty(&login_response).unwrap()
    );

    if status == StatusCode::OK {
        println!("✓ Login succeeded (email verification not required)");
        assert!(login_response["success"].as_bool().unwrap());
        assert!(login_response["data"]["access_token"].as_str().is_some());

        // Check user info
        let user_info = &login_response["data"]["user"];
        assert_eq!(user_info["email"].as_str().unwrap(), test_email);
        assert_eq!(user_info["subscription_tier"].as_str().unwrap(), "pending");
        assert_eq!(
            user_info["onboarding_status"].as_str().unwrap(),
            "registered"
        );
    } else {
        println!("✓ Login blocked (email verification required)");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!login_response["success"].as_bool().unwrap());
        assert_eq!(
            login_response["message"].as_str().unwrap(),
            "Please verify your email address"
        );
    }

    // Step 3: Test resend verification
    println!("\n3. Testing resend verification...");
    let resend_request = ResendVerificationRequest {
        email: test_email.clone(),
    };

    let response = client
        .post(format!("{}/auth/resend-verification", base_url))
        .json(&resend_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let resend_response: serde_json::Value = response.json().await.unwrap();
    println!(
        "Resend response: {}",
        serde_json::to_string_pretty(&resend_response).unwrap()
    );

    assert!(resend_response["success"].as_bool().unwrap());
    assert_eq!(
        resend_response["data"]["email"].as_str().unwrap(),
        test_email
    );

    // Step 4: Test rate limiting on resend (60-second cooldown)
    println!("\n4. Testing resend rate limiting...");
    let response = client
        .post(format!("{}/auth/resend-verification", base_url))
        .json(&resend_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let error_response: serde_json::Value = response.json().await.unwrap();
    println!(
        "Rate limit response: {}",
        serde_json::to_string_pretty(&error_response).unwrap()
    );

    // Step 5: Test verify with invalid code
    println!("\n5. Testing verification with invalid code...");
    let verify_request = VerifyEmailRequest {
        email: test_email.clone(),
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
    println!(
        "Invalid code response: {}",
        serde_json::to_string_pretty(&error_response).unwrap()
    );

    assert!(!error_response["success"].as_bool().unwrap());
    assert_eq!(
        error_response["message"].as_str().unwrap(),
        "Invalid verification code"
    );

    // Step 6: Test too many invalid attempts
    println!("\n6. Testing too many invalid verification attempts...");
    for i in 0..5 {
        println!("  Attempt {}/5", i + 1);
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
    let error_response: serde_json::Value = response.json().await.unwrap();
    println!(
        "Too many attempts response: {}",
        serde_json::to_string_pretty(&error_response).unwrap()
    );

    println!("\n=== TEST COMPLETED SUCCESSFULLY ===");
}

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_login_response_formats() {
    let setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    println!("\n=== LOGIN RESPONSE FORMAT TEST ===");

    // Register and test different scenarios
    let test_email = format!("response_test_{}@example.com", uuid::Uuid::new_v4());

    // Register user
    let register_request = json!({
        "email": test_email,
        "password": "Test@1234",
        "password_confirmation": "Test@1234",
        "full_name": "Response Test User",
        "company_name": "Test Corp",
        "accept_terms": true
    });

    let response = client
        .post(format!("{}/auth/register", base_url))
        .json(&register_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test login response structure
    let login_request = json!({
        "email": test_email,
        "password": "Test@1234",
        "remember_me": false
    });

    let response = client
        .post(format!("{}/auth/login", base_url))
        .json(&login_request)
        .send()
        .await
        .unwrap();

    let status = response.status();
    let login_response: serde_json::Value = response.json().await.unwrap();

    println!("\nLogin Response Structure:");
    println!("{}", serde_json::to_string_pretty(&login_response).unwrap());

    // Verify response structure based on status
    if status == StatusCode::OK {
        println!("\n✓ Successful login response validated:");
        assert!(login_response["success"].as_bool().unwrap());
        assert!(login_response["data"]["access_token"].as_str().is_some());
        assert!(login_response["data"]["refresh_token"].as_str().is_some());
        assert!(login_response["data"]["expires_in"].as_i64().is_some());
        assert_eq!(
            login_response["data"]["token_type"].as_str().unwrap(),
            "Bearer"
        );

        // Validate user info
        let user = &login_response["data"]["user"];
        assert!(user["id"].as_str().is_some());
        assert_eq!(user["email"].as_str().unwrap(), test_email);
        assert_eq!(user["full_name"].as_str().unwrap(), "Response Test User");
        assert_eq!(user["subscription_tier"].as_str().unwrap(), "pending");
        assert_eq!(user["onboarding_status"].as_str().unwrap(), "registered");

        println!("  - Access token present: ✓");
        println!("  - Refresh token present: ✓");
        println!("  - User info complete: ✓");
        println!(
            "  - Subscription tier: {}",
            user["subscription_tier"].as_str().unwrap()
        );
        println!(
            "  - Onboarding status: {}",
            user["onboarding_status"].as_str().unwrap()
        );
    } else if status == StatusCode::FORBIDDEN {
        println!("\n✓ Email verification required response validated:");
        assert!(!login_response["success"].as_bool().unwrap());
        assert_eq!(
            login_response["message"].as_str().unwrap(),
            "Please verify your email address"
        );
        assert!(login_response["data"]["email_verification_required"]
            .as_bool()
            .unwrap());
        assert_eq!(
            login_response["data"]["email"].as_str().unwrap(),
            test_email
        );

        println!("  - Verification required flag: ✓");
        println!("  - User email returned: ✓");
        println!("  - Appropriate error message: ✓");
    }

    println!("\n=== TEST COMPLETED SUCCESSFULLY ===");
}

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_verify_email_auto_login() {
    let setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    println!("\n=== EMAIL VERIFICATION AUTO-LOGIN TEST ===");

    // This test would require a real verification code
    // In production testing, we'd need to either:
    // 1. Mock the email service to capture codes
    // 2. Have a test endpoint that returns codes
    // 3. Query the database directly for codes

    // For now, we'll test the structure when verification fails
    let test_email = format!("auto_login_test_{}@example.com", uuid::Uuid::new_v4());

    // Register user first
    let register_request = json!({
        "email": test_email,
        "password": "Test@1234",
        "password_confirmation": "Test@1234",
        "full_name": "Auto Login Test",
        "accept_terms": true
    });

    let _ = client
        .post(format!("{}/auth/register", base_url))
        .json(&register_request)
        .send()
        .await
        .unwrap();

    // Test verify endpoint structure (will fail with invalid code)
    let verify_request = json!({
        "email": test_email,
        "code": "123456"
    });

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    println!("\nVerify Email Response:");
    let status = response.status();
    let verify_response: serde_json::Value = response.json().await.unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(&verify_response).unwrap()
    );

    // We expect this to fail with an invalid code
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!verify_response["success"].as_bool().unwrap());
    println!("\n✓ Invalid verification code response validated");

    // Document what a successful verification would return (for reference)
    println!("\nNote: A successful verification would return:");
    println!("  - JWT access token for auto-login");
    println!("  - JWT refresh token");
    println!("  - User information with onboarding_status: 'verified'");
    println!("  - Success message");

    // TODO: In a real integration test with database access, we would:
    // 1. Query the database for the actual verification code
    // 2. Use that code to test successful verification
    // 3. Validate the JWT tokens and auto-login functionality

    println!("\n=== TEST COMPLETED SUCCESSFULLY ===");
}
