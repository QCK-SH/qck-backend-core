// Module declarations
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

// Re-export CONFIG for use in other modules
pub use app_config::CONFIG;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    middleware as axum_middleware,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    app::AppState,
    config::RateLimitingConfig,
    db::{
        check_diesel_health, create_diesel_pool, mask_connection_string, DieselDatabaseConfig,
        RedisConfig, RedisPool,
    },
    handlers::{
        auth as auth_handlers, auth_routes, docs as docs_handlers, links as link_handlers,
        onboarding_routes, redirect as redirect_handlers,
    },
    middleware::auth_middleware,
    services::{
        EmailService, JwtService, PasswordResetService, RateLimitService, SubscriptionService,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Handle version flag for Docker health checks - must be FIRST
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--version" {
        println!("qck-backend v{}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qck_backend=debug,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize centralized config (loads all env vars ONCE)
    let config = crate::app_config::config();
    let bind_address = config.bind_address.clone();
    info!("=== STARTING QCK BACKEND API ===");
    info!("Starting QCK Backend API on {}", bind_address);

    // Initialize Diesel database pool
    info!("Initializing database pool...");
    let db_config = DieselDatabaseConfig::default();
    info!("Database URL: {}", mask_connection_string(&db_config.url));

    let max_connections = db_config.max_connections;
    let diesel_pool = match create_diesel_pool(db_config).await {
        Ok(pool) => {
            info!("✓ Database connection pool initialized successfully");
            pool
        },
        Err(e) => {
            error!("✗ Failed to initialize database pool: {}", e);
            return Err(format!("Database initialization failed: {}", e).into());
        },
    };

    // Run embedded migrations (production/test environments)
    if crate::migrations::should_run_migrations() {
        info!("Running embedded migrations...");
        let migration_config = crate::migrations::MigrationConfig::default();

        match crate::migrations::run_all_migrations(&diesel_pool, migration_config).await {
            Ok(()) => {
                info!("✓ All migrations completed successfully");
            },
            Err(e) => {
                error!("✗ Migration failed: {}", e);
                return Err(format!("Migration failed: {}", e).into());
            },
        }
    } else {
        info!("Embedded migrations disabled - using external migration scripts");
    }

    // Initialize Redis pool
    info!("Initializing Redis pool...");
    let redis_config = RedisConfig::from_env();
    let redis_pool = match RedisPool::new(redis_config).await {
        Ok(pool) => {
            info!("✓ Redis connection pool initialized successfully");
            pool
        },
        Err(e) => {
            error!("✗ Failed to initialize Redis pool: {}", e);
            return Err(format!("Redis initialization failed: {}", e).into());
        },
    };

    // Initialize rate limiting service and configuration
    info!("Initializing rate limiting service...");
    let rate_limit_config = Arc::new(RateLimitingConfig::from_env());

    // Validate rate limiting configuration
    if let Err(e) = rate_limit_config.validate() {
        error!("✗ Rate limiting configuration validation failed: {}", e);
        return Err(format!("Rate limiting configuration invalid: {}", e).into());
    }

    // Enable analytics with configured sampling rate for production performance
    let analytics_sample_rate = config.rate_limit_analytics_sample_rate;

    let rate_limit_service = Arc::new(RateLimitService::new_with_analytics(
        redis_pool.clone(),
        analytics_sample_rate,
    ));
    info!(
        "✓ Rate limiting service initialized successfully (analytics enabled with {}% sampling)",
        analytics_sample_rate * 100.0
    );

    // Initialize JWT service with Diesel pool
    info!("Initializing JWT service...");
    let jwt_service =
        match JwtService::from_env_with_diesel(diesel_pool.clone(), redis_pool.clone()) {
            Ok(service) => {
                info!("✓ JWT service initialized successfully");
                Arc::new(service)
            },
            Err(e) => {
                error!("✗ Failed to initialize JWT service: {}", e);
                return Err(format!("JWT service initialization failed: {}", e).into());
            },
        };

    // Initialize subscription service
    info!("Initializing subscription service...");
    let subscription_service = Arc::new(SubscriptionService::new());
    info!("✓ Subscription service initialized successfully");

    // Initialize password reset service
    info!("Initializing password reset service...");
    let password_reset_service = Arc::new(PasswordResetService::new(diesel_pool.clone()));
    info!("✓ Password reset service initialized successfully");

    // Initialize email service
    info!("Initializing email service...");
    let email_service = match EmailService::new(config.email.clone()) {
        Ok(service) => {
            info!("✓ Email service initialized successfully");
            Arc::new(service)
        },
        Err(e) => {
            warn!("⚠ Email service initialization failed: {}", e);
            // For development, continue without email service but log the issue
            // In production, you might want to fail here depending on requirements
            return Err(format!("Email service initialization failed: {}", e).into());
        },
    };

    // Initialize ClickHouse analytics service (unified service for all ClickHouse operations)
    info!("Initializing ClickHouse analytics service...");
    let clickhouse_analytics = if !config.clickhouse_url.is_empty() {
        let client = crate::db::create_clickhouse_client();

        // Create unified ClickHouse analytics service (handles both analytics and event tracking)
        Some(Arc::new(
            crate::services::clickhouse_analytics::ClickHouseAnalyticsService::new(client),
        ))
    } else {
        warn!("ClickHouse URL not configured, click tracking and analytics will be disabled");
        None
    };

    // Create shared application state
    let app_state = AppState {
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
    };

    // Configure CORS - Environment-aware wildcard handling
    info!(
        "CORS: Configuring origins for {} environment: {:?}",
        config.environment, config.cors_allowed_origins
    );

    // Check if wildcard is requested and handle based on environment
    let has_wildcard = config
        .cors_allowed_origins
        .iter()
        .any(|origin| origin == "*");

    if has_wildcard && !config.is_production() {
        info!("CORS: Using dynamic origin reflection for staging/dev (wildcard with credentials support)");
    } else if has_wildcard && config.is_production() {
        error!("CORS: Wildcard '*' detected in production - will be ignored for security!");
    } else {
        info!(
            "CORS: Using whitelist mode with origins: {:?}",
            config.cors_allowed_origins
        );
    }

    // Build the application router - conditionally include Swagger UI
    let mut app = Router::new()
        // Health check endpoints
        .route("/v1/health", get(comprehensive_health_check))
        .route("/v1/metrics/rate-limiting", get(rate_limit_metrics_handler));

    // Conditionally add Swagger UI routes based on configuration
    if config.enable_swagger_ui {
        info!("🔧 Swagger UI: ENABLED at /v1/docs");
        app = app
            // Versioned API Documentation
            .route("/v1/docs", get(docs_handlers::redirect_to_docs))
            .route("/v1/docs/", get(docs_handlers::serve_swagger_ui))
            .route("/v1/docs/openapi.json", get(docs_handlers::serve_openapi_spec));
    } else {
        info!("🔧 Swagger UI: DISABLED (set ENABLE_SWAGGER_UI=true to enable)");
    }

    // Complete router setup
    let app = app
        // Authentication routes
        .nest("/v1/auth", auth_routes())
        // Onboarding routes (protected with auth middleware)
        .nest(
            "/v1/onboarding",
            onboarding_routes()
                .route_layer(axum_middleware::from_fn_with_state(
                    app_state.clone(),
                    auth_middleware,
                ))
        )
        // API routes (protected with auth middleware)
        .nest("/v1", api_routes()
            .route_layer(axum_middleware::from_fn_with_state(
                app_state.clone(),
                auth_middleware,
            )))
        // Short URL redirects at root level (qck.sh/abc123)
        .route("/{short_code}", get(handlers::redirect::redirect_to_url))
        .route("/{short_code}/preview", get(handlers::redirect::preview_url))
        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum_middleware::from_fn(crate::middleware::dynamic_cors_middleware))
                .layer(Extension(app_state.clone()))
        )
        .with_state(app_state.clone());

    // Start URLhaus threat intelligence updater
    crate::utils::urlhaus_client::spawn_urlhaus_updater();
    info!("URLhaus threat intelligence updater started");

    // Start background tasks for click count synchronization
    info!("Starting background tasks for click tracking synchronization...");
    crate::services::background_tasks::initialize_background_tasks(app_state).await;
    info!("Background task manager started - syncing click counts every 5 minutes");

    // Parse and bind to address
    let addr: SocketAddr = bind_address.parse()?;
    info!("Starting HTTP server on {}...", addr);

    // Create the server with ConnectInfo support for client IP tracking
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}

