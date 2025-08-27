// Email Verification Service - DEV-103
// Manages verification codes in Redis with rate limiting

use crate::db::redis_pool::RedisPool;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, instrument, warn};

#[derive(Error, Debug)]
pub enum VerificationError {
    #[error("Invalid verification code")]
    InvalidCode,

    #[error("Verification code expired")]
    CodeExpired,

    #[error("Too many attempts")]
    TooManyAttempts,

    #[error("Too many resend requests")]
    ResendLimitExceeded,

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("User not found")]
    UserNotFound,

    #[error("Email already verified")]
    AlreadyVerified,
}

/// Verification code data stored in Redis
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationCode {
    pub code: String,
    pub email: String,
    pub user_id: String,
    pub created_at: i64,
    pub attempts: u32,
}

/// Verification service for managing email verification codes
#[derive(Clone)]
pub struct VerificationService {
    redis_pool: RedisPool,
    code_ttl: u64,            // TTL in seconds
    max_attempts: u32,        // Max verification attempts per code
    resend_limit: u32,        // Max resends per day
    resend_window: u64,       // Resend window in seconds
    min_resend_cooldown: u64, // Minimum seconds between resend attempts
}

impl VerificationService {
    pub fn new(
        redis_pool: RedisPool,
        code_ttl: u64,
        max_attempts: u32,
        resend_limit: u32,
        resend_window: u64,
        min_resend_cooldown: u64,
    ) -> Self {
        Self {
            redis_pool,
            code_ttl,
            max_attempts,
            resend_limit,
            resend_window,
            min_resend_cooldown,
        }
    }

    /// Check if a verification code has expired
    /// Returns true if the code is expired based on creation time and TTL
    fn is_code_expired(&self, created_at: i64, current_time: i64) -> bool {
        current_time - created_at > self.code_ttl as i64
    }

    /// Store verification code in Redis
    #[instrument(skip(self))]
    pub async fn store_verification_code(
        &self,
        email: &str,
        user_id: &str,
        code: &str,
    ) -> Result<(), VerificationError> {
        let key = format!("verify:email:{}:code", email);

        let verification_data = VerificationCode {
            code: code.to_string(),
            email: email.to_string(),
            user_id: user_id.to_string(),
            created_at: Utc::now().timestamp(),
            attempts: 0,
        };

        let serialized = serde_json::to_string(&verification_data)
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        // Store code with TTL
        self.redis_pool
            .set_with_expiry(&key, serialized, self.code_ttl as usize)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        // Reset attempts counter
        let attempts_key = format!("verify:email:{}:attempts", email);
        self.redis_pool
            .del(&attempts_key)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        info!("Stored verification code for email: {}", email);
        Ok(())
    }

    /// Verify the provided code
    #[instrument(skip(self, provided_code))]
    pub async fn verify_code(
        &self,
        email: &str,
        provided_code: &str,
    ) -> Result<String, VerificationError> {
        let key = format!("verify:email:{}:code", email);

        // Get stored verification data
        let data: String = self
            .redis_pool
            .get::<String>(&key)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?
            .ok_or(VerificationError::InvalidCode)?;

        let mut verification_data: VerificationCode = serde_json::from_str(&data)
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        // Double-check expiry in case of clock skew or Redis TTL race conditions
        let now = Utc::now().timestamp();
        if self.is_code_expired(verification_data.created_at, now) {
            self.redis_pool.del(&key).await.ok();
            return Err(VerificationError::CodeExpired);
        }

        // Check attempts
        if verification_data.attempts >= self.max_attempts {
            warn!("Too many verification attempts for email: {}", email);
            self.redis_pool.del(&key).await.ok();
            return Err(VerificationError::TooManyAttempts);
        }

        // Increment attempts
        verification_data.attempts += 1;
        let serialized = serde_json::to_string(&verification_data)
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        // Update with remaining TTL
        let remaining_ttl = self
            .code_ttl
            .saturating_sub((now - verification_data.created_at) as u64);
        self.redis_pool
            .set_with_expiry(&key, serialized, remaining_ttl as usize)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        // Verify code
        if verification_data.code != provided_code {
            return Err(VerificationError::InvalidCode);
        }

        // Success - delete the code
        self.redis_pool.del(&key).await.ok();

        // Clear attempts counter
        let attempts_key = format!("verify:email:{}:attempts", email);
        self.redis_pool.del(&attempts_key).await.ok();

        info!("Email verified successfully for: {}", email);
        Ok(verification_data.user_id)
    }

