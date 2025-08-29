// Centralized configuration management for QCK Backend
// JavaScript-style config pattern - Load ALL env vars ONCE at startup

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingVar(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

/// Global application configuration loaded once at startup
pub static CONFIG: Lazy<AppConfig> = Lazy::new(|| {
    // For tests, load .env file first
    #[cfg(test)]
    dotenv::dotenv().ok();

    AppConfig::from_env().expect("Failed to load configuration")
});

/// Complete application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Server
    pub bind_address: String,
    pub port: u16,
    pub environment: Environment,
    pub rust_log: String,
    pub rust_backtrace: bool,

    // Database
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub database_connect_timeout: u64,
    pub database_idle_timeout: u64,
    pub database_max_lifetime: u64,

    // Redis
    pub redis_url: String,
    pub redis_pool_size: u32,
    pub redis_connection_timeout: u64,
    pub redis_command_timeout: u64,
    pub redis_retry_attempts: u32,
    pub redis_retry_delay_ms: u64,
    pub redis_idle_timeout: u64,
    pub redis_max_lifetime: u64,

    // ClickHouse
    pub clickhouse_url: String,
    pub clickhouse_database: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,

    // JWT
    pub jwt_access_secret: String,
    pub jwt_refresh_secret: String,
    pub jwt_access_expiry: u64,
    pub jwt_refresh_expiry: u64,
    pub jwt_audience: String,
    pub jwt_issuer: String,
    pub jwt_key_version: u32,

    // Security
    pub bcrypt_cost: u32,
    pub rate_limit_per_second: u32,
    pub rate_limit_burst: u32,
    pub rate_limit_analytics_sample_rate: f64,
    pub cors_allowed_origins: Vec<String>,
    pub jti_hash_salt: Option<String>,

    // Application URLs
    pub dashboard_url: String, // Frontend dashboard URL for email links, etc.

    // Features
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub enable_rate_limiting: bool,
    pub enable_swagger_ui: bool,
    pub disable_embedded_migrations: bool,

    // Nested configs for compatibility
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub clickhouse: ClickHouseConfig,
    pub jwt: JwtConfig,
    pub security: SecurityConfig,
    pub email: EmailConfig,
    pub features: FeatureConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub port: u16,
    pub api_port: u16, // External API port for connections (e.g., Docker exposed port)
    pub environment: Environment,
    pub rust_log: String,
    pub rust_backtrace: bool,
}

/// Environment type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Environment {
    Development,
    Test,
    Staging,
    Production,
}

impl From<String> for Environment {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Environment::Development,
            "test" => Environment::Test,
            "staging" | "stage" => Environment::Staging,
            "production" | "prod" => Environment::Production,
            _ => Environment::Development,
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Development => write!(f, "development"),
            Environment::Test => write!(f, "test"),
            Environment::Staging => write!(f, "staging"),
            Environment::Production => write!(f, "production"),
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: u64,
    pub idle_timeout: u64,
    pub max_lifetime: u64,
    pub statement_cache_capacity: usize,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    pub connection_timeout: u64,
    pub command_timeout: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub idle_timeout: u64,
    pub max_lifetime: u64,
}

/// ClickHouse configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickHouseConfig {
    pub url: String,
    pub database: String,
    pub user: String,
    pub password: String,
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub access_secret: String,
    pub refresh_secret: String,
    pub access_expiry: u64,
    pub refresh_expiry: u64,
    pub audience: String,
    pub issuer: String,
    pub key_version: u32,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub bcrypt_cost: u32,
    pub rate_limit_per_second: u32,
    pub rate_limit_burst: u32,
    pub cors_allowed_origins: Vec<String>,

    // Refresh token specific rate limiting
    pub refresh_rate_limit_max_requests: u32,
    pub refresh_rate_limit_window_seconds: u32,
    pub refresh_rate_limit_burst_limit: u32,
    pub refresh_rate_limit_block_duration: u32,

    // Login specific settings (DEV-102)
    pub login_rate_limit_per_ip: u32, // Max login attempts per IP per minute
    pub login_rate_limit_per_email: u32, // Max login attempts per email per hour
    pub login_lockout_threshold: u32, // Failed attempts before lockout
    pub login_lockout_duration_seconds: u32, // Account lockout duration
    pub remember_me_duration_days: u32, // Extended token duration for remember_me
    pub failed_login_expiry_seconds: usize, // Failed login tracking expiry for email
    pub failed_login_ip_expiry_seconds: usize, // Failed login tracking expiry for IP
    pub require_email_verification: bool, // Whether to require email verification for login
}

