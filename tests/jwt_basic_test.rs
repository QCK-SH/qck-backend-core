// Basic JWT tests without database dependencies
// DEV-94: Test token generation and validation

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use qck_backend::{JwtConfig, JwtError, JwtService};

/// Helper to create test JWT config without relying on environment
fn create_test_jwt_config() -> JwtConfig {
    let access_secret = b"test-access-secret-hs256-minimum-32-characters-long";
    let refresh_secret = b"test-refresh-secret-hs256-minimum-32-characters-long";

    JwtConfig {
        access_token_expiry: 3600,    // 1 hour
        refresh_token_expiry: 604800, // 7 days
        algorithm: Algorithm::HS256,
        audience: "test.qck.sh".to_string(),
        issuer: "test.qck.sh".to_string(),
        access_encoding_key: EncodingKey::from_secret(access_secret),
        access_decoding_key: DecodingKey::from_secret(access_secret),
        refresh_encoding_key: EncodingKey::from_secret(refresh_secret),
        refresh_decoding_key: DecodingKey::from_secret(refresh_secret),
        key_version: 1,
    }
}

#[test]
fn test_jwt_access_token_generation_and_validation() {
    let jwt_config = create_test_jwt_config();
    let jwt_service = JwtService::new(jwt_config);

    let user_id = "test-user-123";
    let email = "test@example.com";
    let subscription_tier = "premium";
    let scope = vec!["read".to_string(), "write".to_string()];

    // Generate access token
    let access_token = jwt_service
        .generate_access_token(user_id, email, subscription_tier, scope.clone())
        .expect("Failed to generate access token");

    // Validate the token
    let claims = jwt_service
        .validate_access_token(&access_token)
        .expect("Failed to validate access token");

    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.email, email);
    assert_eq!(claims.tier, subscription_tier);
    assert_eq!(claims.scope, scope);
    assert_eq!(claims.aud, "test.qck.sh");
    assert_eq!(claims.iss, "test.qck.sh");
}

#[tokio::test]
async fn test_jwt_token_expiry_validation() {
    // First test: Generate a token that will expire quickly
    let access_secret = b"test-access-secret-hs256-minimum-32-characters-long";
    let refresh_secret = b"test-refresh-secret-hs256-minimum-32-characters-long";

    let jwt_config = JwtConfig {
        access_token_expiry: 1, // 1 second expiry
        refresh_token_expiry: 604800,
        algorithm: Algorithm::HS256,
        audience: "test.qck.sh".to_string(),
        issuer: "test.qck.sh".to_string(),
        access_encoding_key: EncodingKey::from_secret(access_secret),
        access_decoding_key: DecodingKey::from_secret(access_secret),
        refresh_encoding_key: EncodingKey::from_secret(refresh_secret),
        refresh_decoding_key: DecodingKey::from_secret(refresh_secret),
        key_version: 1,
    };
    let jwt_service = JwtService::new(jwt_config);

    let user_id = "test-user-456";
    let email = "expired@example.com";
    let subscription_tier = "basic";
    let scope = vec!["read".to_string()];

    // Generate token with 1 second expiry
    let expired_token = jwt_service
        .generate_access_token(user_id, email, subscription_tier, scope.clone())
        .expect("Failed to generate token");

    // Verify token is initially valid
    let initial_result = jwt_service.validate_access_token(&expired_token);
    assert!(initial_result.is_ok(), "Token should be valid initially");

    // Wait for token to expire (add buffer for potential clock skew)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await; // Increased wait time

    // Validation should now fail
    let result = jwt_service.validate_access_token(&expired_token);

    match result {
        Err(qck_backend::JwtError::EncodingError(e)) => {
            // JWT library returns EncodingError for expired tokens
            // Check that the error message contains "ExpiredSignature"
            let error_str = format!("{:?}", e);
            assert!(
                error_str.contains("ExpiredSignature")
                    || error_str.contains("Expired")
                    || error_str.contains("expired"),
                "Expected expired token error, got: {}",
                error_str
            );
        },
        Err(JwtError::TokenExpired) => {
            // This is the expected error for expired tokens
            println!("✓ Got expected TokenExpired error");
        },
        Err(e) => panic!(
            "Expected TokenExpired error for expired token, got: {:?}",
            e
        ),
        Ok(claims) => {
            // Debug: print the claims to see expiry time
            println!("Claims: {:?}", claims);
            println!(
                "Current time: {}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
            panic!("Expected token to be expired, but validation succeeded");
        },
    }
}

#[test]
fn test_jwt_invalid_token_validation() {
    let jwt_config = create_test_jwt_config();
    let jwt_service = JwtService::new(jwt_config);

    // Try to validate an invalid token
    let invalid_token = "invalid.jwt.token";
    let result = jwt_service.validate_access_token(invalid_token);

    assert!(result.is_err(), "Invalid token should fail validation");

    if let Err(e) = result {
        match e {
            qck_backend::JwtError::EncodingError(_) => {
                // JWT library returns EncodingError for invalid tokens
                // This is expected behavior
            },
            _ => panic!("Expected EncodingError for invalid token, got: {:?}", e),
        }
    }
}

#[test]
fn test_jwt_audience_validation() {
    let jwt_config = create_test_jwt_config();
    let jwt_service = JwtService::new(jwt_config);

    let user_id = "test-user-789";
    let email = "audience@example.com";
    let subscription_tier = "pro";
    let scope = vec!["admin".to_string()];

    // Generate token with correct audience
    let token = jwt_service
        .generate_access_token(user_id, email, subscription_tier, scope)
        .expect("Failed to generate token");

    // Token should validate successfully
    let claims = jwt_service
        .validate_access_token(&token)
        .expect("Token with correct audience should validate");

    assert_eq!(claims.aud, "test.qck.sh");
}

#[test]
fn test_jwt_different_scopes() {
    let jwt_config = create_test_jwt_config();
    let jwt_service = JwtService::new(jwt_config);

    // Test with different scope configurations
    let test_cases = vec![
        (vec![], "Empty scope"),
        (vec!["read".to_string()], "Read only"),
        (
            vec!["read".to_string(), "write".to_string()],
            "Read and write",
        ),
        (
            vec![
                "read".to_string(),
                "write".to_string(),
                "delete".to_string(),
                "admin".to_string(),
            ],
            "Full permissions",
        ),
    ];

    for (scope, description) in test_cases {
        let token = jwt_service
            .generate_access_token("user", "test@example.com", "tier", scope.clone())
            .expect(&format!("Failed to generate token for: {}", description));

        let claims = jwt_service
            .validate_access_token(&token)
            .expect(&format!("Failed to validate token for: {}", description));

        assert_eq!(claims.scope, scope, "Scope mismatch for: {}", description);
    }
}

#[test]
fn test_jwt_config_creation() {
    // Test the config creation directly
    let config = create_test_jwt_config();

    // Verify the config values
    assert_eq!(config.access_token_expiry, 3600);
    assert_eq!(config.refresh_token_expiry, 604800);
    assert_eq!(config.audience, "test.qck.sh");
    assert_eq!(config.issuer, "test.qck.sh");
    assert_eq!(config.key_version, 1);
    assert_eq!(config.algorithm, jsonwebtoken::Algorithm::HS256);
}
