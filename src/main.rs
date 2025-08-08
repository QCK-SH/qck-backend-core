use actix_web::{web, App, HttpServer, HttpResponse, middleware};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod db;
mod handlers;
use db::{PostgresPool, DatabaseConfig, postgres::mask_connection_string, RedisPool, RedisConfig};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qck_backend=debug,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenv::dotenv().ok();
    
    let bind_address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    println!("=== STARTING QCK BACKEND API ===");
    info!("Starting QCK Backend API on {}", bind_address);
    
    // Initialize database pool
    println!("Initializing database pool...");
    let db_config = DatabaseConfig::from_env();
    println!("Database URL: {}", mask_connection_string(&db_config.database_url));
    
    let max_connections = db_config.max_connections;
    let postgres_pool = match PostgresPool::new(db_config).await {
        Ok(pool) => {
            println!("✓ Database connection pool initialized successfully");
            info!("Database connection pool initialized successfully");
            pool
        }
        Err(e) => {
            println!("✗ Failed to initialize database pool: {}", e);
            error!("Failed to initialize database pool: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Database initialization failed: {}", e),
            ));
        }
    };
    
    // Initialize Redis pool
    println!("Initializing Redis pool...");
    let redis_config = RedisConfig::from_env();
    let redis_pool = match RedisPool::new(redis_config).await {
        Ok(pool) => {
            println!("✓ Redis connection pool initialized successfully");
            info!("Redis connection pool initialized successfully");
            pool
        }
        Err(e) => {
            println!("✗ Failed to initialize Redis pool: {}", e);
            error!("Failed to initialize Redis pool: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Redis initialization failed: {}", e),
            ));
        }
    };
    
    // Clone pools for the closure
    let pool_data = web::Data::new(postgres_pool.clone_pool());
    let redis_data = web::Data::new(redis_pool.clone());

    println!("Starting HTTP server on {}...", bind_address);
    
    HttpServer::new(move || {
        App::new()
            .app_data(pool_data.clone())
            .app_data(redis_data.clone())
            .app_data(web::Data::new(max_connections))
            .wrap(middleware::Logger::default())
            .wrap(middleware::NormalizePath::trim())
            .service(
                web::scope("/api/v1")
                    .route("/health", web::get().to(comprehensive_health_check))
            )
    })
    .bind(bind_address)?
    .run()
    .await
}

async fn comprehensive_health_check(
    pool: web::Data<sqlx::PgPool>,
    redis_pool: web::Data<RedisPool>,
    max_connections: web::Data<u32>
) -> actix_web::Result<HttpResponse> {
    use serde_json::json;
    
    // Overall service health
    let mut overall_healthy = true;
    let timestamp = chrono::Utc::now().to_rfc3339();
    
    // PostgreSQL health check - use existing pool and config
    let health = PostgresPool::health_check_with_pool(&pool, **max_connections).await;
    if !health.is_healthy {
        overall_healthy = false;
    }
    let postgres_health = json!({
        "status": if health.is_healthy { "healthy" } else { "unhealthy" },
        "latency_ms": health.latency_ms,
        "active_connections": health.active_connections,
        "idle_connections": health.idle_connections,
        "max_connections": health.max_connections,
        "error": health.error
    });
    
    // Redis health check - use the pool
    let redis_health_result = redis_pool.health_check().await;
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
        }
        Err(e) => {
            overall_healthy = false;
            json!({
                "status": "unhealthy",
                "error": format!("ClickHouse connection failed: {}", e)
            })
        }
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
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(response))
    }
}


async fn check_clickhouse_health() -> Result<i64, Box<dyn std::error::Error>> {
    use std::time::Instant;
    
    let clickhouse_url = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://clickhouse:8123".to_string());
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