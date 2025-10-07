// JWT Token Generation Service with Diesel Integration
// Implements DEV-113 requirements with HS256 algorithm per Linear specification
// DEV-107: Enhanced with refresh token rotation support

use diesel_async::AsyncPgConnection;
// Removed ipnetwork dependency - now using String for IP addresses
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

use crate::config::PermissionConfig;
use crate::db::{DieselPool, RedisPool};
use crate::models::auth::{AccessTokenClaims, RefreshTokenClaims};
use crate::models::refresh_token::{DeviceInfo, RefreshToken, RefreshTokenError};
use crate::models::user::{User, UserError};

// Error types for JWT operations
#[derive(Error, Debug)]
pub enum JwtError {
    #[error("JWT encoding error: {0}")]
    EncodingError(String),

    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),

    #[error("Key generation error: {0}")]
    KeyGenerationError(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Database error: {0}")]
    DatabaseError(#[from] RefreshTokenError),

    #[error("User error: {0}")]
    UserError(#[from] UserError),

    #[error("Token revoked")]
    TokenRevoked,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Pool error: {0}")]
    PoolError(String),

    #[error("Token reuse detected - possible security breach")]
    TokenReuseDetected,

    #[error("Suspicious activity detected")]
    SuspiciousActivity,

    #[error("Diesel error: {0}")]
    DieselError(#[from] diesel::result::Error),
}

impl From<jsonwebtoken::errors::Error> for JwtError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        use jsonwebtoken::errors::ErrorKind;
        match err.kind() {
            ErrorKind::ExpiredSignature => JwtError::TokenExpired,
            ErrorKind::InvalidToken => JwtError::InvalidToken,
            _ => JwtError::EncodingError(err.to_string()),
        }
    }
}

// JWT Configuration with separate keys for access and refresh tokens
#[derive(Clone)]
pub struct JwtConfig {
    pub access_token_expiry: u64,  // 1 hour in seconds per Linear DEV-113
    pub refresh_token_expiry: u64, // 7 days in seconds
    pub algorithm: Algorithm,      // HS256 (HMAC SHA-256) per Linear DEV-113

    // JWT validation settings
    pub audience: String, // Expected audience for tokens (e.g., "qck.sh")
    pub issuer: String,   // Token issuer identifier (e.g., "qck.sh")

    // Separate keys for access tokens
    pub access_encoding_key: EncodingKey,
    pub access_decoding_key: DecodingKey,

    // Separate keys for refresh tokens
    pub refresh_encoding_key: EncodingKey,
    pub refresh_decoding_key: DecodingKey,

    // Key versioning for rotation
    pub key_version: u32,
}

impl std::fmt::Debug for JwtConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtConfig")
            .field("access_token_expiry", &self.access_token_expiry)
            .field("refresh_token_expiry", &self.refresh_token_expiry)
            .field("algorithm", &self.algorithm)
            .field("audience", &self.audience)
            .field("issuer", &self.issuer)
            .field("access_encoding_key", &"<redacted>")
            .field("access_decoding_key", &"<redacted>")
            .field("refresh_encoding_key", &"<redacted>")
            .field("refresh_decoding_key", &"<redacted>")
            .field("key_version", &self.key_version)
            .finish()
    }
}

impl JwtConfig {
    /// Build JWT config from provided parameters - shared logic for from_env and for_test
    fn build_from_params(
        access_secret: String,
        refresh_secret: String,
        access_expiry: u64,
        refresh_expiry: u64,
        audience: String,
        issuer: String,
        key_version: u32,
    ) -> Self {
        let access_encoding_key = EncodingKey::from_secret(access_secret.as_bytes());
        let access_decoding_key = DecodingKey::from_secret(access_secret.as_bytes());

        let refresh_encoding_key = EncodingKey::from_secret(refresh_secret.as_bytes());
        let refresh_decoding_key = DecodingKey::from_secret(refresh_secret.as_bytes());

        JwtConfig {
            access_token_expiry: access_expiry,
            refresh_token_expiry: refresh_expiry,
            algorithm: Algorithm::HS256, // Linear DEV-113 requirement: HS256 (HMAC SHA-256)
            audience,
            issuer,
            access_encoding_key,
            access_decoding_key,
            refresh_encoding_key,
            refresh_decoding_key,
            key_version,
        }
    }

