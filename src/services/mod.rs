// Services module for QCK Core Backend (OSS)
// Business logic layer for the application
// NOTE: Verification, Subscription services are cloud-only (qck-cloud)

pub mod analytics;
pub mod background_tasks;
pub mod click_tracking;
pub mod clickhouse_analytics;
pub mod email; // Needed for password reset
pub mod jwt;
pub mod link;
pub mod password_reset;
pub mod rate_limit;
pub mod short_code;

// Re-export commonly used services
pub use analytics::{
    AnalyticsError, MonitoringStats, RateLimitAnalytics, RateLimitEvent, RateLimitMetrics,
};
pub use background_tasks::initialize_background_tasks;
pub use clickhouse_analytics::{create_clickhouse_analytics_service, ClickHouseAnalyticsService};
pub use email::{EmailError, EmailService}; // For password reset emails
pub use jwt::{JwtConfig, JwtError, JwtService};
pub use link::LinkService;
pub use password_reset::{PasswordResetService, PasswordResetTokenInfo};
pub use rate_limit::{
    RateLimitConfig, RateLimitError, RateLimitResult, RateLimitService, SubscriptionLimits,
};
pub use short_code::{GenerationStats, ShortCodeError, ShortCodeGenerator};
