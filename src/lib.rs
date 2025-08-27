// Library exports for QCK Backend
// This file exposes modules for integration testing

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
pub use models::auth::{AccessTokenClaims, RefreshTokenClaims};
pub use models::refresh_token::{RefreshToken, RefreshTokenError};
pub use services::{
    AnalyticsError, JwtConfig, JwtError, JwtService, MonitoringStats, RateLimitAnalytics,
    RateLimitConfig, RateLimitEvent, RateLimitMetrics, RateLimitResult, RateLimitService,
    SubscriptionService, SubscriptionTier,
};

// Diesel database pool type alias
use bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;

pub type DbPool = Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;