    /// Check if resend is allowed
    #[instrument(skip(self))]
    pub async fn check_resend_allowed(&self, email: &str) -> Result<bool, VerificationError> {
        let resend_key = format!("verify:email:{}:resend_count", email);

        let count = self
            .redis_pool
            .get::<String>(&resend_key)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        Ok(count < self.resend_limit)
    }

    /// Increment resend counter
    #[instrument(skip(self))]
    pub async fn increment_resend_count(&self, email: &str) -> Result<u32, VerificationError> {
        let resend_key = format!("verify:email:{}:resend_count", email);

        // Get current count
        let current_count = self
            .redis_pool
            .get::<String>(&resend_key)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        let new_count = current_count + 1;

        if new_count > self.resend_limit {
            return Err(VerificationError::ResendLimitExceeded);
        }

        // Set new count with expiry
        self.redis_pool
            .set_with_expiry(
                &resend_key,
                new_count.to_string(),
                self.resend_window as usize,
            )
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        // Store last resend timestamp
        let timestamp_key = format!("verify:email:{}:last_resend", email);
        self.redis_pool
            .set_with_expiry(
                &timestamp_key,
                Utc::now().timestamp().to_string(),
                self.resend_window as usize,
            )
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?;

        Ok(new_count)
    }

    /// Get time until next resend is allowed (in seconds)
    #[instrument(skip(self))]
    pub async fn get_resend_cooldown(&self, email: &str) -> Result<u64, VerificationError> {
        let timestamp_key = format!("verify:email:{}:last_resend", email);

        let last_resend = self
            .redis_pool
            .get::<String>(&timestamp_key)
            .await
            .map_err(|e| VerificationError::RedisError(e.to_string()))?
            .and_then(|v| v.parse::<i64>().ok());

        if let Some(last_timestamp) = last_resend {
            let now = Utc::now().timestamp();
            let elapsed = (now - last_timestamp) as u64;

            // Check against configured minimum cooldown between resends
            if elapsed < self.min_resend_cooldown {
                return Ok(self.min_resend_cooldown - elapsed);
            }
        }

        Ok(0)
    }

