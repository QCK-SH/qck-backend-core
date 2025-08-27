// Refresh Token Database Model for JWT Management
// DEV-92: Database storage for refresh tokens with revocation support
// DEV-107: Enhanced with token rotation and device fingerprinting
//
// SECURITY NOTICE: JTI Hash Salt Configuration
// ==============================================
// This module implements secure JTI (JWT ID) hashing for refresh tokens.
//
// PRODUCTION REQUIREMENTS:
// - MUST configure JTI_HASH_SALT environment variable (minimum 32 bytes)
// - Use cryptographically secure random salt unique to your deployment
// - Never use predictable, hardcoded, or reused salts
//
// SALT ROTATION IMPACT:
// - Changing JTI_HASH_SALT invalidates ALL existing refresh tokens
// - Plan salt rotation during maintenance windows
// - Consider gradual migration strategies for zero-downtime deployments
//
// SECURITY IMPLICATIONS:
// - JTI hashing prevents token enumeration attacks
// - Salting prevents rainbow table attacks against JTI values
// - Unique salts per deployment prevent cross-deployment token reuse
//
// EXAMPLE SECURE SALT GENERATION:
// ```bash
// openssl rand -base64 48  # Generates 64-character base64 salt
// python3 -c "import secrets; print(secrets.token_urlsafe(48))"
// ```

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
// IP address handling now uses String type
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::schema::refresh_tokens;

/// Device information for refresh token tracking
#[derive(Debug, Clone, Default)]
pub struct DeviceInfo {
    pub fingerprint: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Refresh token database model with rotation support
#[derive(
    Debug, Clone, Serialize, Deserialize, Queryable, QueryableByName, Selectable, Identifiable,
)]
#[diesel(table_name = refresh_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub jti_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub token_family: String,
    pub issued_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_reason: Option<String>,
    pub device_fingerprint: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// New refresh token for insertion with rotation support
#[derive(Debug, Insertable)]
#[diesel(table_name = refresh_tokens)]
pub struct NewRefreshToken {
    pub user_id: Uuid,
    pub jti_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub token_family: String,
    pub issued_at: DateTime<Utc>,
    pub device_fingerprint: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Refresh token update struct for revocation
#[derive(Debug, AsChangeset)]
#[diesel(table_name = refresh_tokens)]
pub struct RefreshTokenUpdate {
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_reason: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Errors for refresh token operations
#[derive(thiserror::Error, Debug)]
pub enum RefreshTokenError {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Token not found")]
    NotFound,

    #[error("Token expired")]
    Expired,

    #[error("Token revoked")]
    Revoked,

    #[error("Invalid token format")]
    InvalidFormat,

    #[error("Connection pool error")]
    Pool(String),

    #[error("Token reuse detected - possible theft attempt")]
    TokenReuseDetected,

    #[error("Too many active tokens for user")]
    TooManyActiveTokens,

    #[error("Invalid device fingerprint")]
    InvalidDeviceFingerprint,

    #[error("Suspicious activity detected")]
    SuspiciousActivity,
}

impl RefreshToken {
    /// Get the JTI hash salt from centralized config with proper security enforcement.
    ///
    /// Application-wide salt for JTI hashing to prevent rainbow table attacks.
    /// The salt must be provided via secure configuration (e.g., environment variable).
    /// Changing the salt will invalidate all existing refresh tokens.
    ///
    /// # Security Notes
    /// - Production environments MUST configure JTI_HASH_SALT environment variable
    /// - The salt should be at least 32 bytes of random data
    /// - Salt rotation invalidates all existing refresh tokens
    /// - Never use predictable or hardcoded salts in production
    ///
    /// # Panics
    /// Panics if the JTI hash salt is not configured in production environments.
    fn get_jti_hash_salt() -> Vec<u8> {
        // For tests only, use a deterministic salt when config is not available
        #[cfg(test)]
        {
            use std::env;
            // Only allow deterministic salt if running under a test environment variable
            // (e.g., RUST_TEST_THREADS is set by cargo test)
            if env::var("RUST_TEST_THREADS").is_ok() || env::var("TEST_ENV").is_ok() {
                // In test mode, check if we can access config without panicking
                use std::panic;
                let result =
                    panic::catch_unwind(|| crate::app_config::config().jti_hash_salt.clone());

                if result.is_err() {
                    // Config not available in unit tests, use test-only deterministic salt
                    // This is only compiled for test builds and never exposed in production
                    return b"test-only-jti-salt-never-use-in-production-this-is-insecure".to_vec();
                }
            }
        }

        // Get configuration
        let config = crate::app_config::config();

        // All environments should have JTI_HASH_SALT configured
        match &config.jti_hash_salt {
            Some(salt) => {
                // Validate salt length
                if salt.len() < 32 {
                    if config.is_production() {
                        panic!("SECURITY ERROR: JTI_HASH_SALT must be at least 32 bytes in production. Current length: {}", salt.len());
                    } else {
                        eprintln!("WARNING: JTI_HASH_SALT should be at least 32 bytes. Current length: {}", salt.len());
                    }
                }
                salt.as_bytes().to_vec()
            },
            None => {
                // Salt MUST be configured in all non-test environments
                if config.is_production() {
                    panic!("SECURITY ERROR: JTI_HASH_SALT environment variable is required in production environments. This prevents using predictable salts that could compromise token security.");
                } else {
                    panic!("JTI_HASH_SALT environment variable must be configured. Set JTI_HASH_SALT to a random 32+ byte string (use: openssl rand -base64 48)");
                }
            },
        }
    }