    /// Create JWT config from centralized app configuration
    pub fn from_env() -> Result<Self, JwtError> {
        // Use the centralized CONFIG from app_config with destructuring
        let crate::app_config::JwtConfig {
            access_secret,
            refresh_secret,
            access_expiry,
            refresh_expiry,
            audience,
            issuer,
            key_version,
        } = &crate::CONFIG.jwt;

        Ok(Self::build_from_params(
            access_secret.clone(),
            refresh_secret.clone(),
            *access_expiry,
            *refresh_expiry,
            audience.clone(),
            issuer.clone(),
            *key_version,
        ))
    }

    /// Create JWT config for tests without using lazy static
    #[cfg(test)]
    pub fn for_test() -> Self {
        // Use hardcoded test secrets and values for deterministic test behavior
        let access_secret = "test-access-secret-hs256".to_string();
        let refresh_secret = "test-refresh-secret-hs256".to_string();

        Self::build_from_params(
            access_secret,
            refresh_secret,
            3600,   // 1 hour
            604800, // 7 days
            "test.qck.sh".to_string(),
            "test.qck.sh".to_string(),
            1,
        )
    }
}

// JWT Service with Diesel database integration
pub struct JwtService {
    config: JwtConfig,
    db_pool: Option<DieselPool>,
    redis_pool: Option<RedisPool>,
}

impl JwtService {
    /// Create new JWT service with configuration
    pub fn new(config: JwtConfig) -> Self {
        Self {
            config,
            db_pool: None,
            redis_pool: None,
        }
    }

    /// Create new JWT service with Diesel database integration
    pub fn new_with_diesel(config: JwtConfig, db_pool: DieselPool) -> Self {
        Self {
            config,
            db_pool: Some(db_pool),
            redis_pool: None,
        }
    }

    /// Create new JWT service with full integration (Diesel + Redis)
    pub fn new_with_full_integration(
        config: JwtConfig,
        db_pool: DieselPool,
        redis_pool: RedisPool,
    ) -> Self {
        Self {
            config,
            db_pool: Some(db_pool),
            redis_pool: Some(redis_pool),
        }
    }

    /// Create JWT service from environment
    pub fn from_env() -> Result<Self, JwtError> {
        let config = JwtConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Create JWT service from environment with Diesel
    pub fn from_env_with_diesel(
        db_pool: DieselPool,
        redis_pool: RedisPool,
    ) -> Result<Self, JwtError> {
        let config = JwtConfig::from_env()?;
        Ok(Self::new_with_full_integration(config, db_pool, redis_pool))
    }

    /// Helper method to check if database pool is available
    #[allow(dead_code)]
    fn require_db_pool(&self) -> Result<&DieselPool, JwtError> {
        self.db_pool
            .as_ref()
            .ok_or_else(|| JwtError::PoolError("Database pool not configured".to_string()))
    }

    /// Helper method to get a database connection from the pool
    /// Centralizes connection acquisition and error handling
    #[allow(dead_code)]
    async fn get_db_connection(
        &self,
    ) -> Result<
        bb8::PooledConnection<
            '_,
            diesel_async::pooled_connection::AsyncDieselConnectionManager<AsyncPgConnection>,
        >,
        JwtError,
    > {
        self.require_db_pool()?
            .get()
            .await
            .map_err(|e| JwtError::PoolError(e.to_string()))
    }

    /// Generate access token
    pub fn generate_access_token(
        &self,
        user_id: &str,
        email: &str,
        subscription_tier: &str,
        scope: Vec<String>,
    ) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| JwtError::KeyGenerationError(e.to_string()))?
            .as_secs();

        let claims = AccessTokenClaims {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            email: email.to_string(),
            tier: subscription_tier.to_string(),
            scope,
            aud: self.config.audience.clone(),
            iss: self.config.issuer.clone(),
            iat: now,
            exp: now + self.config.access_token_expiry,
        };

        let mut header = Header::new(self.config.algorithm);
        header.kid = Some(self.config.key_version.to_string());

