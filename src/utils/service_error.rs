// DEV-114: Service Error type as specified in requirements
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found")]
    NotFound,

    #[error("Alias already exists")]
    AliasAlreadyExists,

    #[error("Link expired")]
    Expired,

    #[error("Link inactive")]
    Inactive,

    #[error("Subscription limit exceeded")]
    SubscriptionLimitExceeded(String),

    #[error("Too many links")]
    TooManyLinks,

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal server error")]
    InternalError,

    #[error("Security blocked: {0}")]
    SecurityBlocked(String),

    #[error("Password required")]
    PasswordRequired,
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServiceError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServiceError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            ServiceError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".to_string()),
            ServiceError::AliasAlreadyExists => {
                (StatusCode::CONFLICT, "Alias already exists".to_string())
            },
            ServiceError::Expired => (StatusCode::GONE, "Link has expired".to_string()),
            ServiceError::Inactive => (StatusCode::GONE, "Link is inactive".to_string()),
            ServiceError::SubscriptionLimitExceeded(msg) => (StatusCode::PAYMENT_REQUIRED, msg),
            ServiceError::TooManyLinks => (
                StatusCode::BAD_REQUEST,
                "Too many links in request".to_string(),
            ),
            ServiceError::CacheError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServiceError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ServiceError::InternalError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            ServiceError::SecurityBlocked(msg) => (StatusCode::FORBIDDEN, msg),
            ServiceError::PasswordRequired => {
                (StatusCode::UNAUTHORIZED, "Password required".to_string())
            },
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

// Conversion from various error types
impl From<diesel::result::Error> for ServiceError {
    fn from(error: diesel::result::Error) -> Self {
        match error {
            diesel::result::Error::NotFound => ServiceError::NotFound,
            _ => ServiceError::DatabaseError(error.to_string()),
        }
    }
}

impl From<redis::RedisError> for ServiceError {
    fn from(error: redis::RedisError) -> Self {
        ServiceError::CacheError(error.to_string())
    }
}

impl From<validator::ValidationErrors> for ServiceError {
    fn from(error: validator::ValidationErrors) -> Self {
        ServiceError::ValidationError(error.to_string())
    }
}

impl From<crate::utils::url_validator::UrlValidationError> for ServiceError {
    fn from(error: crate::utils::url_validator::UrlValidationError) -> Self {
        ServiceError::ValidationError(error.to_string())
    }
}

impl From<crate::services::short_code::ShortCodeError> for ServiceError {
    fn from(error: crate::services::short_code::ShortCodeError) -> Self {
        match error {
            crate::services::short_code::ShortCodeError::DatabaseError(e) => {
                ServiceError::DatabaseError(e.to_string())
            },
            crate::services::short_code::ShortCodeError::InvalidCustomAlias { reason, .. } => {
                ServiceError::ValidationError(reason)
            },
            crate::services::short_code::ShortCodeError::AliasAlreadyExists { .. } => {
                ServiceError::AliasAlreadyExists
            },
            _ => ServiceError::InternalError,
        }
    }
}
