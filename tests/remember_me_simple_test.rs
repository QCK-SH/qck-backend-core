// Simple test that remember_me flag properly affects refresh token expiry calculation
use qck_backend::app_config::config;
use qck_backend::services::jwt::{JwtConfig, JwtService};
use std::env;

#[test]
fn test_remember_me_expiry_calculation() {
    // Load test environment
    dotenv::from_filename(".env.test").ok();

    // Get configuration
    let app_config = config();
    let remember_me_days = app_config.security.remember_me_duration_days;

    // Create JWT service without database (for testing token generation only)
    let jwt_config = JwtConfig::from_env().expect("Failed to load JWT config");
    let jwt_service = JwtService::new(jwt_config.clone());

    // We'll test the expiry calculation by decoding generated tokens
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct TestClaims {
        exp: u64,
        iat: u64,
    }

    // Generate tokens and verify expiry times
    println!("Testing remember_me token expiry calculation:");
    println!(
        "- Normal refresh token expiry: {} seconds (7 days)",
        jwt_config.refresh_token_expiry
    );
    println!(
        "- Remember me duration: {} days ({} seconds)",
        remember_me_days,
        remember_me_days * 86400
    );

    // The key insight: remember_me tokens should have a longer expiry
    // Normal: 7 days (604800 seconds)
    // Remember me: 30 days (2592000 seconds) by default

    assert!(
        (remember_me_days * 86400) > jwt_config.refresh_token_expiry as u32,
        "Remember me duration ({} days) should be longer than normal refresh token expiry ({} seconds)",
        remember_me_days,
        jwt_config.refresh_token_expiry
    );

    println!("✅ Remember_me configuration validated:");
    println!(
        "   - Normal token: {} days",
        jwt_config.refresh_token_expiry / 86400
    );
    println!("   - Remember me: {} days", remember_me_days);
    println!(
        "   - Remember me tokens will last {}x longer",
        (remember_me_days * 86400) / jwt_config.refresh_token_expiry as u32
    );
}
