use qck_backend::db::RedisConfig;
use std::time::Duration;

#[test]
fn test_default_config() {
    // Load test environment variables
    dotenv::from_filename(".env.test").ok();

    let config = RedisConfig::from_env();

    // Verify configuration values
    assert!(config.pool_size > 0);
    assert_eq!(config.connection_timeout, Duration::from_secs(5));
    assert_eq!(config.command_timeout, Duration::from_secs(5));
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

    // Test that environment variables override defaults
    std::env::set_var("REDIS_POOL_SIZE", "100");
    std::env::set_var("REDIS_RETRY_ATTEMPTS", "5");

    let config = RedisConfig::from_env();

    assert_eq!(config.pool_size, 100);
    assert_eq!(config.retry_attempts, 5);

    // Clean up
    std::env::remove_var("REDIS_POOL_SIZE");
    std::env::remove_var("REDIS_RETRY_ATTEMPTS");
}