    /// Create SHA-256 hash of JTI for secure storage, using a salt for defense in depth
    pub fn hash_jti(jti: &str) -> String {
        Self::hash_jti_with_salt(jti, None)
    }

    /// Create SHA-256 hash of JTI with injectable salt for testing and flexibility
    pub fn hash_jti_with_salt(jti: &str, salt: Option<&[u8]>) -> String {
        let mut hasher = Sha256::new();
        let default_salt = Self::get_jti_hash_salt();
        let salt = salt.unwrap_or(&default_salt);
        hasher.update(salt);
        hasher.update(jti.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Store new refresh token hash in database with device info
    pub async fn store(
        conn: &mut AsyncPgConnection,
        user_id_val: Uuid,
        jti: &str,
        expires_at_val: DateTime<Utc>,
        token_family_val: String,
        device_info: DeviceInfo,
    ) -> Result<Self, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();
        let new_token = NewRefreshToken {
            user_id: user_id_val,
            jti_hash: Self::hash_jti(jti),
            created_at: now,
            expires_at: expires_at_val,
            token_family: token_family_val,
            issued_at: now,
            device_fingerprint: device_info.fingerprint,
            ip_address: device_info.ip_address,
            user_agent: device_info.user_agent,
        };

        diesel::insert_into(refresh_tokens)
            .values(&new_token)
            .get_result::<RefreshToken>(conn)
            .await
            .map_err(RefreshTokenError::Database)
    }

    /// Validate refresh token by JTI
    pub async fn validate(
        conn: &mut AsyncPgConnection,
        jti: &str,
    ) -> Result<Self, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let jti_hash_val = Self::hash_jti(jti);
        let now = Utc::now();

        let token = refresh_tokens
            .filter(jti_hash.eq(jti_hash_val))
            .first::<RefreshToken>(conn)
            .await
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RefreshTokenError::NotFound,
                _ => RefreshTokenError::Database(e),
            })?;

        // Check if token is revoked
        if token.revoked_at.is_some() {
            return Err(RefreshTokenError::Revoked);
        }

        // Check if token is expired
        if token.expires_at <= now {
            return Err(RefreshTokenError::Expired);
        }

        Ok(token)
    }

