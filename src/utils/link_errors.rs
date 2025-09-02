// DEV-68: Link Management API - Error handling
// Comprehensive error types for link operations

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

use crate::{services::short_code::ShortCodeError, utils::url_validator::UrlValidationError};

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Error, Debug)]
pub enum LinkError {
    #[error("Invalid URL format: {0}")]
    InvalidUrl(String),

    #[error("URL blocked for security reasons: {0}")]
    SecurityBlocked(String),

    #[error("Custom alias already exists: {0}")]
    AliasExists(String),

    #[error("Custom alias is reserved: {0}")]
    ReservedAlias(String),

    #[error("Invalid custom alias format: {0}")]
    InvalidAlias(String),

    #[error("Rate limit exceeded. Try again in {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Link not found")]
    NotFound,

    #[error("Link has expired")]
    Expired,

    #[error("Link is password protected")]
    PasswordRequired,

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Link is inactive")]
    Inactive,

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Metadata extraction failed: {0}")]
    MetadataExtractionError(String),

    #[error("Subscription limit exceeded: {0}")]
    SubscriptionLimitExceeded(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal server error")]
    InternalError,

    #[error("Service unavailable")]
    ServiceUnavailable,
}

// =============================================================================
// ERROR CONVERSIONS
// =============================================================================

impl From<ShortCodeError> for LinkError {
    fn from(err: ShortCodeError) -> Self {
        match err {
            ShortCodeError::AliasAlreadyExists(alias) => LinkError::AliasExists(alias),
            ShortCodeError::ReservedAlias(alias) => LinkError::ReservedAlias(alias),
            ShortCodeError::InvalidCustomAlias { reason, alias } => {
                LinkError::InvalidAlias(format!("{}: {}", alias, reason))
            },
            ShortCodeError::MaxRetriesExceeded => LinkError::ServiceUnavailable,
            ShortCodeError::DatabaseError(e) => LinkError::DatabaseError(e.to_string()),
            _ => LinkError::InternalError,
        }
    }
}

impl From<UrlValidationError> for LinkError {
    fn from(err: UrlValidationError) -> Self {
        match err {
            UrlValidationError::InvalidFormat(msg) => LinkError::InvalidUrl(msg),
            UrlValidationError::BlacklistedDomain(domain) => {
                LinkError::SecurityBlocked(format!("Domain {} is blacklisted", domain))
            },
            UrlValidationError::SuspiciousPattern(pattern) => {
                LinkError::SecurityBlocked(format!("Suspicious pattern detected: {}", pattern))
            },
            UrlValidationError::PrivateNetwork(ip) => {
                LinkError::SecurityBlocked(format!("Private network address not allowed: {}", ip))
            },
            _ => LinkError::InvalidUrl(err.to_string()),
        }
    }
}

impl From<crate::utils::ValidationError> for LinkError {
    fn from(err: crate::utils::ValidationError) -> Self {
        match err {
            crate::utils::ValidationError::InvalidFormat(msg) => LinkError::InvalidUrl(msg),
            crate::utils::ValidationError::BlockedDomain(domain) => {
                LinkError::SecurityBlocked(format!("Domain {} is blocked", domain))
            },
            crate::utils::ValidationError::BlockedTld(tld) => {
                LinkError::SecurityBlocked(format!("TLD {} is blocked", tld))
            },
            crate::utils::ValidationError::PrivateIp => {
                LinkError::SecurityBlocked("Private IP addresses are not allowed".to_string())
            },
            crate::utils::ValidationError::SuspiciousCharacters => {
                LinkError::SecurityBlocked("URL contains suspicious characters".to_string())
            },
            crate::utils::ValidationError::DataUrlNotAllowed => {
                LinkError::SecurityBlocked("Data URLs are not allowed".to_string())
            },
            crate::utils::ValidationError::JavascriptUrlNotAllowed => {
                LinkError::SecurityBlocked("JavaScript URLs are not allowed".to_string())
            },
            crate::utils::ValidationError::TooLong { max, current } => LinkError::InvalidUrl(
                format!("URL too long: {} characters (max {})", current, max),
            ),
            _ => LinkError::InvalidUrl(err.to_string()),
        }
    }
}

impl From<diesel::result::Error> for LinkError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::NotFound => LinkError::NotFound,
            _ => LinkError::DatabaseError(err.to_string()),
        }
    }
}

impl From<validator::ValidationErrors> for LinkError {
    fn from(err: validator::ValidationErrors) -> Self {
        let messages: Vec<String> = err
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                errors
                    .iter()
                    .map(move |e| format!("{}: {}", field, e.message.as_ref().unwrap_or(&e.code)))
            })
            .collect();

        LinkError::ValidationError(messages.join(", "))
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================

