// Email Builders - Builders for different types of emails
// Each builder knows how to construct its specific email type

use super::types::{
    EmailBuilder, EmailError, EmailMessage, PasswordChangedEmailData, PasswordResetEmailData,
    VerificationEmailData, WelcomeEmailData,
};
use crate::app_config::EmailConfig;
use handlebars::Handlebars;
use tracing::instrument;

/// Builder for verification emails with 6-digit codes
pub struct VerificationEmailBuilder<'a> {
    to_email: &'a str,
    user_name: &'a str,
    code: &'a str,
    config: &'a EmailConfig,
    templates: &'a Handlebars<'a>,
}

impl<'a> VerificationEmailBuilder<'a> {
    pub fn new(
        to_email: &'a str,
        user_name: &'a str,
        code: &'a str,
        config: &'a EmailConfig,
        templates: &'a Handlebars<'a>,
    ) -> Self {
        Self {
            to_email,
            user_name,
            code,
            config,
            templates,
        }
    }
}

impl<'a> EmailBuilder for VerificationEmailBuilder<'a> {
    #[instrument(skip(self))]
    fn build(&self) -> Result<EmailMessage, EmailError> {
        // Prepare template data
        let data = VerificationEmailData {
            code: self.code.to_string(),
            user_name: self.user_name.to_string(),
            user_email: self.to_email.to_string(),
            app_name: self.config.from_name.clone(),
            app_url: self.config.frontend_url.clone(),
            support_email: self.config.support_email.clone(),
            expiry_minutes: (self.config.verification_code_ttl / 60) as u32,
        };

        // Render HTML content
        let html = self
            .templates
            .render("verification", &data)
            .map_err(|e| EmailError::TemplateError(e.to_string()))?;

        // Create plain text version
        let text = format!(
            "Hi {},\n\n\
            Your verification code is: {}\n\n\
            This code will expire in {} minutes.\n\n\
            If you didn't request this code, please ignore this email.\n\n\
            Best regards,\n\
            The {} Team",
            self.user_name, self.code, data.expiry_minutes, self.config.from_name
        );

        // Build email message
        Ok(EmailMessage::new(
            format!("{} <{}>", self.config.from_name, self.config.from_email),
            vec![self.to_email.to_string()],
            format!(
                "Your {} verification code: {}",
                self.config.from_name, self.code
            ),
            html,
        )
        .with_text(text))
    }
}

/// Builder for password reset emails with secure tokens
pub struct PasswordResetEmailBuilder<'a> {
    to_email: &'a str,
    user_name: &'a str,
    reset_token: &'a str,
    config: &'a EmailConfig,
    templates: &'a Handlebars<'a>,
}

impl<'a> PasswordResetEmailBuilder<'a> {
    pub fn new(
        to_email: &'a str,
        user_name: &'a str,
        reset_token: &'a str,
        config: &'a EmailConfig,
        templates: &'a Handlebars<'a>,
    ) -> Self {
        Self {
            to_email,
            user_name,
            reset_token,
            config,
            templates,
        }
    }
}

impl<'a> EmailBuilder for PasswordResetEmailBuilder<'a> {
    #[instrument(skip(self))]
    fn build(&self) -> Result<EmailMessage, EmailError> {
        // Construct reset URL
        let reset_url = format!(
            "{}/reset-password?token={}",
            self.config.frontend_url, self.reset_token
        );

        // Prepare template data
        let data = PasswordResetEmailData {
            reset_url: reset_url.clone(),
            user_name: self.user_name.to_string(),
            app_name: self.config.from_name.clone(),
            app_url: self.config.frontend_url.clone(),
            support_email: self.config.support_email.clone(),
            expiry_minutes: 15, // Password reset tokens expire in 15 minutes
        };

        // Render HTML content
        let html = self
            .templates
            .render("password_reset", &data)
            .map_err(|e| EmailError::TemplateError(e.to_string()))?;

        // Create plain text version
        let text = format!(
            "Hi {},\n\n\
            We received a request to reset your password. Click the link below to set a new password:\n\n\
            {}\n\n\
            This link will expire in {} minutes.\n\n\
            If you didn't request this, please ignore this email. Your password won't be changed.\n\n\
            Best regards,\n\
            The {} Team\n\n\
            P.S. For security reasons, this link can only be used once.",
            self.user_name, reset_url, data.expiry_minutes, self.config.from_name
        );

        // Build email message
        Ok(EmailMessage::new(
            format!("{} <{}>", self.config.from_name, self.config.from_email),
            vec![self.to_email.to_string()],
            format!("Password Reset Request - {}", self.config.from_name),
            html,
        )
        .with_text(text))
    }
}

