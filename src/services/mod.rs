// Services module for QCK Backend
// Business logic layer for the application

pub mod analytics;
pub mod email;
pub mod jwt;
pub mod password_reset;
pub mod rate_limit;
pub mod subscription;
pub mod verification;

// Re-export commonly used services
pub use analytics::{
    AnalyticsError, MonitoringStats, RateLimitAnalytics, RateLimitEvent, RateLimitMetrics,
};
pub use email::{EmailError, EmailService};
pub use jwt::{JwtConfig, JwtError, JwtService};
pub use password_reset::{PasswordResetService, PasswordResetTokenInfo};
pub use rate_limit::{
    RateLimitConfig, RateLimitError, RateLimitResult, RateLimitService, SubscriptionLimits,
};
pub use subscription::{SubscriptionError, SubscriptionService, SubscriptionTier};
pub use verification::{VerificationError, VerificationService};
