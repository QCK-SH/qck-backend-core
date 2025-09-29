// Library exports for QCK Backend
// This file exposes modules and functions for library consumers

pub mod app;
pub mod app_config;
pub mod config;
pub mod db;
pub mod handlers;
pub mod middleware;
pub mod migrations;
pub mod models;
pub mod schema;
pub mod services;
pub mod utils;

// Re-export commonly used types
pub use app::AppState;
pub use app_config::{AppConfig, CONFIG};
pub use config::{GlobalRateLimitSettings, RateLimitingConfig};
pub use db::{DatabaseConfig, DieselPool, RedisConfig, RedisPool};
pub use middleware::AuthenticatedUser;
pub use middleware::auth_middleware;
pub use models::auth::{AccessTokenClaims, RefreshTokenClaims};
pub use models::refresh_token::{RefreshToken, RefreshTokenError};
pub use services::{
    AnalyticsError, JwtConfig, JwtError, JwtService, MonitoringStats, RateLimitAnalytics,
    RateLimitConfig, RateLimitEvent, RateLimitMetrics, RateLimitResult, RateLimitService,
    SubscriptionService, SubscriptionTier, EmailService, VerificationService,
    PasswordResetService,
};

// Re-export handler route builders
pub use handlers::auth_routes;

// Re-export individual handlers for direct use
pub use handlers::auth::{register, login, refresh_token, logout, get_current_user, validate_token, forgot_password, reset_password};
pub use handlers::links::{create_link, get_link, update_link, delete_link, list_links, get_link_stats, bulk_create_links, check_alias_availability};
pub use handlers::redirect::{redirect_to_url, preview_url};

// Diesel database pool type alias
use bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;

pub type DbPool = Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;

// Library initialization function for external consumers
// This allows qck-cloud to initialize the core backend services
pub async fn initialize_app_state() -> Result<AppState, Box<dyn std::error::Error>> {
    use std::sync::Arc;
    use tracing::info;

    // Load environment
    dotenv::dotenv().ok();

    // Initialize config
    let config = app_config::config();

    // Initialize database pool
    info!("Initializing database pool...");
    let db_config = db::DieselDatabaseConfig::default();
    let max_connections = db_config.max_connections;
    let diesel_pool = db::create_diesel_pool(db_config).await?;

    // Run migrations if enabled
    if migrations::should_run_migrations() {
        info!("Running embedded migrations...");
        let migration_config = migrations::MigrationConfig::default();
        migrations::run_all_migrations(&diesel_pool, migration_config).await
            .map_err(|e| format!("Migration failed: {}", e))?;
    }

    // Initialize Redis pool
    info!("Initializing Redis pool...");
    let redis_config = RedisConfig::from_env();
    let redis_pool = RedisPool::new(redis_config).await?;

    // Initialize services
    let rate_limit_config = Arc::new(RateLimitingConfig::from_env());
    let rate_limit_service = Arc::new(RateLimitService::new_with_analytics(
        redis_pool.clone(),
        config.rate_limit_analytics_sample_rate,
    ));

    let jwt_service = Arc::new(
        JwtService::from_env_with_diesel(diesel_pool.clone(), redis_pool.clone())?
    );

    let subscription_service = Arc::new(SubscriptionService::new());
    let password_reset_service = Arc::new(PasswordResetService::new(diesel_pool.clone()));
    let email_service = Arc::new(EmailService::new(config.email.clone())?);

    // Initialize ClickHouse if configured
    let clickhouse_analytics = if !config.clickhouse_url.is_empty() {
        let client = db::create_clickhouse_client();
        Some(Arc::new(
            services::clickhouse_analytics::ClickHouseAnalyticsService::new(client)
        ))
    } else {
        None
    };

    // Create app state
    Ok(AppState {
        config: Arc::new(config.clone()),
        diesel_pool: diesel_pool.clone(),
        redis_pool: redis_pool.clone(),
        jwt_service,
        rate_limit_service,
        rate_limit_config,
        subscription_service,
        password_reset_service,
        email_service,
        clickhouse_analytics,
        max_connections,
    })
}

// Re-export route builders for links
pub fn links_routes() -> axum::Router<AppState> {
    use axum::routing::{delete, get, post, put};
    use handlers::links;

    axum::Router::new()
        .route("/", post(links::create_link).get(links::list_links))
        .route("/bulk", post(links::bulk_create_links))
        .route("/check-alias/:alias", get(links::check_alias_availability))
        .route("/custom", post(links::create_custom_link))
        .route("/:id",
            get(links::get_link)
            .put(links::update_link)
            .delete(links::delete_link))
        .route("/:id/stats", get(links::get_link_stats))
}

// Health check handler
pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use axum::Json;

    let mut overall_healthy = true;
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Check PostgreSQL
    let postgres_health = match db::check_diesel_health(&state.diesel_pool).await {
        Ok(_) => serde_json::json!({
            "status": "healthy",
            "max_connections": state.max_connections,
            "error": null
        }),
        Err(e) => {
            overall_healthy = false;
            serde_json::json!({
                "status": "unhealthy",
                "error": format!("Database connection failed: {}", e)
            })
        }
    };

    // Check Redis
    let redis_health_result = state.redis_pool.health_check().await;
    if !redis_health_result.is_healthy {
        overall_healthy = false;
    }

    let response = serde_json::json!({
        "status": if overall_healthy { "healthy" } else { "degraded" },
        "service": "qck-backend",
        "timestamp": timestamp,
        "components": {
            "postgresql": postgres_health,
            "redis": serde_json::json!({
                "status": if redis_health_result.is_healthy { "healthy" } else { "unhealthy" },
                "latency_ms": redis_health_result.latency_ms,
                "error": redis_health_result.error
            })
        }
    });

    if overall_healthy {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}