/// Builder for welcome emails after successful registration
pub struct WelcomeEmailBuilder<'a> {
    to_email: &'a str,
    user_name: &'a str,
    config: &'a EmailConfig,
    templates: &'a Handlebars<'a>,
}

impl<'a> WelcomeEmailBuilder<'a> {
    pub fn new(
        to_email: &'a str,
        user_name: &'a str,
        config: &'a EmailConfig,
        templates: &'a Handlebars<'a>,
    ) -> Self {
        Self {
            to_email,
            user_name,
            config,
            templates,
        }
    }
}

impl<'a> EmailBuilder for WelcomeEmailBuilder<'a> {
    #[instrument(skip(self))]
    fn build(&self) -> Result<EmailMessage, EmailError> {
        // Prepare template data
        let data = WelcomeEmailData {
            user_name: self.user_name.to_string(),
            app_name: self.config.from_name.clone(),
            app_url: self.config.frontend_url.clone(),
            support_email: self.config.support_email.clone(),
        };

        // Render HTML content
        let html = self
            .templates
            .render("welcome", &data)
            .map_err(|e| EmailError::TemplateError(e.to_string()))?;

        // Create plain text version
        let text = format!(
            "Welcome to {}, {}!\n\n\
            Thank you for joining us. Your account has been successfully created and verified.\n\n\
            You can now access all features of our platform at:\n\
            {}\n\n\
            If you have any questions, feel free to contact us at {}.\n\n\
            Best regards,\n\
            The {} Team",
            self.config.from_name,
            self.user_name,
            self.config.frontend_url,
            self.config.support_email,
            self.config.from_name
        );

        // Build email message
        Ok(EmailMessage::new(
            format!("{} <{}>", self.config.from_name, self.config.from_email),
            vec![self.to_email.to_string()],
            format!("Welcome to {}!", self.config.from_name),
            html,
        )
        .with_text(text))
    }
}

/// Builder for password change security notification emails
pub struct PasswordChangedEmailBuilder<'a> {
    to_email: &'a str,
    user_name: &'a str,
    ip_address: &'a str,
    user_agent: &'a str,
    config: &'a EmailConfig,
    templates: &'a Handlebars<'a>,
}

impl<'a> PasswordChangedEmailBuilder<'a> {
    pub fn new(
        to_email: &'a str,
        user_name: &'a str,
        ip_address: &'a str,
        user_agent: &'a str,
        config: &'a EmailConfig,
        templates: &'a Handlebars<'a>,
    ) -> Self {
        Self {
            to_email,
            user_name,
            ip_address,
            user_agent,
            config,
            templates,
        }
    }
}

