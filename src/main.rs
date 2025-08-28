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
    http::{Method, StatusCode},
    middleware as axum_middleware,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    app::AppState,
    config::RateLimitingConfig,
    db::{
        check_diesel_health, create_diesel_pool, mask_connection_string, DieselDatabaseConfig,
        RedisConfig, RedisPool,
    },
    handlers::{auth_routes, docs as docs_handlers, onboarding_routes},
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
    println!("=== STARTING QCK BACKEND API ===");
    info!("Starting QCK Backend API on {}", bind_address);

    // Initialize Diesel database pool
    println!("Initializing database pool...");
    let db_config = DieselDatabaseConfig::default();
    println!("Database URL: {}", mask_connection_string(&db_config.url));

    let max_connections = db_config.max_connections;
    let diesel_pool = match create_diesel_pool(db_config).await {
        Ok(pool) => {
            println!("✓ Database connection pool initialized successfully");
            info!("Database connection pool initialized successfully");
            pool
        },
        Err(e) => {
            println!("✗ Failed to initialize database pool: {}", e);
            error!("Failed to initialize database pool: {}", e);
            return Err(format!("Database initialization failed: {}", e).into());
        },
    };

    // Run embedded migrations (production/test environments)
    if crate::migrations::should_run_migrations() {
        println!("Running embedded migrations...");
        let migration_config = crate::migrations::MigrationConfig::default();

        match crate::migrations::run_all_migrations(&diesel_pool, migration_config).await {
            Ok(()) => {
                println!("✓ All migrations completed successfully");
                info!("All migrations completed successfully");
            },
            Err(e) => {
                println!("✗ Migration failed: {}", e);
                error!("Migration failed: {}", e);
                return Err(format!("Migration failed: {}", e).into());
            },
        }
    } else {
        println!("Embedded migrations disabled - using external migration scripts");
        info!("Embedded migrations disabled - using external migration scripts");
    }

    // Initialize Redis pool
    println!("Initializing Redis pool...");
    let redis_config = RedisConfig::from_env();
    let redis_pool = match RedisPool::new(redis_config).await {
        Ok(pool) => {
            println!("✓ Redis connection pool initialized successfully");
            info!("Redis connection pool initialized successfully");
            pool
        },
        Err(e) => {
            println!("✗ Failed to initialize Redis pool: {}", e);
            error!("Failed to initialize Redis pool: {}", e);
            return Err(format!("Redis initialization failed: {}", e).into());
        },
    };

    // Initialize rate limiting service and configuration
    println!("Initializing rate limiting service...");
    let rate_limit_config = Arc::new(RateLimitingConfig::from_env());

    // Validate rate limiting configuration
    if let Err(e) = rate_limit_config.validate() {
        println!("✗ Rate limiting configuration validation failed: {}", e);
        error!("Rate limiting configuration validation failed: {}", e);
        return Err(format!("Rate limiting configuration invalid: {}", e).into());
    }

    // Enable analytics with configured sampling rate for production performance
    let analytics_sample_rate = config.rate_limit_analytics_sample_rate;

    let rate_limit_service = Arc::new(RateLimitService::new_with_analytics(
        redis_pool.clone(),
        analytics_sample_rate,
    ));
    println!(
        "✓ Rate limiting service initialized successfully (analytics enabled with {}% sampling)",
        analytics_sample_rate * 100.0
    );
    info!("Rate limiting service initialized successfully with analytics");

    // Initialize JWT service with Diesel pool
    println!("Initializing JWT service...");
    let jwt_service =
        match JwtService::from_env_with_diesel(diesel_pool.clone(), redis_pool.clone()) {
            Ok(service) => {
                println!("✓ JWT service initialized successfully");
                info!("JWT service initialized successfully");
                Arc::new(service)
            },
            Err(e) => {
                println!("✗ Failed to initialize JWT service: {}", e);
                error!("Failed to initialize JWT service: {}", e);
                return Err(format!("JWT service initialization failed: {}", e).into());
            },
        };

    // Initialize subscription service
    println!("Initializing subscription service...");
    let subscription_service = Arc::new(SubscriptionService::new());
    println!("✓ Subscription service initialized successfully");
    info!("Subscription service initialized successfully");

    // Initialize password reset service
    println!("Initializing password reset service...");
    let password_reset_service = Arc::new(PasswordResetService::new(diesel_pool.clone()));
    println!("✓ Password reset service initialized successfully");
    info!("Password reset service initialized successfully");

    // Initialize email service
    println!("Initializing email service...");
    let email_service = match EmailService::new(config.email.clone()) {
        Ok(service) => {
            println!("✓ Email service initialized successfully");
            info!("Email service initialized successfully");
            Arc::new(service)
        },
        Err(e) => {
            println!("✓ Email service initialization failed: {}", e);
            info!("Email service initialization failed: {}", e);
            // For development, continue without email service but log the issue
            // In production, you might want to fail here depending on requirements
            return Err(format!("Email service initialization failed: {}", e).into());
        },
    };

    // Create shared application state
    let app_state = AppState {
        diesel_pool: diesel_pool.clone(),
        redis_pool: redis_pool.clone(),
        jwt_service,
        rate_limit_service,
        rate_limit_config,
        subscription_service,
        password_reset_service,
        email_service,
        max_connections,
    };

    // Configure CORS - NEVER use wildcard
    println!(
        "CORS: Configuring specific origins: {:?}",
        config.cors_allowed_origins
    );

    // Parse all origins, filtering out wildcards and invalid entries
    let valid_origins: Vec<axum::http::HeaderValue> = config
        .cors_allowed_origins
        .iter()
        .filter(|origin| *origin != "*") // Never allow wildcard
        .filter_map(|origin| {
            match origin.parse::<axum::http::HeaderValue>() {
                Ok(parsed) => {
                    println!("  ✓ Valid CORS origin: {}", origin);
                    Some(parsed)
                },
                Err(e) => {
                    eprintln!("  ✗ Invalid CORS origin '{}': {} - skipping", origin, e);
                    None
                }
            }
        })
        .collect();

    if valid_origins.is_empty() {
        eprintln!(
            "ERROR: No valid CORS origins configured! Add valid origins to CORS_ALLOWED_ORIGINS"
        );
        return Err("No valid CORS origins configured".into());
    }

    // Build CORS layer with validated origins
    // Note: Cannot use wildcard headers with credentials
    let cors = CorsLayer::new()
        .allow_origin(valid_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::ORIGIN,
            axum::http::header::ACCESS_CONTROL_REQUEST_METHOD,
            axum::http::header::ACCESS_CONTROL_REQUEST_HEADERS,
        ])
        .allow_credentials(true)
        .expose_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    // Build the application router
    let app = Router::new()
        // Health check endpoints
        .route("/v1/health", get(comprehensive_health_check))
        .route("/v1/metrics/rate-limiting", get(rate_limit_metrics_handler))
        // API Documentation (legacy paths for backward compatibility)
        .route("/docs", get(docs_handlers::redirect_to_docs))
        .route("/docs/", get(docs_handlers::serve_swagger_ui))
        .route("/docs/openapi.json", get(docs_handlers::serve_openapi_spec))
        // Versioned API Documentation
        .route("/v1/docs", get(docs_handlers::redirect_to_docs))
        .route("/v1/docs/", get(docs_handlers::serve_swagger_ui))
        .route("/v1/docs/openapi.json", get(docs_handlers::serve_openapi_spec))
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
        // API routes (to be added)
        .nest("/v1", api_routes())
        // Redirect routes (to be added)
        .nest("/r", redirect_routes())
        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(Extension(app_state.clone()))
        )
        .with_state(app_state);

    // Parse and bind to address
    let addr: SocketAddr = bind_address.parse()?;
    println!("Starting HTTP server on {}...", addr);

    // Create the server with ConnectInfo support for client IP tracking
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}

// API routes (placeholder)
fn api_routes() -> Router<AppState> {
    Router::new()
    // TODO: Add API routes here (links, analytics, etc.)
}

// Redirect routes (placeholder)
fn redirect_routes() -> Router<AppState> {
    Router::new()
    // TODO: Add redirect routes here
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

    let clickhouse_url = crate::app_config::config().clickhouse_url.clone();
    let client = reqwest::Client::new();

    let start = Instant::now();
    let response = client
        .get(format!("{}/ping", clickhouse_url))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err("ClickHouse ping failed".into());
    }

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
