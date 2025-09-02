// Services module for QCK Backend
// Business logic layer for the application

pub mod analytics;
pub mod background_tasks;
pub mod click_tracking;
pub mod clickhouse_analytics;
pub mod email;
pub mod jwt;
pub mod link;
pub mod password_reset;
pub mod rate_limit;
pub mod short_code;
pub mod subscription;
pub mod verification;

// Re-export commonly used services
pub use analytics::{
    AnalyticsError, MonitoringStats, RateLimitAnalytics, RateLimitEvent, RateLimitMetrics,
};
pub use background_tasks::initialize_background_tasks;
pub use clickhouse_analytics::{create_clickhouse_analytics_service, ClickHouseAnalyticsService};
pub use email::{EmailError, EmailService};
pub use jwt::{JwtConfig, JwtError, JwtService};
pub use link::LinkService;
pub use password_reset::{PasswordResetService, PasswordResetTokenInfo};
pub use rate_limit::{
    RateLimitConfig, RateLimitError, RateLimitResult, RateLimitService, SubscriptionLimits,
};
pub use short_code::{GenerationStats, ShortCodeError, ShortCodeGenerator};
pub use subscription::{SubscriptionError, SubscriptionService, SubscriptionTier};
pub use verification::{VerificationError, VerificationService};
