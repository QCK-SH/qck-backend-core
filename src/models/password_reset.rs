use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::password_reset_tokens;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = password_reset_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PasswordResetToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = password_reset_tokens)]
pub struct NewPasswordResetToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl NewPasswordResetToken {
    pub fn new(
        user_id: Uuid,
        token_hash: String,
        expires_at: DateTime<Utc>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            user_id,
            token_hash,
            expires_at,
            ip_address,
            user_agent,
        }
    }
}

// Request/Response models for API
#[derive(Debug, Serialize, Deserialize, validator::Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "Please provide a valid email address"))]
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, validator::Validate)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 32, max = 64, message = "Invalid reset token format"))]
    pub token: String,

    #[validate(length(
        min = 8,
        max = 128,
        message = "Password must be between 8 and 128 characters"
    ))]
    pub new_password: String,

    pub confirm_password: String,
}

impl ResetPasswordRequest {
    /// Validate that passwords match
    pub fn validate_passwords_match(&self) -> Result<(), String> {
        if self.new_password != self.confirm_password {
            return Err("Passwords do not match".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ForgotPasswordResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ResetPasswordResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
