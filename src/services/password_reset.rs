use base64::prelude::*;
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    db::DieselPool,
    models::{NewPasswordResetToken, PasswordResetToken, User},
    schema::{password_reset_tokens, users},
    utils::auth_errors::AuthError,
};

#[derive(Clone)]
pub struct PasswordResetService {
    pool: DieselPool,
    timing_attack_delay_ms: u64,
}

#[derive(Debug)]
pub struct PasswordResetTokenInfo {
    pub token: String,      // Raw token (to send in email)
    pub token_hash: String, // Hashed token (to store in database)
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl PasswordResetService {
    pub fn new(pool: DieselPool) -> Self {
        Self {
            pool,
            timing_attack_delay_ms: 150, // Default 150ms delay to prevent timing attacks
        }
    }

    pub fn new_with_delay(pool: DieselPool, timing_attack_delay_ms: u64) -> Self {
        Self {
            pool,
            timing_attack_delay_ms,
        }
    }

    /// Generate a cryptographically secure password reset token
    pub fn generate_reset_token() -> PasswordResetTokenInfo {
        // Generate 32 bytes of random data (256 bits of entropy)
        let mut token_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut token_bytes);

        // Convert to base64url for safe URL transmission
        let token = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(token_bytes);

        // Create SHA-256 hash for database storage
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let token_hash = format!("{:x}", hasher.finalize());

        // Set expiration to 15 minutes from now
        let expires_at = Utc::now() + Duration::minutes(15);

        PasswordResetTokenInfo {
            token,
            token_hash,
            expires_at,
        }
    }

