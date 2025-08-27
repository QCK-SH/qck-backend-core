// Authentication-specific error handling utilities
// DEV-102: Login API with comprehensive error handling

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
use thiserror::Error;

/// Authentication-specific errors
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Account is locked due to too many failed attempts")]
    AccountLocked { retry_after_seconds: u64 },

    #[error("Email not verified")]
    EmailNotVerified,

    #[error("Account is inactive")]
    AccountInactive,

    #[error("Too many login attempts")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Token generation failed: {0}")]
    TokenError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Internal server error")]
    InternalError,
}

/// Standard authentication response structure
#[derive(Debug, Serialize)]
pub struct AuthErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
}

impl AuthError {
    /// Convert to HTTP status code
    pub fn status_code(&self) -> StatusCode {
        match self {
            AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AuthError::AccountLocked { .. } => StatusCode::LOCKED,
            AuthError::EmailNotVerified => StatusCode::FORBIDDEN,
            AuthError::AccountInactive => StatusCode::FORBIDDEN,
            AuthError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            AuthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::TokenError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AuthError::UserNotFound => StatusCode::NOT_FOUND,
            AuthError::InvalidToken => StatusCode::BAD_REQUEST,
            AuthError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Convert to error code string
    pub fn error_code(&self) -> &'static str {
        match self {
            AuthError::InvalidCredentials => "INVALID_CREDENTIALS",
            AuthError::AccountLocked { .. } => "ACCOUNT_LOCKED",
            AuthError::EmailNotVerified => "EMAIL_NOT_VERIFIED",
            AuthError::AccountInactive => "ACCOUNT_INACTIVE",
            AuthError::RateLimited { .. } => "RATE_LIMITED",
            AuthError::DatabaseError(_) => "DATABASE_ERROR",
            AuthError::TokenError(_) => "TOKEN_ERROR",
            AuthError::ValidationError(_) => "VALIDATION_ERROR",
            AuthError::UserNotFound => "USER_NOT_FOUND",
            AuthError::InvalidToken => "INVALID_TOKEN",
            AuthError::InternalError => "INTERNAL_ERROR",
        }
    }

    /// Get retry_after value if applicable
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            AuthError::AccountLocked {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            AuthError::RateLimited {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            _ => None,
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let response = AuthErrorResponse {
            success: false,
            error: ErrorDetail {
                code: self.error_code().to_string(),
                description: self.to_string(),
                retry_after: self.retry_after(),
            },
            message: self.to_string(),
        };

        (status, Json(response)).into_response()
    }
}

/// Helper function to log authentication failures
pub fn log_auth_failure(
    user_email: &str,
    ip_address: &str,
    error: &AuthError,
    user_agent: Option<&str>,
) {
    tracing::warn!(
        email = user_email,
        ip = ip_address,
        user_agent = user_agent.unwrap_or("unknown"),
        error_code = error.error_code(),
        "Authentication failure"
    );
}

/// Helper function to create audit log entry for authentication events
pub fn create_auth_audit_entry(
    event_type: AuthEventType,
    user_id: Option<&str>,
    email: &str,
    ip_address: &str,
    user_agent: Option<&str>,
    additional_data: Option<serde_json::Value>,
) -> AuthAuditEntry {
    AuthAuditEntry {
        event_type,
        user_id: user_id.map(String::from),
        email: email.to_string(),
        ip_address: ip_address.to_string(),
        user_agent: user_agent.map(String::from),
        timestamp: chrono::Utc::now(),
        additional_data,
    }
}

#[derive(Debug, Serialize)]
pub enum AuthEventType {
    LoginSuccess,
    LoginFailed,
    LoginRateLimited,
    AccountLocked,
    // TODO: Implement audit logging for AccountUnlocked events
    AccountUnlocked,
    // TODO: Implement audit logging for PasswordReset events
    PasswordReset,
    // TODO: Implement audit logging for EmailVerified events
    EmailVerified,
}

#[derive(Debug, Serialize)]
pub struct AuthAuditEntry {
    pub event_type: AuthEventType,
    pub user_id: Option<String>,
    pub email: String,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub additional_data: Option<serde_json::Value>,
}
