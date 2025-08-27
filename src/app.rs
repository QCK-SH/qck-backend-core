// Application state and configuration
use std::sync::Arc;

use crate::{
    config::RateLimitingConfig,
    db::DieselPool,
    services::{
        EmailService, JwtService, PasswordResetService, RateLimitService, SubscriptionService,
    },
    RedisPool,
};

// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub diesel_pool: DieselPool,
    pub redis_pool: RedisPool,
    pub jwt_service: Arc<JwtService>,
    pub rate_limit_service: Arc<RateLimitService>,
    pub rate_limit_config: Arc<RateLimitingConfig>,
    pub subscription_service: Arc<SubscriptionService>,
    pub password_reset_service: Arc<PasswordResetService>,
    pub email_service: Arc<EmailService>,
    pub max_connections: u32,
}
