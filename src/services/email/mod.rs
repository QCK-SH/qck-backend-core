// Email Service Module - Refactored for better separation of concerns
// Main orchestration module that coordinates builders and sender

pub mod builders;
pub mod sender;
pub mod types;

use self::types::EmailBuilder;
use crate::app_config::EmailConfig;
use anyhow::Result;
use builders::{
    PasswordChangedEmailBuilder, PasswordResetEmailBuilder,
};
use handlebars::Handlebars;
use rand::Rng;
use sender::EmailSender;
use std::sync::Arc;
use tracing::{info, instrument};

/// Email service for sending various types of emails
#[derive(Clone)]
pub struct EmailService {
    sender: EmailSender,
    config: EmailConfig,
    templates: Arc<Handlebars<'static>>,
}

impl EmailService {
    /// Create a new email service instance
    pub fn new(config: EmailConfig) -> Result<Self> {
        // Initialize templates
        let mut templates = Handlebars::new();

        // Register all email templates
        Self::register_templates(&mut templates)?;

        // Create the email sender with API URL from config
        let sender =
            EmailSender::new_resend(config.resend_api_key.clone(), config.resend_api_url.clone())
                .with_max_retries(3)
                .with_retry_delay(std::time::Duration::from_secs(1));

        Ok(Self {
            sender,
            config,
            templates: Arc::new(templates),
        })
    }

    /// Register all email templates
    fn register_templates(templates: &mut Handlebars) -> Result<(), types::EmailError> {
        // Register password reset email template
        let password_reset_template = include_str!("../../templates/email/password_reset.html");
        templates
            .register_template_string("password_reset", password_reset_template)
            .map_err(|e| types::EmailError::TemplateError(e.to_string()))?;

        // Register password changed notification template
        let password_changed_template = include_str!("../../templates/email/password_changed.html");
        templates
            .register_template_string("password_changed", password_changed_template)
            .map_err(|e| types::EmailError::TemplateError(e.to_string()))?;

        Ok(())
    }

    /// Generate a random 6-digit verification code
    pub fn generate_verification_code() -> String {
        let mut rng = rand::thread_rng();
        let code: u32 = rng.gen_range(100000..999999);
        code.to_string()
    }

    /// Send password reset email with secure token
    #[instrument(skip(self))]
    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        user_name: &str,
        reset_token: &str,
    ) -> Result<(), types::EmailError> {
        info!("Sending password reset email to {}", to_email);

        let builder = PasswordResetEmailBuilder::new(
            to_email,
            user_name,
            reset_token,
            &self.config,
            &self.templates,
        );

        let message = builder.build()?;
        self.sender.send_with_retry(message).await
    }

    /// Send password change security notification
    #[instrument(skip(self))]
    pub async fn send_password_change_notification(
        &self,
        to_email: &str,
        user_name: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<(), types::EmailError> {
        info!("Sending password change notification to {}", to_email);

        let builder = PasswordChangedEmailBuilder::new(
            to_email,
            user_name,
            ip_address,
            user_agent,
            &self.config,
            &self.templates,
        );

        let message = builder.build()?;
        // Security notifications should be sent immediately without retry
        self.sender.send(message).await
    }

    /// Perform a health check on the email service
    pub async fn health_check(&self) -> Result<(), EmailError> {
        self.sender.health_check().await
    }
}

// Re-export commonly used types for convenience
pub use types::{EmailError, EmailMessage};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_config::EmailProvider;

    fn create_test_config() -> EmailConfig {
        EmailConfig {
            provider: EmailProvider::Resend,
            resend_api_key: "test_key".to_string(),
            resend_api_url: "https://api.resend.com/emails".to_string(),
            from_email: "noreply@test.com".to_string(),
            from_name: "Test App".to_string(),
            support_email: "support@test.com".to_string(),
            frontend_url: "https://app.test.com".to_string(),
            dashboard_url: "https://dashboard.test.com".to_string(),
            verification_code_ttl: 900,
            verification_max_attempts: 5,
            resend_limit: 10,
            resend_window: 3600,
            min_resend_cooldown: 60,
        }
    }

    #[test]
    fn test_email_service_creation() {
        let config = create_test_config();
        let service = EmailService::new(config);
        assert!(service.is_ok());
    }

    #[test]
    fn test_verification_code_generation() {
        let code = EmailService::generate_verification_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_verification_code_range() {
        for _ in 0..100 {
            let code = EmailService::generate_verification_code();
            let num: u32 = code.parse().unwrap();
            assert!(num >= 100000 && num <= 999999);
        }
    }
}
