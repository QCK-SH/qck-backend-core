// Email Sender - Generic email sending functionality
// This module handles the actual sending of emails through email providers

use super::types::{EmailError, EmailMessage, ResendEmailPayload};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

/// Generic email sender that handles delivery to email providers
#[derive(Clone)]
pub struct EmailSender {
    client: Arc<Client>,
    api_key: String,
    api_url: String,
    max_retries: u32,
    retry_delay: Duration,
}

impl EmailSender {
    /// Create a new email sender for Resend API
    pub fn new_resend(api_key: String, api_url: String) -> Self {
        Self {
            client: Arc::new(Client::new()),
            api_key,
            api_url,
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
        }
    }

    /// Create a custom email sender with specific configuration
    pub fn new(api_url: String, api_key: String) -> Self {
        Self {
            client: Arc::new(Client::new()),
            api_key,
            api_url,
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
        }
    }

    /// Set maximum retry attempts
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set delay between retries
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Send an email message
    #[instrument(skip(self, message), fields(to = ?message.to, subject = %message.subject))]
    pub async fn send(&self, message: EmailMessage) -> Result<(), EmailError> {
        let payload: ResendEmailPayload = message.into();

        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(res) if res.status().is_success() => {
                info!("Email sent successfully");
                Ok(())
            },
            Ok(res) => {
                let status = res.status();
                let error_text = res
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());

                error!(
                    "Failed to send email. Status: {}, Error: {}",
                    status, error_text
                );

                // Check for rate limiting
                if status.as_u16() == 429 {
                    Err(EmailError::RateLimitExceeded)
                } else if status.is_server_error() {
                    Err(EmailError::ServiceUnavailable)
                } else {
                    Err(EmailError::SendError(format!(
                        "Email send failed with status {}: {}",
                        status, error_text
                    )))
                }
            },
            Err(e) => {
                error!("Network error while sending email: {:?}", e);
                Err(EmailError::SendError(format!("Network error: {}", e)))
            },
        }
    }

    /// Send an email with automatic retry on failure
    #[instrument(skip(self, message), fields(to = ?message.to, subject = %message.subject))]
    pub async fn send_with_retry(&self, message: EmailMessage) -> Result<(), EmailError> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.send(message.clone()).await {
                Ok(()) => return Ok(()),
                Err(EmailError::RateLimitExceeded) => {
                    warn!("Rate limit hit, not retrying");
                    return Err(EmailError::RateLimitExceeded);
                },
                Err(e) => {
                    warn!("Email send attempt {} failed: {:?}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        // Exponential backoff with jitter to prevent thundering herd
                        // Prevents overflow panics and caps delay at reasonable maximum
                        let max_delay = Duration::from_secs(60); // Cap at 60 seconds
                        let exp = (2_u32).checked_pow(attempt - 1).unwrap_or(u32::MAX);
                        let base_delay = self.retry_delay.checked_mul(exp).unwrap_or(max_delay);
                        let base_delay = if base_delay > max_delay {
                            max_delay
                        } else {
                            base_delay
                        };

                        // Add random jitter (0-25% of base delay) to prevent thundering herd
                        use rand::rngs::StdRng;
                        use rand::{Rng, SeedableRng};
                        let mut rng = StdRng::from_entropy();
                        let jitter_millis = rng.gen_range(0..=(base_delay.as_millis() / 4) as u64);
                        let delay = base_delay + Duration::from_millis(jitter_millis);

                        info!("Retrying in {:?} (with jitter)", delay);
                        tokio::time::sleep(delay).await;
                    }
                },
            }
        }

        Err(last_error.unwrap_or_else(|| {
            EmailError::SendError("Failed after maximum retry attempts".to_string())
        }))
    }

    /// Send multiple emails in batch (useful for newsletters)
    pub async fn send_batch(&self, messages: Vec<EmailMessage>) -> Vec<Result<(), EmailError>> {
        let mut results = Vec::with_capacity(messages.len());

        for message in messages {
            // Add small delay between emails to avoid rate limiting
            tokio::time::sleep(Duration::from_millis(100)).await;
            results.push(self.send(message).await);
        }

        results
    }

    /// Health check for the email service
    pub async fn health_check(&self) -> Result<(), EmailError> {
        // Try to make an authenticated request to check API key validity
        let response = self
            .client
            .get(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await;

        match response {
            Ok(res) if res.status().as_u16() == 401 => {
                Err(EmailError::ConfigError("Invalid API key".to_string()))
            },
            Ok(_) => Ok(()),
            Err(_e) => Err(EmailError::ServiceUnavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_message_builder() {
        let message = EmailMessage::new(
            "sender@example.com".to_string(),
            vec!["recipient@example.com".to_string()],
            "Test Subject".to_string(),
            "<h1>Test</h1>".to_string(),
        )
        .with_text("Test".to_string())
        .with_reply_to("reply@example.com".to_string());

        assert_eq!(message.from, "sender@example.com");
        assert_eq!(message.to, vec!["recipient@example.com"]);
        assert_eq!(message.subject, "Test Subject");
        assert_eq!(message.html, "<h1>Test</h1>");
        assert_eq!(message.text, Some("Test".to_string()));
        assert_eq!(message.reply_to, Some("reply@example.com".to_string()));
    }

    #[test]
    fn test_resend_payload_conversion() {
        let message = EmailMessage::new(
            "sender@example.com".to_string(),
            vec!["recipient@example.com".to_string()],
            "Test Subject".to_string(),
            "<h1>Test</h1>".to_string(),
        );

        let payload: ResendEmailPayload = message.into();
        assert_eq!(payload.from, "sender@example.com");
        assert_eq!(payload.to, vec!["recipient@example.com"]);
        assert_eq!(payload.subject, "Test Subject");
        assert_eq!(payload.html, "<h1>Test</h1>");
        assert!(payload.text.is_none());
        assert!(payload.reply_to.is_none());
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let sender = EmailSender::new_resend(
            "test_key".to_string(),
            "https://api.resend.com/emails".to_string(),
        )
        .with_retry_delay(Duration::from_secs(2));

        // Test normal exponential backoff (should work fine)
        for attempt in 1..=3 {
            let max_delay = Duration::from_secs(60);
            let exp = (2_u32).checked_pow(attempt - 1).unwrap_or(u32::MAX);
            let delay = sender.retry_delay.checked_mul(exp).unwrap_or(max_delay);
            let delay = if delay > max_delay { max_delay } else { delay };

            match attempt {
                1 => assert_eq!(delay, Duration::from_secs(2)), // 2 * 2^0 = 2 * 1 = 2
                2 => assert_eq!(delay, Duration::from_secs(4)), // 2 * 2^1 = 2 * 2 = 4
                3 => assert_eq!(delay, Duration::from_secs(8)), // 2 * 2^2 = 2 * 4 = 8
                _ => unreachable!(),
            }
        }

        // Test overflow protection - simulate high attempt number
        let high_attempt = 50; // Would cause 2^49 which overflows u32
        let exp = (2_u32).checked_pow(high_attempt - 1).unwrap_or(u32::MAX);
        let delay = sender
            .retry_delay
            .checked_mul(exp)
            .unwrap_or(Duration::from_secs(60));
        let delay = if delay > Duration::from_secs(60) {
            Duration::from_secs(60)
        } else {
            delay
        };

        // Should be capped at 60 seconds
        assert_eq!(delay, Duration::from_secs(60));
    }

    #[test]
    fn test_multiplication_overflow_protection() {
        // Test with very large retry_delay to trigger multiplication overflow
        let sender = EmailSender::new_resend(
            "test_key".to_string(),
            "https://api.resend.com/emails".to_string(),
        )
        .with_retry_delay(Duration::from_secs(u32::MAX as u64));

        let attempt = 2; // 2^1 = 2, but u32::MAX * 2 would overflow
        let max_delay = Duration::from_secs(60);
        let exp = (2_u32).checked_pow(attempt - 1).unwrap_or(u32::MAX);
        let delay = sender.retry_delay.checked_mul(exp).unwrap_or(max_delay);
        let delay = if delay > max_delay { max_delay } else { delay };

        // Should fallback to max_delay due to multiplication overflow
        assert_eq!(delay, Duration::from_secs(60));
    }
}