impl<'a> EmailBuilder for PasswordChangedEmailBuilder<'a> {
    #[instrument(skip(self))]
    fn build(&self) -> Result<EmailMessage, EmailError> {
        // Prepare template data
        let data = PasswordChangedEmailData {
            user_name: self.user_name.to_string(),
            ip_address: self.ip_address.to_string(),
            user_agent: self.user_agent.to_string(),
            timestamp: chrono::Utc::now()
                .format("%B %d, %Y at %H:%M UTC")
                .to_string(),
            app_name: self.config.from_name.clone(),
            app_url: self.config.frontend_url.clone(),
            support_email: self.config.support_email.clone(),
        };

        // Render HTML content
        let html = self
            .templates
            .render("password_changed", &data)
            .map_err(|e| EmailError::TemplateError(e.to_string()))?;

        // Create plain text version
        let text = format!(
            "Security Alert: Your Password Was Changed\n\n\
            Hi {},\n\n\
            Your password was successfully changed on {}.\n\n\
            Details:\n\
            - IP Address: {}\n\
            - Device: {}\n\n\
            If you made this change, you can safely ignore this email.\n\n\
            If you did NOT make this change:\n\
            1. Reset your password immediately at {}/forgot-password\n\
            2. Review your account for any unauthorized activity\n\
            3. Contact our support team at {}\n\n\
            Best regards,\n\
            The {} Security Team",
            self.user_name,
            data.timestamp,
            self.ip_address,
            self.user_agent,
            self.config.frontend_url,
            self.config.support_email,
            self.config.from_name
        );

        // Build email message - Security alerts should have high priority
        Ok(EmailMessage::new(
            format!("{} <{}>", self.config.from_name, self.config.from_email),
            vec![self.to_email.to_string()],
            format!(
                "{} Security Alert: Your password was changed",
                self.config.from_name
            ),
            html,
        )
        .with_text(text)
        .with_reply_to(self.config.support_email.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_config() -> EmailConfig {
        EmailConfig {
            provider: crate::app_config::EmailProvider::Resend,
            resend_api_key: "test_key".to_string(),
            resend_api_url: "https://api.resend.com/emails".to_string(),
            from_email: "noreply@example.com".to_string(),
            from_name: "Test App".to_string(),
            support_email: "support@example.com".to_string(),
            frontend_url: "https://app.example.com".to_string(),
            dashboard_url: "https://dashboard.example.com".to_string(),
            verification_code_ttl: 900,
            verification_max_attempts: 5,
            resend_limit: 10,
            resend_window: 3600,
            min_resend_cooldown: 60,
        }
    }

    fn setup_test_templates() -> Handlebars<'static> {
        let mut templates = Handlebars::new();
        // Register dummy templates for testing
        templates
            .register_template_string("verification", "Verification: {{code}}")
            .unwrap();
        templates
            .register_template_string("password_reset", "Reset: {{reset_url}}")
            .unwrap();
        templates
            .register_template_string("welcome", "Welcome {{user_name}}!")
            .unwrap();
        templates
            .register_template_string("password_changed", "Password changed from {{ip_address}}")
            .unwrap();
        templates
    }

    #[test]
    fn test_verification_email_builder() {
        let config = setup_test_config();
        let templates = setup_test_templates();
        let builder = VerificationEmailBuilder::new(
            "user@example.com",
            "John Doe",
            "123456",
            &config,
            &templates,
        );

        let message = builder.build().unwrap();
        assert_eq!(message.to, vec!["user@example.com"]);
        assert_eq!(message.subject, "Your Test App verification code: 123456");
        assert!(message.text.is_some());
    }

    #[test]
    fn test_password_reset_email_builder() {
        let config = setup_test_config();
        let templates = setup_test_templates();
        let builder = PasswordResetEmailBuilder::new(
            "user@example.com",
            "John Doe",
            "reset_token_123",
            &config,
            &templates,
        );

        let message = builder.build().unwrap();
        assert_eq!(message.to, vec!["user@example.com"]);
        assert_eq!(message.subject, "Password Reset Request - Test App");
        assert!(message.text.unwrap().contains("reset_token_123"));
    }

    #[test]
    fn test_welcome_email_builder() {
        let config = setup_test_config();
        let templates = setup_test_templates();
        let builder = WelcomeEmailBuilder::new("user@example.com", "John Doe", &config, &templates);

        let message = builder.build().unwrap();
        assert_eq!(message.to, vec!["user@example.com"]);
        assert_eq!(message.subject, "Welcome to Test App!");
        assert!(message.text.is_some());
    }

    #[test]
    fn test_password_changed_email_builder() {
        let config = setup_test_config();
        let templates = setup_test_templates();
        let builder = PasswordChangedEmailBuilder::new(
            "user@example.com",
            "John Doe",
            "192.168.1.1",
            "Mozilla/5.0",
            &config,
            &templates,
        );

        let message = builder.build().unwrap();
        assert_eq!(message.to, vec!["user@example.com"]);
        assert_eq!(
            message.subject,
            "Test App Security Alert: Your password was changed"
        );
        assert_eq!(message.reply_to, Some("support@example.com".to_string()));
    }
}
