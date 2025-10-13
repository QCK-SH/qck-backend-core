// Application state and configuration
use std::sync::Arc;

use crate::{
    app_config::AppConfig,
    config::RateLimitingConfig,
    db::DieselPool,
    services::{EmailService, JwtService, PasswordResetService, RateLimitService},
    RedisPool,
};

// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub diesel_pool: DieselPool,
    pub redis_pool: RedisPool,
    pub jwt_service: Arc<JwtService>,
    pub rate_limit_service: Arc<RateLimitService>,
    pub rate_limit_config: Arc<RateLimitingConfig>,
    pub password_reset_service: Arc<PasswordResetService>,
    pub email_service: Arc<EmailService>, // For password reset emails
    pub clickhouse_analytics:
        Option<Arc<crate::services::clickhouse_analytics::ClickHouseAnalyticsService>>,
    pub max_connections: u32,
}
