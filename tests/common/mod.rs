// Common test utilities and helper structs
// Shared across all test files to avoid duplication

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, Response, StatusCode},
    Router,
};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection};
use qck_backend_core::{
    app::AppState,
    config::rate_limit::RateLimitingConfig,
    db::{create_diesel_pool, DieselDatabaseConfig, DieselPool, RedisConfig, RedisPool},
    services::{
        EmailService, JwtService, PasswordResetService, RateLimitService,
    },
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::util::ServiceExt;
use uuid::Uuid;

/// Helper struct for test queries that return a single integer
#[derive(QueryableByName)]
pub struct TestRow {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub test: i32,
}

/// Helper struct for count queries
#[derive(QueryableByName)]
pub struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub count: i64,
}

/// Helper struct for queries returning numeric values
#[derive(QueryableByName)]
pub struct NumericRow {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub num: i32,
}

/// Helper struct for refresh token queries
#[derive(QueryableByName)]
pub struct RefreshTokenRow {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pub revoked: bool,
    #[diesel(sql_type = diesel::sql_types::Timestamptz)]
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Generate unique table name for test isolation
pub fn test_table_name(prefix: &str) -> String {
    format!("test_{}_{}", prefix, Uuid::new_v4().simple())
}

/// Common test permissions
pub const TEST_PERMISSIONS_BASIC: &[&str] = &["read"];
pub const TEST_PERMISSIONS_PREMIUM: &[&str] = &["read", "write"];

/// Helper function to convert permission constants to Vec<String>
pub fn test_permissions(permissions: &[&str]) -> Vec<String> {
    permissions.iter().map(|s| s.to_string()).collect()
}

/// Test application wrapper
pub struct TestApp {
    pub app: Router,
    pub diesel_pool: DieselPool,
    pub redis_pool: RedisPool,
    pub jwt_service: Arc<JwtService>,
}

impl TestApp {
    /// Send a POST request
    pub fn post(&self, uri: &str) -> TestRequest {
        TestRequest::new(self, "POST", uri)
    }

    /// Send a GET request
    pub fn get(&self, uri: &str) -> TestRequest {
        TestRequest::new(self, "GET", uri)
    }
}

/// Test request builder
pub struct TestRequest<'a> {
    app: &'a TestApp,
    request: Request<Body>,
    custom_ip: Option<String>,
}

impl<'a> TestRequest<'a> {
    fn new(app: &'a TestApp, method: &str, uri: &str) -> Self {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        Self {
            app,
            request,
            custom_ip: None,
        }
    }

    /// Add JSON body to request
    pub fn json<T: Serialize>(mut self, body: &T) -> Self {
        let body_bytes = serde_json::to_vec(body).unwrap();
        self.request = Request::builder()
            .method(self.request.method().clone())
            .uri(self.request.uri().clone())
            .header("content-type", "application/json")
            .body(Body::from(body_bytes))
            .unwrap();
        self
    }

    /// Set a custom IP address for this request (useful for rate limiting tests)
    pub fn with_ip(mut self, ip: &str) -> Self {
        self.custom_ip = Some(ip.to_string());
        self
    }

    /// Send the request
    pub async fn send(self) -> TestResponse {
        // Add ConnectInfo to the request extensions to simulate a client connection
        let mut request = self.request;

        // Use custom IP if provided, otherwise use a random IP to avoid rate limiting
        let ip_address = if let Some(ip) = self.custom_ip {
            ip
        } else {
            // Use a random IP to avoid rate limiting in tests
            format!(
                "127.0.0.{}:12345",
                rand::random::<u8>().saturating_add(1) // Avoid 127.0.0.0
            )
        };

        request
            .extensions_mut()
            .insert(ConnectInfo(ip_address.parse::<SocketAddr>().unwrap()));

        let response = self.app.app.clone().oneshot(request).await.unwrap();

        TestResponse { response }
    }
}

/// Test response wrapper
pub struct TestResponse {
    response: Response<Body>,
}

impl TestResponse {
    /// Get status code
    pub fn status(&self) -> StatusCode {
        self.response.status()
    }