#[derive(Debug, Serialize)]
pub struct LinkErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl LinkError {
    /// Get HTTP status code for error
    pub fn status_code(&self) -> StatusCode {
        match self {
            LinkError::InvalidUrl(_) | LinkError::InvalidAlias(_) | LinkError::BadRequest(_) => {
                StatusCode::BAD_REQUEST
            },

            LinkError::Unauthorized => StatusCode::UNAUTHORIZED,

            LinkError::Forbidden(_) | LinkError::SecurityBlocked(_) => StatusCode::FORBIDDEN,

            LinkError::NotFound => StatusCode::NOT_FOUND,

            LinkError::AliasExists(_) | LinkError::ReservedAlias(_) => StatusCode::CONFLICT,

            LinkError::Expired => StatusCode::GONE,

            LinkError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,

            LinkError::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,

            LinkError::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,

            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get error code for API response
    pub fn error_code(&self) -> &'static str {
        match self {
            LinkError::InvalidUrl(_) => "INVALID_URL",
            LinkError::SecurityBlocked(_) => "SECURITY_BLOCKED",
            LinkError::AliasExists(_) => "ALIAS_EXISTS",
            LinkError::ReservedAlias(_) => "RESERVED_ALIAS",
            LinkError::InvalidAlias(_) => "INVALID_ALIAS",
            LinkError::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
            LinkError::NotFound => "NOT_FOUND",
            LinkError::Expired => "LINK_EXPIRED",
            LinkError::PasswordRequired => "PASSWORD_REQUIRED",
            LinkError::InvalidPassword => "INVALID_PASSWORD",
            LinkError::Inactive => "LINK_INACTIVE",
            LinkError::Unauthorized => "UNAUTHORIZED",
            LinkError::Forbidden(_) => "FORBIDDEN",
            LinkError::ValidationError(_) => "VALIDATION_ERROR",
            LinkError::DatabaseError(_) => "DATABASE_ERROR",
            LinkError::CacheError(_) => "CACHE_ERROR",
            LinkError::MetadataExtractionError(_) => "METADATA_EXTRACTION_ERROR",
            LinkError::SubscriptionLimitExceeded(_) => "SUBSCRIPTION_LIMIT_EXCEEDED",
            LinkError::BadRequest(_) => "BAD_REQUEST",
            LinkError::InternalError => "INTERNAL_ERROR",
            LinkError::ServiceUnavailable => "SERVICE_UNAVAILABLE",
        }
    }

    /// Create error response
    pub fn to_response(&self) -> LinkErrorResponse {
        let details = match self {
            LinkError::RateLimitExceeded { retry_after } => {
                Some(serde_json::json!({ "retry_after": retry_after }))
            },
            LinkError::ValidationError(msg) => {
                Some(serde_json::json!({ "validation_errors": msg }))
            },
            _ => None,
        };

        LinkErrorResponse {
            error: self.to_string(),
            code: self.error_code().to_string(),
            details,
        }
    }
}

impl IntoResponse for LinkError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = self.to_response();

        (status, Json(body)).into_response()
    }
}

// =============================================================================
// RESULT TYPE
// =============================================================================

pub type LinkResult<T> = Result<T, LinkError>;

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(LinkError::NotFound.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(
            LinkError::InvalidUrl("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            LinkError::AliasExists("test".to_string()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            LinkError::RateLimitExceeded { retry_after: 60 }.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            LinkError::Unauthorized.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            LinkError::ServiceUnavailable.status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(LinkError::NotFound.error_code(), "NOT_FOUND");
        assert_eq!(
            LinkError::InvalidUrl("test".to_string()).error_code(),
            "INVALID_URL"
        );
        assert_eq!(
            LinkError::SecurityBlocked("test".to_string()).error_code(),
            "SECURITY_BLOCKED"
        );
        assert_eq!(
            LinkError::RateLimitExceeded { retry_after: 60 }.error_code(),
            "RATE_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn test_error_response() {
        let error = LinkError::RateLimitExceeded { retry_after: 60 };
        let response = error.to_response();

        assert_eq!(response.code, "RATE_LIMIT_EXCEEDED");
        assert!(response.details.is_some());

        let details = response.details.unwrap();
        assert_eq!(details["retry_after"], 60);
    }

    #[test]
    fn test_from_short_code_error() {
        let short_code_err = ShortCodeError::AliasAlreadyExists("test".to_string());
        let link_err: LinkError = short_code_err.into();

        assert!(matches!(link_err, LinkError::AliasExists(_)));
    }

    #[test]
    fn test_from_url_validation_error() {
        let url_err = UrlValidationError::InvalidFormat("bad url".to_string());
        let link_err: LinkError = url_err.into();

        assert!(matches!(link_err, LinkError::InvalidUrl(_)));
    }
}
