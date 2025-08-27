// Email Service Types - Shared types and structures for email module
// This module contains all shared types used across the email service

use serde::Serialize;
use thiserror::Error;

/// Errors that can occur during email operations
#[derive(Error, Debug)]
pub enum EmailError {
    #[error("Failed to send email: {0}")]
    SendError(String),

    #[error("Template rendering error: {0}")]
    TemplateError(String),

    #[error("Invalid email address: {0}")]
    InvalidEmail(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Generic email message structure that can be sent
#[derive(Debug, Clone, Serialize)]
pub struct EmailMessage {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub html: String,
    pub text: Option<String>,
    pub reply_to: Option<String>,
}

impl EmailMessage {
    pub fn new(from: String, to: Vec<String>, subject: String, html: String) -> Self {
        Self {
            from,
            to,
            subject,
            html,
            text: None,
            reply_to: None,
        }
    }

    pub fn with_text(mut self, text: String) -> Self {
        self.text = Some(text);
        self
    }

    pub fn with_reply_to(mut self, reply_to: String) -> Self {
        self.reply_to = Some(reply_to);
        self
    }
}

/// Trait that all email builders must implement
pub trait EmailBuilder {
    /// Build the email message
    fn build(&self) -> Result<EmailMessage, EmailError>;
}

/// Data structure for verification email template
#[derive(Serialize)]
pub struct VerificationEmailData {
    pub code: String,
    pub user_name: String,
    pub user_email: String,
    pub app_name: String,
    pub app_url: String,
    pub support_email: String,
    pub expiry_minutes: u32,
}

/// Data structure for password reset email template
#[derive(Serialize)]
pub struct PasswordResetEmailData {
    pub reset_url: String,
    pub user_name: String,
    pub app_name: String,
    pub app_url: String,
    pub support_email: String,
    pub expiry_minutes: u32,
}

/// Data structure for welcome email template
#[derive(Serialize)]
pub struct WelcomeEmailData {
    pub user_name: String,
    pub app_name: String,
    pub app_url: String,
    pub support_email: String,
}

/// Data structure for password change notification template
#[derive(Serialize)]
pub struct PasswordChangedEmailData {
    pub user_name: String,
    pub ip_address: String,
    pub user_agent: String,
    pub timestamp: String,
    pub app_name: String,
    pub app_url: String,
    pub support_email: String,
}

/// Resend API specific email format
///
/// This struct represents the email payload sent to the Resend API.
/// Optional fields (`text` and `reply_to`) are omitted from the JSON payload
/// when they are `None`, reducing payload size and avoiding sending null values.
///
/// # Fields
/// - `from`: Sender email address (required)
/// - `to`: List of recipient email addresses (required)
/// - `subject`: Email subject line (required)
/// - `html`: HTML content of the email (required)
/// - `text`: Optional plain text version of the email. Omitted from API payload when None.
/// - `reply_to`: Optional reply-to email address. Omitted from API payload when None.
#[derive(Debug, Serialize)]
pub struct ResendEmailPayload {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

impl From<EmailMessage> for ResendEmailPayload {
    fn from(message: EmailMessage) -> Self {
        Self {
            from: message.from,
            to: message.to,
            subject: message.subject,
            html: message.html,
            text: message.text,
            reply_to: message.reply_to,
        }
    }
}
