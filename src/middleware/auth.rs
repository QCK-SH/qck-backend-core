// Temporary auth middleware for compatibility
// This will be replaced with proper Axum middleware

use serde::{Deserialize, Serialize};

/// Authenticated user information extracted from JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub token_id: String,
    pub email: String,
    pub subscription_tier: String,
    pub permissions: Vec<String>,
    pub exp: u64,
}