    /// Parse JSON response
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> T {
        let body = axum::body::to_bytes(self.response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// Get response body as text
    pub async fn text(self) -> String {
        let body = axum::body::to_bytes(self.response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }
}

/// Setup test application with all dependencies
pub async fn setup_test_app() -> TestApp {
    // Load test environment
    dotenv::from_filename(".env.test").ok();

    // Initialize test database pool
    let db_config = DieselDatabaseConfig::default();
    let diesel_pool = create_diesel_pool(db_config).await.unwrap();

    // Initialize test Redis pool
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config).await.unwrap();

    // Initialize services
    let jwt_service = Arc::new(
        JwtService::from_env_with_diesel(diesel_pool.clone(), redis_pool.clone()).unwrap(),
    );

    let rate_limit_service = Arc::new(RateLimitService::new(redis_pool.clone()));

    // Get config
    let config = qck_backend_core::app_config::config();

    // Create email service
    let email_service =
        Arc::new(EmailService::new(config.email.clone()).expect("Failed to create email service"));

    // Create password reset service
    let password_reset_service = Arc::new(PasswordResetService::new(diesel_pool.clone()));

    // Create app state
    let app_state = AppState {
        config: Arc::new(config.clone()),
        diesel_pool: diesel_pool.clone(),
        redis_pool: redis_pool.clone(),
        jwt_service: jwt_service.clone(),
        rate_limit_service,
        rate_limit_config: Arc::new(RateLimitingConfig::from_env()),
        email_service,
        password_reset_service,
        clickhouse_analytics: None, // Disabled for tests
        max_connections: config.database.max_connections,
    };

    // Build router with auth routes (public + protected)
    let app = Router::new()
        .nest("/v1/auth", qck_backend_core::handlers::public_auth_routes())
        .nest("/v1/auth", qck_backend_core::handlers::protected_auth_routes())
        .with_state(app_state);

    TestApp {
        app,
        diesel_pool,
        redis_pool,
        jwt_service,
    }
}

/// Test setup for integration tests with real API
pub struct TestSetup {
    pub api_port: u16,
    pub database_url: String,
    redis_pool: Option<RedisPool>,
}

impl TestSetup {
    pub async fn new() -> Self {
        // Use development environment variables
        dotenv::from_filename(".env.dev").ok();

        // Get API port from app config (which reads from API_PORT env var)
        let config = qck_backend_core::app_config::config();
        let api_port = config.server.api_port;

        // Get database configuration from app_config
        let database_url = config.database.url.clone();

        // For tests running outside containers, we may need to replace container hostname
        // with localhost if the DATABASE_URL contains a container hostname
        let database_url = if database_url.contains("postgres-dev")
            || database_url.contains("postgres-test")
        {
            // Get host and port from environment or use defaults
            let db_host =
                std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
            let db_port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "15432".to_string());

            // Replace any container hostname patterns with the actual host:port
            database_url
                .replace("postgres-dev:5432", &format!("{}:{}", db_host, db_port))
                .replace("postgres-test:5432", &format!("{}:{}", db_host, db_port))
        } else {
            database_url
        };

        Self {
            api_port,
            database_url,
            redis_pool: None,
        }
    }

    /// Get a direct database connection for test queries
    /// This executes via docker to connect to the postgres container
    pub async fn get_database_connection(&self) -> AsyncPgConnection {
        // For the docker-compose.dev.yml environment, we need to connect
        // via the same network as the containers

        // Get database credentials from environment variables with defaults
        let db_user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "qck_user".to_string());
        let db_password =
            std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "qck_password".to_string());
        let db_name = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "qck_db".to_string());

        // First try to connect directly if postgres port is mapped
        let possible_urls = vec![
            // Try common postgres ports with environment credentials
            format!(
                "postgresql://{}:{}@localhost:5432/{}",
                db_user, db_password, db_name
            ),
            format!(
                "postgresql://{}:{}@localhost:15432/{}",
                db_user, db_password, db_name
            ),
            format!(
                "postgresql://{}:{}@127.0.0.1:5432/{}",
                db_user, db_password, db_name
            ),
            // Original env URL
            self.database_url.clone(),
        ];

        for url in possible_urls {
            if let Ok(conn) = AsyncPgConnection::establish(&url).await {
                return conn;
            }
        }

        panic!("Could not establish database connection. Make sure docker-compose.dev.yml is running and postgres is accessible.");
    }

    /// Get Redis pool for test queries
    pub async fn get_redis_pool(&mut self) -> &RedisPool {
        if self.redis_pool.is_none() {
            // Create Redis pool using the same config as the app
            let redis_config = RedisConfig::from_env();
            let redis_pool = RedisPool::new(redis_config)
                .await
                .expect("Failed to create Redis pool for tests");
            self.redis_pool = Some(redis_pool);
        }
        self.redis_pool.as_ref().unwrap()
    }
}