    /// Create a password reset request for a user
    pub async fn create_reset_request(
        &self,
        email: &str,
        ip_address: Option<IpAddr>,
        user_agent: Option<String>,
    ) -> Result<Option<PasswordResetTokenInfo>, AuthError> {
        let mut conn =
            self.pool.get().await.map_err(|e| {
                AuthError::DatabaseError(format!("Database connection failed: {}", e))
            })?;

        // First, find the user by email (don't reveal if user exists)
        let user_result: Result<User, diesel::result::Error> = users::table
            .filter(users::email.eq(email))
            .first(&mut conn)
            .await;

        // Generate secure token (do this regardless of user existence for timing consistency)
        let token_info = Self::generate_reset_token();

        let user = match user_result {
            Ok(user) => user,
            Err(_) => {
                // Don't reveal that user doesn't exist
                // Perform similar operations to maintain consistent timing
                tracing::info!("Password reset requested for non-existent email: {}", email);

                // Perform a realistic delay equivalent to database operations
                // This prevents timing attacks by ensuring similar response times
                // Use async sleep to avoid busy-waiting and high CPU usage
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.timing_attack_delay_ms,
                ))
                .await;

                return Ok(None);
            },
        };

        // Clean up any existing tokens for this user (prevent token accumulation)
        diesel::delete(
            password_reset_tokens::table.filter(password_reset_tokens::user_id.eq(user.id)),
        )
        .execute(&mut conn)
        .await
        .map_err(|e| AuthError::DatabaseError(format!("Failed to clean existing tokens: {}", e)))?;

        // Create new reset token record
        let new_token = NewPasswordResetToken::new(
            user.id,
            token_info.token_hash.clone(),
            token_info.expires_at,
            ip_address.map(|ip| ip.to_string()),
            user_agent,
        );

        diesel::insert_into(password_reset_tokens::table)
            .values(&new_token)
            .execute(&mut conn)
            .await
            .map_err(|e| {
                AuthError::DatabaseError(format!("Failed to create reset token: {}", e))
            })?;

        tracing::info!(
            "Password reset token created for user {} from IP {:?}",
            user.id,
            ip_address
        );

        Ok(Some(token_info))
    }

    /// Validate and consume a password reset token
    /// Uses constant-time comparison to prevent timing attacks
    pub async fn validate_and_consume_token(&self, token: &str) -> Result<Uuid, AuthError> {
        let mut conn =
            self.pool.get().await.map_err(|e| {
                AuthError::DatabaseError(format!("Database connection failed: {}", e))
            })?;

        // Hash the provided token
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let provided_token_hash = format!("{:x}", hasher.finalize());

        // Get all active (unused, unexpired) tokens and validate using constant-time comparison
        let active_tokens: Vec<PasswordResetToken> = password_reset_tokens::table
            .filter(password_reset_tokens::used_at.is_null())
            .filter(password_reset_tokens::expires_at.gt(Utc::now()))
            .load(&mut conn)
            .await
            .map_err(|e| AuthError::DatabaseError(format!("Failed to load reset tokens: {}", e)))?;

        // Use constant-time comparison to find matching token
        let mut found_token: Option<PasswordResetToken> = None;
        for token_record in active_tokens {
            let is_match = provided_token_hash
                .as_bytes()
                .ct_eq(token_record.token_hash.as_bytes());

            if is_match.into() {
                found_token = Some(token_record);
                break;
            }
        }

        let reset_token = found_token.ok_or(AuthError::InvalidToken)?;

        // Mark token as used
        diesel::update(password_reset_tokens::table.find(reset_token.id))
            .set(password_reset_tokens::used_at.eq(Utc::now()))
            .execute(&mut conn)
            .await
            .map_err(|e| {
                AuthError::DatabaseError(format!("Failed to mark token as used: {}", e))
            })?;

        tracing::info!(
            "Password reset token consumed for user {} (token created: {:?})",
            reset_token.user_id,
            reset_token.created_at
        );

        Ok(reset_token.user_id)
    }

    /// Clean up expired tokens (should be called periodically)
    pub async fn cleanup_expired_tokens(&self) -> Result<u64, AuthError> {
        let mut conn =
            self.pool.get().await.map_err(|e| {
                AuthError::DatabaseError(format!("Database connection failed: {}", e))
            })?;

        let deleted_count = diesel::delete(
            password_reset_tokens::table.filter(password_reset_tokens::expires_at.lt(Utc::now())),
        )
        .execute(&mut conn)
        .await
        .map_err(|e| {
            AuthError::DatabaseError(format!("Failed to cleanup expired tokens: {}", e))
        })?;

        if deleted_count > 0 {
            tracing::info!("Cleaned up {} expired password reset tokens", deleted_count);
        }

        Ok(deleted_count as u64)
    }

    /// Get active token count for a user (for monitoring)
    pub async fn get_active_token_count(&self, user_id: Uuid) -> Result<i64, AuthError> {
        let mut conn =
            self.pool.get().await.map_err(|e| {
                AuthError::DatabaseError(format!("Database connection failed: {}", e))
            })?;

        let count = password_reset_tokens::table
            .filter(password_reset_tokens::user_id.eq(user_id))
            .filter(password_reset_tokens::used_at.is_null())
            .filter(password_reset_tokens::expires_at.gt(Utc::now()))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(|e| {
                AuthError::DatabaseError(format!("Failed to count active tokens: {}", e))
            })?;

        Ok(count)
    }

    /// Check if user has recent reset attempts (for rate limiting)
    pub async fn check_recent_attempts(
        &self,
        email: &str,
        within_hours: i64,
    ) -> Result<i64, AuthError> {
        let mut conn =
            self.pool.get().await.map_err(|e| {
                AuthError::DatabaseError(format!("Database connection failed: {}", e))
            })?;

        let cutoff_time = Utc::now() - Duration::hours(within_hours);

        let count = password_reset_tokens::table
            .inner_join(users::table.on(password_reset_tokens::user_id.eq(users::id)))
            .filter(users::email.eq(email))
            .filter(password_reset_tokens::created_at.gt(cutoff_time))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(|e| {
                AuthError::DatabaseError(format!("Failed to count recent attempts: {}", e))
            })?;

        Ok(count)
    }

    /// Get user's full name by email for personalized emails
    pub async fn get_user_name_by_email(
        &self,
        user_email: &str,
    ) -> Result<Option<String>, AuthError> {
        use crate::schema::users::dsl::{email, full_name, users as users_table};

        let mut conn =
            self.pool.get().await.map_err(|e| {
                AuthError::DatabaseError(format!("Database connection failed: {}", e))
            })?;

        let user_name: Option<String> = users_table
            .select(full_name)
            .filter(email.eq(user_email))
            .first(&mut conn)
            .await
            .optional()
            .map_err(|e| AuthError::DatabaseError(format!("Failed to get user name: {}", e)))?;

        Ok(user_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation() {
        // generate_reset_token doesn't use self, so we can call it directly
        let token_info = PasswordResetService::generate_reset_token();

        // Token should be 43 characters (32 bytes base64url encoded)
        assert_eq!(token_info.token.len(), 43);

        // Hash should be 64 characters (SHA-256 hex)
        assert_eq!(token_info.token_hash.len(), 64);

        // Should expire in the future
        assert!(token_info.expires_at > Utc::now());

        // Should expire within 16 minutes (15 + 1 for test timing)
        let max_expiry = Utc::now() + Duration::minutes(16);
        assert!(token_info.expires_at < max_expiry);
    }

    #[test]
    fn test_token_uniqueness() {
        let token1 = PasswordResetService::generate_reset_token();
        let token2 = PasswordResetService::generate_reset_token();

        // Tokens should be unique
        assert_ne!(token1.token, token2.token);
        assert_ne!(token1.token_hash, token2.token_hash);
    }
}