/// Email configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub provider: EmailProvider,
    pub resend_api_key: String,
    pub resend_api_url: String, // API URL for Resend service (configurable for different environments)
    pub from_email: String,
    pub from_name: String,
    pub support_email: String,          // Support email for help/contact
    pub frontend_url: String, // Frontend URL for email links (e.g., http://localhost:10111, https://app.qck.sh)
    pub dashboard_url: String, // Dashboard URL for email links (backward compatibility)
    pub verification_code_ttl: u64, // TTL in seconds (15 minutes)
    pub verification_max_attempts: u32, // Max attempts per code
    pub resend_limit: u32,    // Max resends per day
    pub resend_window: u64,   // Resend window in seconds (24 hours)
    pub min_resend_cooldown: u64, // Minimum seconds between resend attempts (60 seconds)
}

/// Email provider type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmailProvider {
    Resend,
    Smtp,
    SendGrid,
}

impl From<String> for EmailProvider {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "resend" => EmailProvider::Resend,
            "smtp" => EmailProvider::Smtp,
            "sendgrid" => EmailProvider::SendGrid,
            _ => EmailProvider::Resend,
        }
    }
}

/// Feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub enable_rate_limiting: bool,
    pub enable_swagger_ui: bool,
}

impl AppConfig {
    /// Get refresh token rate limiting configuration
    /// Centralizes refresh token rate limit settings for reuse across handlers
    pub fn get_refresh_rate_limit_config(&self) -> crate::services::rate_limit::RateLimitConfig {
        crate::services::rate_limit::RateLimitConfig {
            max_requests: self.security.refresh_rate_limit_max_requests,
            window_seconds: self.security.refresh_rate_limit_window_seconds,
            burst_limit: Some(self.security.refresh_rate_limit_burst_limit),
            block_duration: self.security.refresh_rate_limit_block_duration,
            distributed: true,
        }
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        // Helper function to get required env var
        let get_required = |key: &str| -> Result<String, ConfigError> {
            env::var(key).map_err(|_| ConfigError::MissingVar(key.to_string()))
        };

        // Helper function to get optional env var with default
        let get_or_default = |key: &str, default: &str| -> String {
            env::var(key).unwrap_or_else(|_| default.to_string())
        };

        // Helper function to parse env var with default
        let parse_or_default = |key: &str, default: &str| -> Result<u32, ConfigError> {
            get_or_default(key, default).parse().map_err(|_| {
                ConfigError::InvalidValue(key.to_string(), "not a valid u32".to_string())
            })
        };

        let parse_u64_or_default = |key: &str, default: &str| -> Result<u64, ConfigError> {
            get_or_default(key, default).parse().map_err(|_| {
                ConfigError::InvalidValue(key.to_string(), "not a valid u64".to_string())
            })
        };

        let parse_bool_or_default = |key: &str, default: &str| -> bool {
            get_or_default(key, default).to_lowercase() == "true"
        };

        // Parse bind address to extract port
        let bind_address = get_or_default("BIND_ADDRESS", "0.0.0.0:8080");
        let port = bind_address
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        // Application URLs - Load once, use everywhere
        let dashboard_url = get_or_default("NEXT_PUBLIC_DASHBOARD_URL", "http://localhost:3000");

        // JWT secrets validation
        let jwt_access_secret = get_required("JWT_ACCESS_SECRET")?;
        if jwt_access_secret.len() < 32 {
            return Err(ConfigError::InvalidValue(
                "JWT_ACCESS_SECRET".to_string(),
                "Secret must be at least 32 characters long".to_string(),
            ));
        }

        let jwt_refresh_secret = get_required("JWT_REFRESH_SECRET")?;
        if jwt_refresh_secret.len() < 32 {
            return Err(ConfigError::InvalidValue(
                "JWT_REFRESH_SECRET".to_string(),
                "Secret must be at least 32 characters long".to_string(),
            ));
        }

        let environment_str = get_or_default("ENVIRONMENT", "development");
        let environment = Environment::from(environment_str.clone());

        // Load all config values
        let database_url = get_required("DATABASE_URL")?;
        let database_max_connections = parse_or_default("DATABASE_MAX_CONNECTIONS", "100")?;
        let database_min_connections = parse_or_default("DATABASE_MIN_CONNECTIONS", "10")?;
        let database_connect_timeout = parse_u64_or_default("DATABASE_CONNECT_TIMEOUT", "30")?;
        let database_idle_timeout = parse_u64_or_default("DATABASE_IDLE_TIMEOUT", "600")?;
        let database_max_lifetime = parse_u64_or_default("DATABASE_MAX_LIFETIME", "1800")?;

        let redis_url = get_or_default("REDIS_URL", "redis://localhost:6379");
        let redis_pool_size = parse_or_default("REDIS_POOL_SIZE", "50")?;
        let redis_connection_timeout = parse_u64_or_default("REDIS_CONNECTION_TIMEOUT", "5")?;
        let redis_command_timeout = parse_u64_or_default("REDIS_COMMAND_TIMEOUT", "5")?;
        let redis_retry_attempts = parse_or_default("REDIS_RETRY_ATTEMPTS", "3")?;
        let redis_retry_delay_ms = parse_u64_or_default("REDIS_RETRY_DELAY_MS", "100")?;
        let redis_idle_timeout = parse_u64_or_default("REDIS_IDLE_TIMEOUT", "300")?;
        let redis_max_lifetime = parse_u64_or_default("REDIS_MAX_LIFETIME", "3600")?;

        let clickhouse_url = get_or_default("CLICKHOUSE_URL", "http://localhost:8123");
        let clickhouse_database = get_or_default("CLICKHOUSE_DB", "qck_analytics");
        let clickhouse_user = get_or_default("CLICKHOUSE_USER", "qck_user");
        let clickhouse_password = get_or_default("CLICKHOUSE_PASSWORD", "qck_password");

        let jwt_access_expiry = parse_u64_or_default("JWT_ACCESS_EXPIRY", "3600")?;
        let jwt_refresh_expiry = parse_u64_or_default("JWT_REFRESH_EXPIRY", "604800")?;
        let jwt_audience = get_or_default("JWT_AUDIENCE", "qck.sh");
        let jwt_issuer = get_or_default("JWT_ISSUER", "qck.sh");
        let jwt_key_version = parse_or_default("JWT_KEY_VERSION", "1")?;

        let bcrypt_cost = parse_or_default("BCRYPT_COST", "10")?;
        let rate_limit_per_second = parse_or_default("RATE_LIMIT_PER_SECOND", "100")?;
        let rate_limit_burst = parse_or_default("RATE_LIMIT_BURST", "200")?;
        let rate_limit_analytics_sample_rate =
            get_or_default("RATE_LIMIT_ANALYTICS_SAMPLE_RATE", "0.1")
                .parse::<f64>()
                .unwrap_or(0.1);
        let cors_allowed_origins: Vec<String> = get_or_default("CORS_ALLOWED_ORIGINS", "*")
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let jti_hash_salt = env::var("JTI_HASH_SALT").ok();

        // Validate JTI hash salt for production environments
        if environment == Environment::Production {
            if let Some(ref salt) = jti_hash_salt {
                if salt.len() < 32 {
                    return Err(ConfigError::InvalidValue(
                        "JTI_HASH_SALT".to_string(),
                        format!("Salt must be at least 32 bytes in production (current: {} bytes). Generate a secure random salt.", salt.len()),
                    ));
                }
            } else {
                return Err(ConfigError::MissingVar("JTI_HASH_SALT".to_string()));
            }
        }

        // Refresh token rate limiting
        let refresh_rate_limit_max_requests =
            parse_or_default("REFRESH_RATE_LIMIT_MAX_REQUESTS", "10")?;
        let refresh_rate_limit_window_seconds =
            parse_or_default("REFRESH_RATE_LIMIT_WINDOW_SECONDS", "60")?;
        let refresh_rate_limit_burst_limit =
            parse_or_default("REFRESH_RATE_LIMIT_BURST_LIMIT", "3")?;
        let refresh_rate_limit_block_duration =
            parse_or_default("REFRESH_RATE_LIMIT_BLOCK_DURATION", "300")?;

        // Login security configuration (DEV-102)
        let login_rate_limit_per_ip = parse_or_default("LOGIN_RATE_LIMIT_PER_IP", "5")?;
        let login_rate_limit_per_email = parse_or_default("LOGIN_RATE_LIMIT_PER_EMAIL", "10")?;
        let login_lockout_threshold = parse_or_default("LOGIN_LOCKOUT_THRESHOLD", "5")?;
        let login_lockout_duration_seconds =
            parse_or_default("LOGIN_LOCKOUT_DURATION_SECONDS", "1800")?;
        let remember_me_duration_days = parse_or_default("REMEMBER_ME_DURATION_DAYS", "30")?;
        let failed_login_expiry_seconds = parse_or_default("FAILED_LOGIN_EXPIRY_SECONDS", "3600")?;
        let failed_login_ip_expiry_seconds =
            parse_or_default("FAILED_LOGIN_IP_EXPIRY_SECONDS", "300")?;
        let require_email_verification =
            parse_bool_or_default("REQUIRE_EMAIL_VERIFICATION", "false"); // Default to false for now

        let enable_metrics = parse_bool_or_default("ENABLE_METRICS", "true");
        let enable_tracing = parse_bool_or_default("ENABLE_TRACING", "true");
        let enable_rate_limiting = parse_bool_or_default("ENABLE_RATE_LIMITING", "true");
        let enable_swagger_ui = parse_bool_or_default("ENABLE_SWAGGER_UI", "false");
        let disable_embedded_migrations =
            parse_bool_or_default("DISABLE_EMBEDDED_MIGRATIONS", "false");

        let rust_log = get_or_default("RUST_LOG", "info");
        let rust_backtrace = get_or_default("RUST_BACKTRACE", "0") != "0";

        // Get API port (external port for connections, e.g., Docker exposed port)
        let api_port: u16 = env::var("API_PORT")
            .unwrap_or_else(|_| port.to_string())
            .parse()
            .unwrap_or(port); // Default to internal port if not set

        // Create nested configs for compatibility
        let server = ServerConfig {
            bind_address: bind_address.clone(),
            port,
            api_port,
            environment: environment.clone(),
            rust_log: rust_log.clone(),
            rust_backtrace,
        };

        let database = DatabaseConfig {
            url: database_url.clone(),
            max_connections: database_max_connections,
            min_connections: database_min_connections,
            connect_timeout: database_connect_timeout,
            idle_timeout: database_idle_timeout,
            max_lifetime: database_max_lifetime,
            statement_cache_capacity: 100,
        };

        let redis = RedisConfig {
            url: redis_url.clone(),
            pool_size: redis_pool_size,
            connection_timeout: redis_connection_timeout,
            command_timeout: redis_command_timeout,
            retry_attempts: redis_retry_attempts,
            retry_delay_ms: redis_retry_delay_ms,
            idle_timeout: redis_idle_timeout,
            max_lifetime: redis_max_lifetime,
        };

        let clickhouse = ClickHouseConfig {
            url: clickhouse_url.clone(),
            database: clickhouse_database.clone(),
            user: clickhouse_user.clone(),
            password: clickhouse_password.clone(),
        };

        let jwt = JwtConfig {
            access_secret: jwt_access_secret.clone(),
            refresh_secret: jwt_refresh_secret.clone(),
            access_expiry: jwt_access_expiry,
            refresh_expiry: jwt_refresh_expiry,
            audience: jwt_audience.clone(),
            issuer: jwt_issuer.clone(),
            key_version: jwt_key_version,
        };

        let security = SecurityConfig {
            bcrypt_cost,
            rate_limit_per_second,
            rate_limit_burst,
            cors_allowed_origins: cors_allowed_origins.clone(),
            refresh_rate_limit_max_requests,
            refresh_rate_limit_window_seconds,
            refresh_rate_limit_burst_limit,
            refresh_rate_limit_block_duration,
            login_rate_limit_per_ip,
            login_rate_limit_per_email,
            login_lockout_threshold,
            login_lockout_duration_seconds,
            remember_me_duration_days,
            failed_login_expiry_seconds: failed_login_expiry_seconds as usize,
            failed_login_ip_expiry_seconds: failed_login_ip_expiry_seconds as usize,
            require_email_verification,
        };

        // Email configuration
        let email_provider: EmailProvider = get_or_default("EMAIL_PROVIDER", "resend").into();
        let resend_api_key = get_required("RESEND_API_KEY")?;
        let from_email = get_or_default("EMAIL_FROM_ADDRESS", "noreply@qck.sh");
        let from_name = get_or_default("EMAIL_FROM_NAME", "QCK Platform");

        // Use NEXT_PUBLIC_DASHBOARD_URL from environment (same as frontend uses)
        let frontend_url = if let Ok(url) = env::var("NEXT_PUBLIC_DASHBOARD_URL") {
            url
        } else {
            // Fallback to auto-detect based on environment
            match environment.to_string().as_str() {
                "production" => "https://app.qck.sh".to_string(),
                "staging" => "https://s_app.qck.sh".to_string(),
                _ => "http://localhost:10111".to_string(), // dev/local
            }
        };

        let verification_code_ttl: u32 = parse_or_default("EMAIL_VERIFICATION_CODE_TTL", "900")?;
        let verification_max_attempts = parse_or_default("EMAIL_VERIFICATION_MAX_ATTEMPTS", "5")?;
        let resend_limit = parse_or_default("EMAIL_RESEND_LIMIT", "3")?;
        let resend_window: u32 = parse_or_default("EMAIL_RESEND_WINDOW", "86400")?;
        let min_resend_cooldown: u32 = parse_or_default("EMAIL_MIN_RESEND_COOLDOWN", "60")?;

        let support_email = get_or_default("SUPPORT_EMAIL", "support@qck.sh");
        let resend_api_url = get_or_default("RESEND_API_URL", "https://api.resend.com/emails");

        let email = EmailConfig {
            provider: email_provider,
            resend_api_key,
            resend_api_url,
            from_email,
            from_name,
            support_email,
            frontend_url: frontend_url.clone(),
            dashboard_url: dashboard_url.clone(), // Use the top-level dashboard_url
            verification_code_ttl: verification_code_ttl as u64,
            verification_max_attempts,
            resend_limit,
            resend_window: resend_window as u64,
            min_resend_cooldown: min_resend_cooldown as u64,
        };

        let features = FeatureConfig {
            enable_metrics,
            enable_tracing,
            enable_rate_limiting,
            enable_swagger_ui,
        };

        Ok(Self {
            // Direct fields
            bind_address,
            port,
            environment,
            rust_log,
            rust_backtrace,
            database_url,
            database_max_connections,
            database_min_connections,
            database_connect_timeout,
            database_idle_timeout,
            database_max_lifetime,
            redis_url,
            redis_pool_size,
            redis_connection_timeout,
            redis_command_timeout,
            redis_retry_attempts,
            redis_retry_delay_ms,
            redis_idle_timeout,
            redis_max_lifetime,
            clickhouse_url,
            clickhouse_database,
            clickhouse_user,
            clickhouse_password,
            jwt_access_secret,
            jwt_refresh_secret,
            jwt_access_expiry,
            jwt_refresh_expiry,
            jwt_audience,
            jwt_issuer,
            jwt_key_version,
            bcrypt_cost,
            rate_limit_per_second,
            rate_limit_burst,
            rate_limit_analytics_sample_rate,
            cors_allowed_origins,
            jti_hash_salt,
            dashboard_url, // Application URL
            enable_metrics,
            enable_tracing,
            enable_rate_limiting,
            enable_swagger_ui,
            disable_embedded_migrations,
            // Nested configs
            server,
            database,
            redis,
            clickhouse,
            jwt,
            security,
            email,
            features,
        })
    }