        encode(&header, &claims, &self.config.access_encoding_key).map_err(Into::into)
    }

    /// Generate refresh token with database storage
    pub async fn generate_refresh_token(&self, user_id: &str) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| JwtError::KeyGenerationError(e.to_string()))?
            .as_secs();

        let jti = Uuid::new_v4().to_string();
        let token_family = Uuid::new_v4().to_string(); // New family for new tokens

        let claims = RefreshTokenClaims {
            sub: user_id.to_string(),
            jti: jti.clone(),
            iat: now,
            exp: now + self.config.refresh_token_expiry,
        };

        // Store in database if pool is available
        if let Some(pool) = &self.db_pool {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;

            let expires_at = chrono::Utc::now()
                + chrono::Duration::seconds(self.config.refresh_token_expiry as i64);
            RefreshToken::store(
                &mut conn,
                Uuid::parse_str(user_id).map_err(|_| JwtError::InvalidToken)?,
                &jti,
                expires_at,
                token_family,
                DeviceInfo::default(), // device info will be set by handler
            )
            .await?;
        }

        let mut header = Header::new(self.config.algorithm);
        header.kid = Some(self.config.key_version.to_string());

        encode(&header, &claims, &self.config.refresh_encoding_key).map_err(Into::into)
    }

    /// Validates an access token and returns the decoded claims
    ///
    /// # Arguments
    /// * `token` - The JWT access token string to validate
    ///
    /// # Returns
    /// * `Ok(AccessTokenClaims)` - The decoded token claims if validation succeeds
    /// * `Err(JwtError)` - An error if the token is invalid, expired, or has wrong audience/issuer
    ///
    /// # Errors
    /// * `JwtError::EncodingError` - Token format is invalid or signature verification failed
    /// * `JwtError::TokenExpired` - Token has expired (checked with leeway=0 for strict validation)
    /// * `JwtError::InvalidToken` - Token validation failed for other reasons
    pub fn validate_access_token(&self, token: &str) -> Result<AccessTokenClaims, JwtError> {
        let mut validation = Validation::new(self.config.algorithm);
        validation.set_audience(&[self.config.audience.clone()]);
        validation.set_issuer(&[self.config.issuer.clone()]);
        validation.validate_exp = true;
        validation.validate_nbf = false;
        validation.leeway = 0; // No leeway for expiry validation

        let token_data =
            decode::<AccessTokenClaims>(token, &self.config.access_decoding_key, &validation)?;

        Ok(token_data.claims)
    }

    /// Validate refresh token with database check
    pub async fn validate_refresh_token(
        &self,
        token: &str,
    ) -> Result<RefreshTokenClaims, JwtError> {
        let mut validation = Validation::new(self.config.algorithm);
        validation.validate_exp = true;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        validation.leeway = 0; // No leeway for expiry validation

        let token_data =
            decode::<RefreshTokenClaims>(token, &self.config.refresh_decoding_key, &validation)
                .map_err(|e| {
                    // Add context to JWT decoding errors
                    match e.kind() {
                        jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::TokenExpired,
                        jsonwebtoken::errors::ErrorKind::InvalidToken => JwtError::InvalidToken,
                        _ => JwtError::EncodingError(e.to_string()),
                    }
                })?;

        // Validate against database if pool is available
        if let Some(pool) = &self.db_pool {
            let mut conn = pool.get().await.map_err(|e| {
                JwtError::PoolError(format!(
                    "Failed to get database connection for refresh token validation: {}",
                    e
                ))
            })?;

            RefreshToken::validate(&mut conn, &token_data.claims.jti)
                .await
                .map_err(|e| {
                    // Add context to database validation errors
                    match e {
                        RefreshTokenError::Expired => JwtError::TokenExpired,
                        RefreshTokenError::Revoked => JwtError::TokenRevoked,
                        RefreshTokenError::NotFound => JwtError::InvalidToken,
                        _ => JwtError::DatabaseError(e),
                    }
                })?;
        }

        Ok(token_data.claims)
    }

    /// Refresh tokens with rotation (DEV-94)
    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<(String, String), JwtError> {
        // Validate the refresh token
        let claims = self
            .validate_refresh_token(refresh_token)
            .await
            .map_err(|e| match e {
                JwtError::TokenExpired => JwtError::TokenExpired,
                JwtError::TokenRevoked => JwtError::TokenRevoked,
                _ => JwtError::InvalidToken,
            })?;

        // Get user from database
        let user = if let Some(pool) = &self.db_pool {
            let mut conn = pool.get().await.map_err(|e| {
                JwtError::PoolError(format!(
                    "Failed to get database connection for user lookup: {}",
                    e
                ))
            })?;

            let user_id = Uuid::parse_str(&claims.sub).map_err(|_| JwtError::InvalidToken)?;
            User::find_by_id(&mut conn, user_id)
                .await
                .map_err(|e| match e {
                    UserError::NotFound => JwtError::InvalidToken,
                    _ => JwtError::UserError(e),
                })?
        } else {
            return Err(JwtError::PoolError(
                "Database pool not configured for token refresh".to_string(),
            ));
        };

        // Revoke old refresh token
        if let Some(pool) = &self.db_pool {
            let mut conn = pool.get().await.map_err(|e| {
                JwtError::PoolError(format!(
                    "Failed to get database connection for token revocation: {}",
                    e
                ))
            })?;

            RefreshToken::revoke(&mut conn, &claims.jti)
                .await
                .map_err(JwtError::DatabaseError)?;
        }

        // Generate new tokens
        let scope = crate::config::PermissionConfig::get_default_permissions();
        let access_token = self.generate_access_token(
            &user.id.to_string(),
            &user.email,
            user.subscription_tier_str(),
            scope,
        )?;

        let refresh_token = self.generate_refresh_token(&user.id.to_string()).await?;

        Ok((access_token, refresh_token))
    }

    /// Logout token - blacklist in Redis
    pub async fn logout_token(&self, jti: &str, ttl_seconds: u64) -> Result<(), JwtError> {
        if let Some(redis_pool) = &self.redis_pool {
            let mut conn = redis_pool
                .get_connection()
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;

            let key = format!("blacklist:token:{}", jti);
            conn.set_ex::<_, _, ()>(key, "1", ttl_seconds)
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;
        }
        Ok(())
    }

    /// Check if token is blacklisted
    pub async fn is_token_blacklisted(&self, jti: &str) -> Result<bool, JwtError> {
        if let Some(redis_pool) = &self.redis_pool {
            let mut conn = redis_pool
                .get_connection()
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;

            let key = format!("blacklist:token:{}", jti);
            let exists: bool = conn
                .exists(&key)
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;
            Ok(exists)
        } else {
            Ok(false)
        }
    }

    /// Revoke all user tokens
    pub async fn revoke_all_user_tokens(&self, user_id: &str) -> Result<usize, JwtError> {
        if let Some(pool) = &self.db_pool {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;

            let user_uuid = Uuid::parse_str(user_id).map_err(|_| JwtError::InvalidToken)?;
            Ok(RefreshToken::revoke_all_for_user(&mut conn, user_uuid).await?)
        } else {
            Ok(0)
        }
    }

    /// Generate refresh token with device information
    pub async fn generate_refresh_token_with_device(
        &self,
        user_id: &str,
        device_fingerprint: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<String, JwtError> {
        // Default to non-remember_me behavior for backward compatibility
        self.generate_refresh_token_with_device_and_remember(
            user_id,
            device_fingerprint,
            ip_address,
            user_agent,
            false,
        )
        .await
    }

    /// Generate refresh token with device information and remember_me option
    pub async fn generate_refresh_token_with_device_and_remember(
        &self,
        user_id: &str,
        device_fingerprint: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        remember_me: bool,
    ) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| JwtError::KeyGenerationError(e.to_string()))?
            .as_secs();

        let jti = Uuid::new_v4().to_string();
        let token_family = Uuid::new_v4().to_string();

        // Calculate expiry based on remember_me flag
        let expiry = if remember_me {
            // Get remember_me duration from config (in days) and convert to seconds
            let config = crate::app_config::config();
            let remember_me_seconds = config.security.remember_me_duration_days as u64 * 86400; // days to seconds
            now + remember_me_seconds
        } else {
            now + self.config.refresh_token_expiry
        };

        let claims = RefreshTokenClaims {
            sub: user_id.to_string(),
            jti: jti.clone(),
            iat: now,
            exp: expiry,
        };

        // Store in database with device info
        if let Some(pool) = &self.db_pool {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| JwtError::PoolError(e.to_string()))?;

            // Use the calculated expiry (which accounts for remember_me)
            let expires_at = chrono::Utc::now() + chrono::Duration::seconds((expiry - now) as i64);

            // IP addresses are stored as strings for simplicity and to avoid extra dependencies

            RefreshToken::store(
                &mut conn,
                Uuid::parse_str(user_id).map_err(|_| JwtError::InvalidToken)?,
                &jti,
                expires_at,
                token_family,
                DeviceInfo {
                    fingerprint: device_fingerprint,
                    ip_address,
                    user_agent,
                },
            )
            .await?;
        }

        let mut header = Header::new(self.config.algorithm);
        header.kid = Some(self.config.key_version.to_string());

        encode(&header, &claims, &self.config.refresh_encoding_key).map_err(Into::into)
    }

    /// Rotate refresh token - validates old token, generates new pair, revokes old
    /// DEV-107: Implements secure token rotation with family tracking
    pub async fn rotate_refresh_token(
        &self,
        old_refresh_token: &str,
        device_fingerprint: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(String, String), JwtError> {
        // Decode and validate the old refresh token
        // If it fails due to revocation, check if this is a security breach
        let old_claims = match self.validate_refresh_token(old_refresh_token).await {
            Ok(claims) => claims,
            Err(JwtError::TokenRevoked) => {
                // Token is revoked - check if this is a reuse attack that should revoke the family
                // We need to decode the JWT to get the JTI even though it's revoked
                let _header = decode_header(old_refresh_token)?;
                let key = &self.config.refresh_decoding_key;
                let mut validation = Validation::new(self.config.algorithm);
                validation.validate_exp = false; // Don't validate expiry since we just want the claims
                validation.validate_nbf = false;
                validation.set_audience(&[&self.config.audience]);
                validation.set_issuer(&[&self.config.issuer]);

                let token_data = decode::<RefreshTokenClaims>(old_refresh_token, key, &validation)
                    .map_err(|_| JwtError::TokenRevoked)?; // If decode fails, just return TokenRevoked

                // Now check if this revoked token was used in a reuse attack
                if let Some(pool) = &self.db_pool {
                    let mut conn = pool
                        .get()
                        .await
                        .map_err(|e| JwtError::PoolError(e.to_string()))?;

                    // Check the token in database to see if it should trigger family revocation
                    use crate::schema::refresh_tokens::dsl::*;
                    use diesel::prelude::*;
                    use diesel_async::RunQueryDsl;

                    let jti_hash_val = RefreshToken::hash_jti(&token_data.claims.jti);
                    let token_info = refresh_tokens
                        .filter(jti_hash.eq(&jti_hash_val))
                        .first::<RefreshToken>(&mut conn)
                        .await
                        .optional()
                        .map_err(|e| JwtError::DatabaseError(RefreshTokenError::Database(e)))?;

                    if let Some(token) = token_info {
                        // If this token was revoked due to rotation, it's a security breach
                        if token.revoked_reason.as_deref() == Some("rotation") {
                            // Revoke entire family
                            eprintln!(
                                "DEV-105: Reuse of rotated token detected! Revoking family {}",
                                &token.token_family
                            );
                            RefreshToken::revoke_token_family(
                                &mut conn,
                                &token.token_family,
                                "token_reuse_detected",
                            )
                            .await?;
                            return Err(JwtError::TokenReuseDetected);
                        }
                    }
                }
                return Err(JwtError::TokenRevoked);
            },
            Err(e) => return Err(e),
        };

        let pool = self
            .db_pool
            .as_ref()
            .ok_or_else(|| JwtError::PoolError("Database pool not available".to_string()))?;

        let mut conn = pool
            .get()
            .await
            .map_err(|e| JwtError::PoolError(e.to_string()))?;

        // Start transaction for atomic rotation
        use diesel_async::AsyncConnection;
        let result: Result<(String, String), JwtError> = conn
            .transaction::<_, JwtError, _>(|tx| {
                Box::pin(async move {
                    // Try to validate and lock the token first
                    // If it's revoked, check if this is a reuse attack
                    eprintln!(
                        "DEV-105: Attempting to validate token with jti: {}",
                        &old_claims.jti
                    );
                    let validation_result =
                        RefreshToken::validate_and_lock(tx, &old_claims.jti).await;

                    let existing_token = match validation_result {
                        Ok(token) => token,
                        Err(RefreshTokenError::Revoked) => {
                            // Token is revoked - this could be a reuse attack
                            // Get the token info to check its family
                            use crate::schema::refresh_tokens::dsl::*;
                            use diesel::prelude::*;
                            use diesel_async::RunQueryDsl;

                            let jti_hash_val = RefreshToken::hash_jti(&old_claims.jti);
                            let token_info = refresh_tokens
                                .filter(jti_hash.eq(&jti_hash_val))
                                .first::<RefreshToken>(tx)
                                .await
                                .optional()
                                .map_err(|e| {
                                    JwtError::DatabaseError(RefreshTokenError::Database(e))
                                })?;

                            if let Some(token) = token_info {
                                // Check if this was revoked as part of normal rotation
                                // For security, any attempt to use a rotated token should revoke the family
                                // This prevents token theft where an attacker gets an old token
                                if token.revoked_reason.as_deref() == Some("rotation") {
                                    // This is a reuse attack - someone is trying to use an already-rotated token!
                                    // Revoke entire family for security
                                    let _revoked_count = RefreshToken::revoke_token_family(
                                        tx,
                                        &token.token_family,
                                        "token_reuse_detected",
                                    )
                                    .await?;
                                    return Err(JwtError::TokenReuseDetected);
                                }
                            }
                            // Token was already revoked, return the error
                            return Err(JwtError::TokenRevoked);
                        },
                        Err(e) => return Err(e.into()),
                    };

                    // Immediately revoke the old token to prevent reuse
                    // This must happen before any other operations
                    let revoked =
                        RefreshToken::revoke_in_transaction(tx, &old_claims.jti, Some("rotation"))
                            .await?;
                    eprintln!(
                        "DEV-105: Revoked old token: {}, jti: {}",
                        revoked, &old_claims.jti
                    );

                    // Check for suspicious activity
                    let ip_str = ip_address.as_deref();
                    if RefreshToken::check_suspicious_activity(
                        tx,
                        existing_token.user_id,
                        device_fingerprint.as_deref(),
                        ip_str,
                    )
                    .await?
                    {
                        // Revoke all tokens for security
                        RefreshToken::revoke_all_for_user(tx, existing_token.user_id).await?;
                        return Err(JwtError::SuspiciousActivity);
                    }

                    // Fetch actual user data from database
                    let user = User::find_by_id(tx, existing_token.user_id).await?;

                    // Get user's permissions (OSS: everyone gets full permissions)
                    let user_scopes = PermissionConfig::get_default_permissions();

                    // Generate new token pair with actual user data
                    let new_access_token = self.generate_access_token(
                        &existing_token.user_id.to_string(),
                        &user.email,
                        &user.subscription_tier,
                        user_scopes,
                    )?;

                    // Generate new refresh token with same family
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|e| JwtError::KeyGenerationError(e.to_string()))?
                        .as_secs();

                    let new_jti = Uuid::new_v4().to_string();
                    let new_claims = RefreshTokenClaims {
                        sub: old_claims.sub.clone(),
                        jti: new_jti.clone(),
                        iat: now,
                        exp: now + self.config.refresh_token_expiry,
                    };

                    // Store new refresh token with same family
                    let expires_at = chrono::Utc::now()
                        + chrono::Duration::seconds(self.config.refresh_token_expiry as i64);

                    RefreshToken::store_in_transaction(
                        tx,
                        existing_token.user_id,
                        &new_jti,
                        expires_at,
                        existing_token.token_family.clone(), // Keep same family
                        DeviceInfo {
                            fingerprint: device_fingerprint.clone(),
                            ip_address,
                            user_agent: user_agent.clone(),
                        },
                    )
                    .await?;

                    // Old token already revoked above

                    // Encode new refresh token
                    let mut header = Header::new(self.config.algorithm);
                    header.kid = Some(self.config.key_version.to_string());

                    let new_refresh_token =
                        encode(&header, &new_claims, &self.config.refresh_encoding_key)?;

                    Ok((new_access_token, new_refresh_token))
                })
            })
            .await;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_generation() {
        // Use test config that doesn't rely on lazy static
        let config = JwtConfig::for_test();
        let service = JwtService::new(config);

        let token = service
            .generate_access_token(
                "test-user-id",
                "test@example.com",
                "free",
                vec!["read".to_string()],
            )
            .unwrap();

        assert!(!token.is_empty());
    }

    #[tokio::test]
    async fn test_token_validation() {
        // Use test config that doesn't rely on lazy static
        let config = JwtConfig::for_test();
        let service = JwtService::new(config);

        let token = service
            .generate_access_token(
                "test-user-id",
                "test@example.com",
                "free",
                vec!["read".to_string()],
            )
            .unwrap();

        let claims = service.validate_access_token(&token).unwrap();
        assert_eq!(claims.sub, "test-user-id");
        assert_eq!(claims.email, "test@example.com");
    }
}