// API routes for link management
fn api_routes() -> Router<AppState> {
    use axum::routing::{delete, get, post, put};
    use handlers::links;

    Router::new()
        // Link management routes (protected)
        .route("/links", post(links::create_link).get(links::list_links))
        .route("/links/bulk", post(links::bulk_create_links))
        .route("/links/check-alias/{alias}", get(links::check_alias_availability))
        .route("/links/custom", post(links::create_custom_link))
        .route("/links/{id}", get(links::get_link).put(links::update_link).delete(links::delete_link))
        .route("/links/{id}/stats", get(links::get_link_stats))
}

// Health check handler
async fn comprehensive_health_check(State(state): State<AppState>) -> impl IntoResponse {
    use serde_json::json;

    // Overall service health
    let mut overall_healthy = true;
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Diesel/PostgreSQL health check
    let postgres_health = match check_diesel_health(&state.diesel_pool).await {
        Ok(_) => {
            json!({
                "status": "healthy",
                "max_connections": state.max_connections,
                "error": null
            })
        },
        Err(e) => {
            overall_healthy = false;
            json!({
                "status": "unhealthy",
                "error": format!("Database connection failed: {}", e)
            })
        },
    };

    // Redis health check
    let redis_health_result = state.redis_pool.health_check().await;
    if !redis_health_result.is_healthy {
        overall_healthy = false;
    }
    let redis_health = json!({
        "status": if redis_health_result.is_healthy { "healthy" } else { "unhealthy" },
        "latency_ms": redis_health_result.latency_ms,
        "active_connections": redis_health_result.active_connections,
        "total_connections": redis_health_result.total_connections,
        "error": redis_health_result.error
    });

    // ClickHouse health check
    let clickhouse_health = match check_clickhouse_health().await {
        Ok(latency) => {
            json!({
                "status": "healthy",
                "latency_ms": latency,
                "error": null
            })
        },
        Err(e) => {
            overall_healthy = false;
            json!({
                "status": "unhealthy",
                "error": format!("ClickHouse connection failed: {}", e)
            })
        },
    };

    let response = json!({
        "status": if overall_healthy { "healthy" } else { "degraded" },
        "service": "qck-backend",
        "timestamp": timestamp,
        "components": {
            "postgresql": postgres_health,
            "redis": redis_health,
            "clickhouse": clickhouse_health
        }
    });

    if overall_healthy {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}

async fn check_clickhouse_health() -> Result<i64, Box<dyn std::error::Error>> {
    use std::time::Instant;

    let start = Instant::now();
    let clickhouse_client = crate::db::create_clickhouse_client();

    // Use the client's health_check method
    clickhouse_client.health_check().await?;

    let latency = start.elapsed().as_millis() as i64;
    Ok(latency)
}

async fn rate_limit_metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    use serde_json::json;

    // Get analytics metrics for the last hour
    let analytics_metrics = state.rate_limit_service.get_analytics_metrics(60).await;
    let monitoring_stats = state.rate_limit_service.get_monitoring_stats().await;

    let response = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "analytics_enabled": analytics_metrics.is_some(),
        "metrics": analytics_metrics,
        "monitoring": monitoring_stats
    });

    Json(response)
}