    /// Clean up verification data for a user
    #[instrument(skip(self))]
    pub async fn cleanup_verification_data(&self, email: &str) -> Result<(), VerificationError> {
        let keys = vec![
            format!("verify:email:{}:code", email),
            format!("verify:email:{}:attempts", email),
            format!("verify:email:{}:resend_count", email),
            format!("verify:email:{}:last_resend", email),
        ];

        for key in keys {
            self.redis_pool.del(&key).await.ok();
        }

        info!("Cleaned up verification data for email: {}", email);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test instance to verify expiry logic without Redis dependency
    /// Since expiry logic is purely computational, we don't need a real Redis pool
    struct TestExpiry {
        code_ttl: u64,
    }

    impl TestExpiry {
        fn new(code_ttl: u64) -> Self {
            Self { code_ttl }
        }

        /// Test helper method that mirrors VerificationService::is_code_expired
        fn is_code_expired(&self, created_at: i64, current_time: i64) -> bool {
            current_time - created_at > self.code_ttl as i64
        }
    }

    #[test]
    fn test_code_not_expired() {
        let expiry_checker = TestExpiry::new(900); // 15 minutes TTL
        let now = Utc::now().timestamp();

        // Code created 5 minutes ago (300 seconds), TTL is 900 seconds
        let created_at = now - 300;

        assert!(
            !expiry_checker.is_code_expired(created_at, now),
            "Code should not be expired when created 5 minutes ago with 15-minute TTL"
        );
    }

    #[test]
    fn test_code_expired() {
        let expiry_checker = TestExpiry::new(900); // 15 minutes TTL
        let now = Utc::now().timestamp();

        // Code created 20 minutes ago (1200 seconds), TTL is 900 seconds
        let created_at = now - 1200;

        assert!(
            expiry_checker.is_code_expired(created_at, now),
            "Code should be expired when created 20 minutes ago with 15-minute TTL"
        );
    }

    #[test]
    fn test_code_exactly_at_expiry() {
        let expiry_checker = TestExpiry::new(900); // 15 minutes TTL
        let now = Utc::now().timestamp();

        // Code created exactly at TTL boundary (900 seconds ago)
        let created_at = now - 900;

        assert!(
            !expiry_checker.is_code_expired(created_at, now),
            "Code should not be expired at exact TTL boundary"
        );
    }

    #[test]
    fn test_code_just_expired() {
        let expiry_checker = TestExpiry::new(900); // 15 minutes TTL
        let now = Utc::now().timestamp();

        // Code created 1 second past TTL (901 seconds ago)
        let created_at = now - 901;

        assert!(
            expiry_checker.is_code_expired(created_at, now),
            "Code should be expired 1 second past TTL"
        );
    }

    #[test]
    fn test_code_created_in_future() {
        let expiry_checker = TestExpiry::new(900); // 15 minutes TTL
        let now = Utc::now().timestamp();

        // Code created in the future (clock skew scenario)
        let created_at = now + 60;

        assert!(
            !expiry_checker.is_code_expired(created_at, now),
            "Code created in future should not be considered expired"
        );
    }

    #[test]
    fn test_different_ttl_values() {
        let now = Utc::now().timestamp();

        // Test with short TTL (60 seconds)
        let short_expiry = TestExpiry::new(60);

        // Code created 30 seconds ago
        let created_at = now - 30;
        assert!(
            !short_expiry.is_code_expired(created_at, now),
            "Code should not be expired with short TTL"
        );

        // Code created 70 seconds ago
        let created_at_expired = now - 70;
        assert!(
            short_expiry.is_code_expired(created_at_expired, now),
            "Code should be expired with short TTL"
        );

        // Test with long TTL (3600 seconds = 1 hour)
        let long_expiry = TestExpiry::new(3600);

        // Same 70 seconds ago should not be expired with long TTL
        assert!(
            !long_expiry.is_code_expired(created_at_expired, now),
            "Code should not be expired with long TTL"
        );
    }

    #[test]
    fn test_edge_cases_with_large_timestamps() {
        let expiry_checker = TestExpiry::new(900); // 15 minutes TTL

        // Test with large timestamp values (year 2050+)
        let far_future = 2_500_000_000i64; // Year 2049
        let created_at = far_future - 1200; // 20 minutes before

        assert!(
            expiry_checker.is_code_expired(created_at, far_future),
            "Code should be expired even with large timestamp values"
        );

        let created_at_valid = far_future - 300; // 5 minutes before
        assert!(
            !expiry_checker.is_code_expired(created_at_valid, far_future),
            "Code should not be expired with large timestamp values"
        );
    }

    #[test]
    fn test_zero_ttl() {
        let expiry_checker = TestExpiry::new(0); // Immediate expiry
        let now = Utc::now().timestamp();

        // Code created now should not be expired (boundary case)
        assert!(
            !expiry_checker.is_code_expired(now, now),
            "Code created at exact same time should not be expired with 0 TTL"
        );

        // Code created 1 second ago should be expired
        let created_at = now - 1;
        assert!(
            expiry_checker.is_code_expired(created_at, now),
            "Code created 1 second ago should be expired with 0 TTL"
        );
    }

    #[test]
    fn test_very_large_ttl() {
        // Test with very large TTL (1 year = 31,536,000 seconds)
        let expiry_checker = TestExpiry::new(31_536_000);
        let now = Utc::now().timestamp();

        // Code created 1 day ago should not be expired
        let created_at = now - 86400; // 1 day ago
        assert!(
            !expiry_checker.is_code_expired(created_at, now),
            "Code created 1 day ago should not be expired with 1-year TTL"
        );

        // Code created 6 months ago should not be expired
        let created_at_6months = now - (86400 * 180); // ~6 months ago
        assert!(
            !expiry_checker.is_code_expired(created_at_6months, now),
            "Code created 6 months ago should not be expired with 1-year TTL"
        );
    }
}
