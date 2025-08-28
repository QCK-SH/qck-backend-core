// Middleware modules for QCK Backend
// DEV-113: JWT Token Validation + DEV-115: Rate Limiting Middleware

pub mod auth;
pub mod auth_middleware;

// Re-export auth types and middleware
pub use auth::AuthenticatedUser;
pub use auth_middleware::auth_middleware;

// TODO: Implement the following middleware modules for Actix-web:
// - AuthMiddleware: JWT validation middleware
// - PermissionsMiddleware: Role-based access control
// - RateLimitMiddleware: Request rate limiting per user/IP
// These will be implemented as part of DEV-115
