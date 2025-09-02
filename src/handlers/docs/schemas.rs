// OpenAPI schema definitions

use super::onboarding;
use serde_json::json;
use utoipa::OpenApi;

// Import utoipa-generated schemas for Link CRUD operations
use crate::models::link::{
    CreateLinkRequest, Link, LinkFilter, LinkListResponse, LinkMetadata, LinkPagination,
    LinkResponse, UpdateLinkRequest,
};

/// Define utoipa OpenAPI document for Link CRUD operations
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::links::create_link,
        crate::handlers::links::get_link,
        crate::handlers::links::update_link,
        crate::handlers::links::delete_link,
        crate::handlers::links::list_links,
        crate::handlers::links::get_link_stats,
        crate::handlers::links::bulk_create_links,
    ),
    components(
        schemas(
            CreateLinkRequest,
            UpdateLinkRequest,
            LinkResponse,
            LinkListResponse,
            LinkPagination,
            LinkFilter,
            LinkMetadata,
            Link,
        )
    ),
    tags(
        (name = "Links", description = "Link management endpoints")
    )
)]
struct LinkApiDoc;

/// Return all schema definitions including utoipa-generated ones
pub fn all_schemas() -> serde_json::Value {
    let mut schemas = json!({
        "RegisterRequest": register_request_schema(),
        "RegisterResponse": register_response_schema(),
        "LoginRequest": login_request_schema(),
        "LoginResponse": login_response_schema(),
        "LoginUserInfo": login_user_info_schema(),
        "RefreshTokenRequest": refresh_token_request_schema(),
        "RefreshTokenResponse": refresh_token_response_schema(),
        "AuthError": auth_error_schema(),
        "VerifyEmailRequest": verify_email_request_schema(),
        "VerifyEmailResponse": verify_email_response_schema(),
        "ResendVerificationRequest": resend_verification_request_schema(),
        "ResendVerificationResponse": resend_verification_response_schema(),
        "VerificationStatusResponse": verification_status_response_schema(),
        "ForgotPasswordRequest": forgot_password_request_schema(),
        "ForgotPasswordResponse": forgot_password_response_schema(),
        "ResetPasswordRequest": reset_password_request_schema(),
        "ResetPasswordResponse": reset_password_response_schema(),
    });

    // Merge utoipa-generated schemas for Link operations
    let openapi = LinkApiDoc::openapi();
    if let Some(components) = openapi.components {
        if let serde_json::Value::Object(ref mut map) = schemas {
            for (key, schema) in components.schemas {
                map.insert(key, serde_json::to_value(schema).unwrap_or_default());
            }
        }
    }

    // Merge onboarding schemas
    if let serde_json::Value::Object(ref mut map) = schemas {
        if let serde_json::Value::Object(onboarding_map) = onboarding::onboarding_schemas() {
            for (key, value) in onboarding_map {
                map.insert(key, value);
            }
        }
    }

    schemas
}

fn register_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["email", "password", "password_confirmation", "full_name", "accept_terms"],
        "properties": {
            "email": {
                "type": "string",
                "format": "email",
                "maxLength": 320,
                "description": "User's email address (stored in lowercase)"
            },
            "password": {
                "type": "string",
                "format": "password",
                "minLength": 8,
                "description": "Password with uppercase, lowercase, number, and special character"
            },
            "password_confirmation": {
                "type": "string",
                "format": "password",
                "description": "Must match the password field"
            },
            "full_name": {
                "type": "string",
                "minLength": 1,
                "maxLength": 255,
                "description": "User's full name"
            },
            "company_name": {
                "type": "string",
                "maxLength": 255,
                "nullable": true,
                "description": "User's company name (optional)"
            },
            "accept_terms": {
                "type": "boolean",
                "description": "User must accept terms and conditions"
            }
        }
    })
}

fn register_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "user_id": {
                "type": "string",
                "format": "uuid"
            },
            "email": {
                "type": "string",
                "format": "email"
            },
            "full_name": {
                "type": "string",
                "description": "User's full name"
            },
            "company_name": {
                "type": "string",
                "nullable": true,
                "description": "User's company name"
            },
            "email_verification_required": {
                "type": "boolean"
            },
            "verification_sent": {
                "type": "boolean",
                "description": "Whether verification email was sent"
            },
            "onboarding_status": {
                "type": "string",
                "description": "Current onboarding status",
                "enum": ["registered", "verified", "plan_selected", "payment_pending", "completed"]
            },
            "message": {
                "type": "string"
            }
        }
    })
}

fn login_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["email", "password"],
        "properties": {
            "email": {
                "type": "string",
                "format": "email",
                "description": "User's email address"
            },
            "password": {
                "type": "string",
                "format": "password",
                "description": "User's password"
            },
            "remember_me": {
                "type": "boolean",
                "default": false,
                "description": "Extend refresh token expiry to 30 days"
            }
        }
    })
}

fn login_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "access_token": {
                "type": "string",
                "description": "JWT access token for API requests"
            },
            "refresh_token": {
                "type": "string",
                "description": "JWT refresh token for obtaining new access tokens"
            },
            "expires_in": {
                "type": "integer",
                "description": "Access token expiry time in seconds"
            },
            "token_type": {
                "type": "string",
                "description": "Token type (always 'Bearer')",
                "default": "Bearer"
            },
            "user": {
                "$ref": "#/components/schemas/LoginUserInfo"
            }
        }
    })
}

