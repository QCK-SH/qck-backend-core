use reqwest::StatusCode;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

mod common;
use common::TestSetup;

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_real_email_verification_with_database() {
    let mut setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    // Generate unique email for this test
    let test_email = format!("real_test_{}@example.com", uuid::Uuid::new_v4());

    println!("\n=== REAL EMAIL VERIFICATION INTEGRATION TEST ===");
    println!("Testing with email: {}", test_email);
    println!("API running on port: {}", setup.api_port);

    // Step 1: Register a new user
    println!("\n1. Registering new user...");
    let register_request = json!({
        "email": test_email,
        "password": "Test@1234",
        "password_confirmation": "Test@1234",
        "full_name": "Real Integration Test User",
        "company_name": "Real Test Company",
        "accept_terms": true
    });

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
    let user_id = register_response["data"]["user_id"].as_str().unwrap();
    println!("Created user with ID: {}", user_id);

    // Step 2: Send verification email
    println!("\n2. Sending verification email...");
    let resend_request = json!({
        "email": test_email
    });

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

    // Step 3: Query Redis for the actual verification code
    println!("\n3. Querying Redis for verification code...");

    // Get Redis connection from the test setup
    let redis_pool = setup.get_redis_pool().await;
    let verification_key = format!("verify:email:{}:code", test_email);

    // Retrieve verification data from Redis
    let verification_data_str: Option<String> = redis_pool
        .get(&verification_key)
        .await
        .expect("Should be able to query Redis");

    assert!(
        verification_data_str.is_some(),
        "Verification code should be stored in Redis"
    );

    let verification_data_str = verification_data_str.unwrap();
    let verification_data: serde_json::Value = serde_json::from_str(&verification_data_str)
        .expect("Should be able to parse verification data from Redis");

    let actual_code = verification_data["code"]
        .as_str()
        .expect("Code should be present in verification data")
        .to_string();

    println!("Retrieved verification code from Redis: {}", actual_code);
    println!(
        "Full verification data: {}",
        serde_json::to_string_pretty(&verification_data).unwrap()
    );

    // Step 4: Verify email with the actual code from Redis
    println!("\n4. Verifying email with real code...");
    let verify_request = json!({
        "email": test_email,
        "code": actual_code
    });

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    println!("Verification response status: {}", response.status());

    // Should succeed with real code
    assert_eq!(response.status(), StatusCode::OK);

    let verify_response: serde_json::Value = response.json().await.unwrap();
    println!(
        "Verification response: {}",
        serde_json::to_string_pretty(&verify_response).unwrap()
    );

    assert!(verify_response["success"].as_bool().unwrap());

    // Validate the response contains JWT tokens for auto-login
    assert!(verify_response["data"]["access_token"].as_str().is_some());
    assert!(verify_response["data"]["refresh_token"].as_str().is_some());
    assert_eq!(
        verify_response["data"]["token_type"].as_str().unwrap(),
        "Bearer"
    );

    // Validate user information
    let user_info = &verify_response["data"]["user"];
    assert_eq!(user_info["email"].as_str().unwrap(), test_email);
    assert_eq!(user_info["onboarding_status"].as_str().unwrap(), "verified");

    println!("✓ Email verification successful with JWT auto-login!");

    // Step 5: Verify Redis state after verification
    println!("\n5. Checking Redis state after verification...");

    // Check if verification code was cleared from Redis after successful verification
    let code_after_verification: Option<String> = redis_pool
        .get(&verification_key)
        .await
        .expect("Should be able to query Redis");

    println!(
        "Code in Redis after verification: {:?}",
        code_after_verification
    );

    // After successful verification, code should be cleared from Redis
    assert!(
        code_after_verification.is_none(),
        "Verification code should be cleared from Redis after successful verification"
    );

    // Check user's email verification status in database
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use qck_backend::schema::users::dsl::*;

    let mut conn = setup.get_database_connection().await;
    let user_verified: (bool, Option<chrono::DateTime<chrono::Utc>>) = users
        .filter(email.eq(&test_email))
        .select((email_verified, email_verified_at))
        .first(&mut conn)
        .await
        .expect("User should still exist in database");

    let (is_verified, verified_at) = user_verified;
    println!("User email_verified: {}", is_verified);
    println!("User email_verified_at: {:?}", verified_at);

    assert!(is_verified, "User should be marked as email verified");
    assert!(
        verified_at.is_some(),
        "Email verified timestamp should be set"
    );

    println!("✓ Redis and database state correctly updated after verification!");

    // Step 6: Test that the same code cannot be used again
    println!("\n6. Testing code reuse prevention...");
    let reuse_request = json!({
        "email": test_email,
        "code": actual_code
    });

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&reuse_request)
        .send()
        .await
        .unwrap();

    // Should fail since code was already used
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let reuse_response: serde_json::Value = response.json().await.unwrap();
    assert!(!reuse_response["success"].as_bool().unwrap());

    println!("✓ Code reuse properly prevented!");

    // Step 7: Test that already verified email cannot be verified again
    println!("\n7. Testing resend to already verified email...");
    let resend_verified_request = json!({
        "email": test_email
    });

    let response = client
        .post(format!("{}/auth/resend-verification", base_url))
        .json(&resend_verified_request)
        .send()
        .await
        .unwrap();

    // Should fail since email is already verified
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let resend_verified_response: serde_json::Value = response.json().await.unwrap();
    assert!(!resend_verified_response["success"].as_bool().unwrap());
    assert_eq!(
        resend_verified_response["message"].as_str().unwrap(),
        "Email is already verified"
    );

    println!("✓ Resend to verified email properly blocked!");

    println!("\n=== REAL INTEGRATION TEST COMPLETED SUCCESSFULLY ===");
}

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_verification_code_expiry() {
    let mut setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    // Generate unique email for this test
    let test_email = format!("expiry_test_{}@example.com", uuid::Uuid::new_v4());

    println!("\n=== VERIFICATION CODE EXPIRY TEST ===");
    println!("Testing with email: {}", test_email);

    // Step 1: Register user
    let register_request = json!({
        "email": test_email,
        "password": "Test@1234",
        "password_confirmation": "Test@1234",
        "full_name": "Expiry Test User",
        "accept_terms": true
    });

    let response = client
        .post(format!("{}/auth/register", base_url))
        .json(&register_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Step 2: Get verification code from Redis
    let redis_pool = setup.get_redis_pool().await;
    let verification_key = format!("verify:email:{}:code", test_email);

    let verification_data_str: String = redis_pool
        .get(&verification_key)
        .await
        .expect("Should be able to query Redis")
        .expect("Verification code should exist in Redis");

    let verification_data: serde_json::Value = serde_json::from_str(&verification_data_str)
        .expect("Should be able to parse verification data from Redis");

    let verification_code = verification_data["code"]
        .as_str()
        .expect("Code should be present in verification data")
        .to_string();

    println!("Retrieved verification code: {}", verification_code);

    // Step 3: Manually expire the code by updating the created_at timestamp in Redis
    println!("\n2. Manually expiring the verification code...");

    // Create expired verification data (set created_at to 1 hour ago)
    let expired_timestamp = chrono::Utc::now().timestamp() - 3600; // 1 hour ago
    let mut expired_data = verification_data.as_object().unwrap().clone();
    expired_data.insert(
        "created_at".to_string(),
        serde_json::Value::Number(serde_json::Number::from(expired_timestamp)),
    );

    let expired_data_str =
        serde_json::to_string(&expired_data).expect("Should be able to serialize expired data");

    // Update Redis with expired data (use a short TTL since it should expire quickly)
    redis_pool.set_with_expiry(&verification_key, expired_data_str, 300) // 5 minutes TTL
        .await
        .expect("Should be able to update Redis");

    println!("Set verification code to expire (created 1 hour ago)");

    // Step 4: Try to verify with expired code
    println!("\n3. Attempting to verify with expired code...");
    let verify_request = json!({
        "email": test_email,
        "code": verification_code
    });

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    // Should fail due to expiry
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let verify_response: serde_json::Value = response.json().await.unwrap();
    assert!(!verify_response["success"].as_bool().unwrap());

    println!(
        "Expired code response: {}",
        serde_json::to_string_pretty(&verify_response).unwrap()
    );

    // The error message should indicate expiry or invalid code
    let error_msg = verify_response["message"].as_str().unwrap();
    assert!(
        error_msg.contains("expired")
            || error_msg.contains("Invalid")
            || error_msg.contains("Verification code expired"),
        "Error message should mention expiry or invalid code, got: {}",
        error_msg
    );

    println!("✓ Expired verification code properly rejected!");

    println!("\n=== EXPIRY TEST COMPLETED SUCCESSFULLY ===");
}

#[tokio::test]
#[ignore = "Requires running API server on localhost:10110"]
async fn test_verification_rate_limiting() {
    let mut setup = TestSetup::new().await;
    let client = reqwest::Client::new();
    let base_url = format!("http://localhost:{}/v1", setup.api_port);

    // Generate unique email for this test
    let test_email = format!("rate_limit_test_{}@example.com", uuid::Uuid::new_v4());

    println!("\n=== VERIFICATION RATE LIMITING TEST ===");
    println!("Testing with email: {}", test_email);

    // Step 1: Register user
    let register_request = json!({
        "email": test_email,
        "password": "Test@1234",
        "password_confirmation": "Test@1234",
        "full_name": "Rate Limit Test User",
        "accept_terms": true
    });

    let response = client
        .post(format!("{}/auth/register", base_url))
        .json(&register_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Step 2: Make multiple invalid verification attempts
    println!("\n2. Making multiple invalid verification attempts...");

    let invalid_verify_request = json!({
        "email": test_email,
        "code": "000000" // Invalid code
    });

    let max_attempts = 5; // Based on EMAIL_VERIFICATION_MAX_ATTEMPTS in .env.dev

    for attempt in 1..=max_attempts {
        println!("  Attempt {}/{}", attempt, max_attempts);

        let response = client
            .post(format!("{}/auth/verify-email", base_url))
            .json(&invalid_verify_request)
            .send()
            .await
            .unwrap();

        if attempt < max_attempts {
            // Should still allow attempts
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let error_response: serde_json::Value = response.json().await.unwrap();
            assert_eq!(
                error_response["message"].as_str().unwrap(),
                "Invalid verification code"
            );
        } else {
            // Last attempt should still be BAD_REQUEST but might have different message
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // Small delay between attempts
        sleep(Duration::from_millis(100)).await;
    }

    // Step 3: Next attempt should be rate limited
    println!("\n3. Testing rate limiting after max attempts...");

    let response = client
        .post(format!("{}/auth/verify-email", base_url))
        .json(&invalid_verify_request)
        .send()
        .await
        .unwrap();

    // Should be rate limited
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let rate_limit_response: serde_json::Value = response.json().await.unwrap();
    assert!(!rate_limit_response["success"].as_bool().unwrap());

    println!(
        "Rate limit response: {}",
        serde_json::to_string_pretty(&rate_limit_response).unwrap()
    );

    println!(
        "✓ Rate limiting working correctly after {} attempts!",
        max_attempts
    );

    println!("\n=== RATE LIMITING TEST COMPLETED SUCCESSFULLY ===");
}
