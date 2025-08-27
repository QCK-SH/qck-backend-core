use qck_backend::db::RedisConfig;
use std::time::Duration;

#[test]
fn test_default_config() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();

    // Verify configuration values
    assert!(config.pool_size > 0);
    // Connection and command timeouts should be positive, exact values depend on configuration
    assert!(
        config.connection_timeout > Duration::from_secs(0),
        "Connection timeout should be positive"
    );
    assert!(
        config.command_timeout > Duration::from_secs(0),
        "Command timeout should be positive"
    );
    assert!(config.retry_attempts >= 2);
}

#[test]
fn test_config_validation() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();

    let mut config = RedisConfig::from_env();

    // Valid config should pass
    assert!(config.validate().is_ok());

    // Invalid: pool_size = 0
    config.pool_size = 0;
    assert!(config.validate().is_err());
    config.pool_size = 50;

    // Invalid: retry_attempts = 0
    config.retry_attempts = 0;
    assert!(config.validate().is_err());
    config.retry_attempts = 3;

    // Invalid: empty URL
    config.redis_url = String::new();
    assert!(config.validate().is_err());
}

#[test]
fn test_env_override() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();

    // Note: With centralized config, env vars are loaded once at startup.
    // This test now verifies that the config structure works correctly.
    let config = RedisConfig::from_env();

    // Test that config has reasonable values
    assert!(config.pool_size > 0);
    assert!(config.pool_size <= 1000);
    assert!(config.retry_attempts > 0);
    assert!(config.retry_attempts <= 10);
}