fn login_user_info_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "format": "uuid",
                "description": "User's unique identifier"
            },
            "email": {
                "type": "string",
                "format": "email",
                "description": "User's email address"
            },
            "full_name": {
                "type": "string",
                "description": "User's full name"
            },
            "subscription_tier": {
                "type": "string",
                "description": "User's subscription tier",
                "enum": ["free", "basic", "pro", "enterprise"]
            },
            "onboarding_status": {
                "type": "string",
                "description": "User's onboarding status",
                "enum": ["registered", "verified", "plan_selected", "payment_pending", "completed"]
            }
        }
    })
}

fn refresh_token_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["refresh_token"],
        "properties": {
            "refresh_token": {
                "type": "string",
                "description": "JWT refresh token obtained from login or previous refresh"
            }
        }
    })
}

fn refresh_token_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "access_token": {
                "type": "string",
                "description": "New JWT access token for API requests"
            },
            "refresh_token": {
                "type": "string",
                "description": "New JWT refresh token (rotated for security)"
            },
            "expires_in": {
                "type": "integer",
                "description": "Access token expiry time in seconds"
            },
            "token_type": {
                "type": "string",
                "description": "Token type (always 'Bearer')",
                "default": "Bearer"
            }
        }
    })
}

fn auth_error_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "success": {
                "type": "boolean",
                "description": "Always false for errors"
            },
            "error": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Error code",
                        "enum": ["INVALID_CREDENTIALS", "ACCOUNT_LOCKED", "EMAIL_NOT_VERIFIED", "ACCOUNT_INACTIVE", "RATE_LIMITED", "DATABASE_ERROR", "TOKEN_ERROR", "INVALID_TOKEN", "USER_NOT_FOUND", "VALIDATION_ERROR", "INTERNAL_ERROR"]
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable error description"
                    },
                    "retry_after": {
                        "type": "integer",
                        "nullable": true,
                        "description": "Seconds until retry is allowed (for rate limiting and lockout)"
                    }
                }
            },
            "message": {
                "type": "string",
                "description": "Error message"
            }
        }
    })
}

fn verify_email_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["email", "code"],
        "properties": {
            "email": {
                "type": "string",
                "format": "email",
                "description": "Email address to verify"
            },
            "code": {
                "type": "string",
                "pattern": "^[0-9]{6}$",
                "description": "6-digit verification code"
            }
        }
    })
}

fn verify_email_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "success": {
                "type": "boolean",
                "description": "Whether verification was successful"
            },
            "message": {
                "type": "string",
                "description": "Success message"
            },
            "email": {
                "type": "string",
                "format": "email",
                "description": "Verified email address"
            },
            "user_id": {
                "type": "string",
                "format": "uuid",
                "description": "User ID of verified account"
            }
        }
    })
}

fn resend_verification_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["email"],
        "properties": {
            "email": {
                "type": "string",
                "format": "email",
                "description": "Email address to resend verification code to"
            }
        }
    })
}

fn resend_verification_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "success": {
                "type": "boolean",
                "description": "Whether resend was successful"
            },
            "message": {
                "type": "string",
                "description": "Success message"
            },
            "cooldown_seconds": {
                "type": "integer",
                "nullable": true,
                "description": "Seconds until next resend is allowed"
            },
            "remaining_resends": {
                "type": "integer",
                "description": "Number of resends remaining today"
            }
        }
    })
}

fn verification_status_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "email": {
                "type": "string",
                "format": "email",
                "description": "Email address checked"
            },
            "is_verified": {
                "type": "boolean",
                "description": "Whether email is verified"
            },
            "verified_at": {
                "type": "string",
                "format": "date-time",
                "nullable": true,
                "description": "Timestamp when email was verified"
            }
        }
    })
}

fn forgot_password_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["email"],
        "properties": {
            "email": {
                "type": "string",
                "format": "email",
                "maxLength": 320,
                "description": "Email address to send password reset link to"
            }
        }
    })
}

fn forgot_password_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "success": {
                "type": "boolean",
                "description": "Always true for security (even if email doesn't exist)"
            },
            "message": {
                "type": "string",
                "description": "Success message"
            },
            "data": {
                "type": "object",
                "nullable": true,
                "description": "Additional data (currently null)"
            }
        }
    })
}

fn reset_password_request_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["token", "new_password", "confirm_password"],
        "properties": {
            "token": {
                "type": "string",
                "minLength": 32,
                "maxLength": 64,
                "description": "Password reset token from email"
            },
            "new_password": {
                "type": "string",
                "format": "password",
                "minLength": 8,
                "maxLength": 128,
                "description": "New password (8-128 characters)"
            },
            "confirm_password": {
                "type": "string",
                "format": "password",
                "description": "Must match new_password exactly"
            }
        }
    })
}

fn reset_password_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "success": {
                "type": "boolean",
                "description": "Whether password reset was successful"
            },
            "message": {
                "type": "string",
                "description": "Success message"
            },
            "data": {
                "type": "object",
                "nullable": true,
                "description": "Additional data (currently null)"
            }
        }
    })
}