    /// Check if running in production
    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }

    /// Check if running in development
    pub fn is_development(&self) -> bool {
        self.environment == Environment::Development
    }

    /// Check if running in test environment
    pub fn is_test(&self) -> bool {
        self.environment == Environment::Test
    }
}

/// Get the global configuration instance
/// This is the primary way to access configuration throughout the app
pub fn config() -> &'static AppConfig {
    &CONFIG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_from_string() {
        assert_eq!(
            Environment::from("development".to_string()),
            Environment::Development
        );
        assert_eq!(
            Environment::from("prod".to_string()),
            Environment::Production
        );
        assert_eq!(Environment::from("test".to_string()), Environment::Test);
        assert_eq!(
            Environment::from("staging".to_string()),
            Environment::Staging
        );
    }

    #[test]
    fn test_config_with_env() {
        // Set required environment variables
        env::set_var("DATABASE_URL", "postgresql://test:test@localhost/test");
        env::set_var(
            "JWT_ACCESS_SECRET",
            "test-secret-that-is-at-least-32-characters-long",
        );
        env::set_var(
            "JWT_REFRESH_SECRET",
            "another-test-secret-that-is-at-least-32-chars",
        );
        env::set_var("JWT_ACCESS_EXPIRY", "7200");
        env::set_var("JWT_REFRESH_EXPIRY", "86400");

        // Load config
        let config = AppConfig::from_env().expect("Failed to load test config");

        // Verify values match what was set
        assert_eq!(config.database_url, "postgresql://test:test@localhost/test");
        assert!(config.jwt_access_secret.len() >= 32);
        assert!(config.jwt_refresh_secret.len() >= 32);
        assert_eq!(config.jwt_access_expiry, 7200);
        assert_eq!(config.jwt_refresh_expiry, 86400);

        // Verify defaults
        assert_eq!(config.environment, Environment::Development);
        // Redis URL can vary based on environment (localhost or redis-dev)
        assert!(config.redis_url.contains("redis://"));

        // Clean up
        env::remove_var("DATABASE_URL");
        env::remove_var("JWT_ACCESS_SECRET");
        env::remove_var("JWT_REFRESH_SECRET");
        env::remove_var("JWT_ACCESS_EXPIRY");
        env::remove_var("JWT_REFRESH_EXPIRY");
    }

    #[test]
    fn test_get_refresh_rate_limit_config() {
        // Set required environment variables
        env::set_var("DATABASE_URL", "postgresql://test:test@localhost/test");
        env::set_var(
            "JWT_ACCESS_SECRET",
            "test-secret-that-is-at-least-32-characters-long",
        );
        env::set_var(
            "JWT_REFRESH_SECRET",
            "another-test-secret-that-is-at-least-32-chars",
        );
        env::set_var("REFRESH_RATE_LIMIT_MAX_REQUESTS", "15");
        env::set_var("REFRESH_RATE_LIMIT_WINDOW_SECONDS", "600");
        env::set_var("REFRESH_RATE_LIMIT_BURST_LIMIT", "5");
        env::set_var("REFRESH_RATE_LIMIT_BLOCK_DURATION", "400");

        // Load config and test refresh rate limit configuration
        let config = AppConfig::from_env().expect("Failed to load test config");
        let refresh_config = config.get_refresh_rate_limit_config();

        assert_eq!(refresh_config.max_requests, 15);
        assert_eq!(refresh_config.window_seconds, 600);
        assert_eq!(refresh_config.burst_limit, Some(5));
        assert_eq!(refresh_config.block_duration, 400);
        assert!(refresh_config.distributed);

        // Clean up
        env::remove_var("DATABASE_URL");
        env::remove_var("JWT_ACCESS_SECRET");
        env::remove_var("JWT_REFRESH_SECRET");
        env::remove_var("REFRESH_RATE_LIMIT_MAX_REQUESTS");
        env::remove_var("REFRESH_RATE_LIMIT_WINDOW_SECONDS");
        env::remove_var("REFRESH_RATE_LIMIT_BURST_LIMIT");
        env::remove_var("REFRESH_RATE_LIMIT_BLOCK_DURATION");
    }
}