    /// Validate refresh token with pessimistic locking for concurrent rotation
    /// Uses FOR UPDATE to prevent race conditions during token rotation
    /// This ensures only one request can rotate a token at a time
    pub async fn validate_and_lock(
        conn: &mut AsyncPgConnection,
        jti: &str,
    ) -> Result<Self, RefreshTokenError> {
        use diesel::sql_query;
        use diesel::sql_types::Text;

        let jti_hash_val = Self::hash_jti(jti);
        let now = Utc::now();

        // Use raw SQL with FOR UPDATE lock to prevent concurrent access
        // This is critical for preventing race conditions in token rotation
        let token = sql_query(
            "SELECT id, user_id, jti_hash, created_at, expires_at, revoked_at, \
             token_family, issued_at, last_used_at, revoked_reason, \
             device_fingerprint, ip_address, user_agent, updated_at \
             FROM refresh_tokens \
             WHERE jti_hash = $1 \
             FOR UPDATE",
        )
        .bind::<Text, _>(jti_hash_val)
        .get_result::<RefreshToken>(conn)
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => RefreshTokenError::NotFound,
            _ => RefreshTokenError::Database(e),
        })?;

        // Check if token is revoked
        if token.revoked_at.is_some() {
            return Err(RefreshTokenError::Revoked);
        }

        // Check if token is expired
        if token.expires_at <= now {
            return Err(RefreshTokenError::Expired);
        }

        Ok(token)
    }

    /// Revoke refresh token by JTI
    pub async fn revoke(
        conn: &mut AsyncPgConnection,
        jti: &str,
    ) -> Result<bool, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let jti_hash_val = Self::hash_jti(jti);
        let now = Utc::now();

        let updated = diesel::update(
            refresh_tokens
                .filter(jti_hash.eq(jti_hash_val))
                .filter(revoked_at.is_null()),
        )
        .set(revoked_at.eq(Some(now)))
        .execute(conn)
        .await?;

        Ok(updated > 0)
    }

    /// Revoke all refresh tokens for a user
    pub async fn revoke_all_for_user(
        conn: &mut AsyncPgConnection,
        user_id_val: Uuid,
    ) -> Result<usize, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();

        let updated = diesel::update(
            refresh_tokens
                .filter(user_id.eq(user_id_val))
                .filter(revoked_at.is_null())
                .filter(expires_at.gt(now)),
        )
        .set(revoked_at.eq(Some(now)))
        .execute(conn)
        .await?;

        Ok(updated)
    }

    /// Clean up expired tokens (should be run periodically)
    pub async fn cleanup_expired(conn: &mut AsyncPgConnection) -> Result<usize, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();

        let deleted = diesel::delete(
            refresh_tokens
                .filter(expires_at.le(now))
                .or_filter(revoked_at.is_not_null()),
        )
        .execute(conn)
        .await?;

        Ok(deleted)
    }

    /// Get active token count for user (for rate limiting)
    pub async fn count_active_for_user(
        conn: &mut AsyncPgConnection,
        user_id_val: Uuid,
    ) -> Result<i64, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();

        let count = refresh_tokens
            .filter(user_id.eq(user_id_val))
            .filter(revoked_at.is_null())
            .filter(expires_at.gt(now))
            .count()
            .get_result::<i64>(conn)
            .await?;

        Ok(count)
    }

    /// Check if token is active (not expired and not revoked)
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        self.revoked_at.is_none() && self.expires_at > now
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        self.expires_at <= now
    }

    /// Check if token is revoked
    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    /// Detect token reuse by checking if a revoked token is being used
    /// Returns true if reuse is detected, false otherwise
    pub async fn detect_token_reuse(
        conn: &mut AsyncPgConnection,
        jti: &str,
    ) -> Result<bool, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let jti_hash_val = Self::hash_jti(jti);

        // Check if this JTI belongs to a revoked token
        let revoked_token = refresh_tokens
            .filter(jti_hash.eq(&jti_hash_val))
            .filter(revoked_at.is_not_null())
            .first::<RefreshToken>(conn)
            .await
            .optional()?;

        Ok(revoked_token.is_some())
    }

    /// Revoke all tokens in a family (for token reuse detection)
    pub async fn revoke_token_family(
        conn: &mut AsyncPgConnection,
        token_family_val: &str,
        reason: &str,
    ) -> Result<usize, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();

        let updated = diesel::update(
            refresh_tokens
                .filter(token_family.eq(token_family_val))
                .filter(revoked_at.is_null()),
        )
        .set((
            revoked_at.eq(Some(now)),
            revoked_reason.eq(Some(reason)),
            updated_at.eq(now),
        ))
        .execute(conn)
        .await?;

        Ok(updated)
    }

    /// Update token as used
    pub async fn mark_as_used(
        conn: &mut AsyncPgConnection,
        jti: &str,
    ) -> Result<bool, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let jti_hash_val = Self::hash_jti(jti);
        let now = Utc::now();

        let updated = diesel::update(
            refresh_tokens
                .filter(jti_hash.eq(jti_hash_val))
                .filter(revoked_at.is_null()),
        )
        .set((last_used_at.eq(Some(now)), updated_at.eq(now)))
        .execute(conn)
        .await?;

        Ok(updated > 0)
    }

    /// Check for suspicious activity based on device fingerprint and IP
    pub async fn check_suspicious_activity(
        conn: &mut AsyncPgConnection,
        user_id_val: Uuid,
        device_fingerprint_val: Option<&str>,
        ip_address_val: Option<&str>,
    ) -> Result<bool, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();

        // Get recent active tokens for the user
        let recent_tokens = refresh_tokens
            .filter(user_id.eq(user_id_val))
            .filter(revoked_at.is_null())
            .filter(expires_at.gt(now))
            .order(created_at.desc())
            .limit(10)
            .load::<RefreshToken>(conn)
            .await?;

        // Check if current device/IP is already known
        let current_device_known = if let Some(fingerprint) = device_fingerprint_val {
            recent_tokens
                .iter()
                .any(|t| t.device_fingerprint.as_deref() == Some(fingerprint))
        } else {
            false
        };

        let current_ip_known = if let Some(ip) = ip_address_val {
            recent_tokens
                .iter()
                .any(|t| t.ip_address.as_deref() == Some(ip))
        } else {
            false
        };

        // Check for rapid token creation from different devices/IPs
        if recent_tokens.len() >= 5 {
            let unique_fingerprints: std::collections::HashSet<_> = recent_tokens
                .iter()
                .filter_map(|t| t.device_fingerprint.as_ref())
                .collect();

            let unique_ips: std::collections::HashSet<_> = recent_tokens
                .iter()
                .filter_map(|t| t.ip_address.as_ref())
                .collect();

            // Suspicious if many different devices/IPs in short time
            // Or if current device/IP is unknown and there are already many devices
            if unique_fingerprints.len() >= 4 || unique_ips.len() >= 4 {
                // Extra suspicious if current device/IP is new
                if !current_device_known || !current_ip_known {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Store new refresh token hash in database within a transaction
    /// DEV-94: For atomic token rotation
    pub async fn store_in_transaction(
        tx: &mut AsyncPgConnection,
        user_id_val: Uuid,
        jti: &str,
        expires_at_val: DateTime<Utc>,
        token_family_val: String,
        device_info: DeviceInfo,
    ) -> Result<Self, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let now = Utc::now();
        let new_token = NewRefreshToken {
            user_id: user_id_val,
            jti_hash: Self::hash_jti(jti),
            created_at: now,
            expires_at: expires_at_val,
            token_family: token_family_val,
            issued_at: now,
            device_fingerprint: device_info.fingerprint,
            ip_address: device_info.ip_address,
            user_agent: device_info.user_agent,
        };

        diesel::insert_into(refresh_tokens)
            .values(&new_token)
            .get_result::<RefreshToken>(tx)
            .await
            .map_err(RefreshTokenError::Database)
    }

    /// Revoke refresh token by JTI within a transaction
    /// DEV-94/DEV-107: For atomic token rotation
    pub async fn revoke_in_transaction(
        tx: &mut AsyncPgConnection,
        jti: &str,
        reason: Option<&str>,
    ) -> Result<bool, RefreshTokenError> {
        use crate::schema::refresh_tokens::dsl::*;

        let jti_hash_val = Self::hash_jti(jti);
        let now = Utc::now();

        let updated = diesel::update(
            refresh_tokens
                .filter(jti_hash.eq(jti_hash_val))
                .filter(revoked_at.is_null()),
        )
        .set((
            revoked_at.eq(Some(now)),
            revoked_reason.eq(reason),
            updated_at.eq(now),
        ))
        .execute(tx)
        .await?;

        Ok(updated > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use chrono::Duration;

    #[test]
    fn test_jti_hashing() {
        let jti1 = "test-jti-123";
        let jti2 = "test-jti-456";

        let hash1 = RefreshToken::hash_jti(jti1);
        let hash2 = RefreshToken::hash_jti(jti2);

        // Hashes should be different
        assert_ne!(hash1, hash2);

        // Same input should produce same hash
        let hash1_again = RefreshToken::hash_jti(jti1);
        assert_eq!(hash1, hash1_again);

        // Hash should be hex string
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash1.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars
    }

    #[test]
    fn test_jti_hashing_with_injectable_salt() {
        let jti = "test-jti-123";
        let custom_salt = b"custom-test-salt-for-testing";

        // Hash with custom salt
        let hash_with_custom = RefreshToken::hash_jti_with_salt(jti, Some(custom_salt));

        // Hash with default salt
        let hash_with_default = RefreshToken::hash_jti(jti);

        // Hashes should be different with different salts
        assert_ne!(hash_with_custom, hash_with_default);

        // Same salt should produce same hash
        let hash_with_custom_again = RefreshToken::hash_jti_with_salt(jti, Some(custom_salt));
        assert_eq!(hash_with_custom, hash_with_custom_again);

        // Hash should be hex string
        assert!(hash_with_custom.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash_with_custom.len(), 64);
    }

    #[test]
    fn test_token_state_checks() {
        let now = Utc::now();

        // Active token
        let active_token = RefreshToken {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            jti_hash: "hash123".to_string(),
            created_at: now - Duration::hours(1),
            expires_at: now + Duration::hours(1),
            revoked_at: None,
            token_family: "test-family".to_string(),
            issued_at: now - Duration::hours(1),
            last_used_at: None,
            revoked_reason: None,
            device_fingerprint: None,
            ip_address: None,
            user_agent: None,
            updated_at: now,
        };

        assert!(active_token.is_active());
        assert!(!active_token.is_expired());
        assert!(!active_token.is_revoked());

        // Expired token
        let expired_token = RefreshToken {
            expires_at: now - Duration::hours(1),
            ..active_token.clone()
        };

        assert!(!expired_token.is_active());
        assert!(expired_token.is_expired());
        assert!(!expired_token.is_revoked());

        // Revoked token
        let revoked_token = RefreshToken {
            revoked_at: Some(now - Duration::minutes(30)),
            expires_at: now + Duration::hours(1),
            ..active_token.clone()
        };

        assert!(!revoked_token.is_active());
        assert!(!revoked_token.is_expired());
        assert!(revoked_token.is_revoked());
    }
}
