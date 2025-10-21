// Authentication models for QCK Backend (DEV-92)
// EXACT claims structures as specified in Linear requirements

use serde::{Deserialize, Serialize};

/// Access token claims structure (EXACT Linear DEV-113 requirements)
/// Contains user identification and subscription information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessTokenClaims {
    /// User ID (subject)
    pub sub: String,

    /// JWT ID for token revocation (UUID format)
    pub jti: String,

    /// User email address
    pub email: String,

    /// Subscription tier (free, basic, premium, enterprise)
    pub tier: String,

    /// Token scope/permissions (Linear DEV-113 requirement)
    pub scope: Vec<String>,

    /// Audience (aud) - Linear DEV-113 requirement
    pub aud: String,

    /// Issuer (iss) - Linear DEV-113 requirement  
    pub iss: String,

    /// Issued at timestamp (Unix epoch seconds)
    pub iat: u64,

    /// Expires at timestamp (Unix epoch seconds)
    pub exp: u64,
}

/// Refresh token claims structure (EXACT Linear requirements)
/// Contains minimal information for token refresh and revocation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefreshTokenClaims {
    /// User ID (subject)
    pub sub: String,

    /// JWT ID for token revocation (UUID format)
    pub jti: String,

    /// Issued at timestamp (Unix epoch seconds)
    pub iat: u64,

    /// Expires at timestamp (Unix epoch seconds)
    pub exp: u64,

    /// Remember me flag - determines if cookie should persist across browser sessions
    #[serde(default)]
    pub remember_me: bool,
}

impl AccessTokenClaims {
    /// Create new access token claims
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: String,
        token_id: String,
        email: String,
        tier: String,
        scope: Vec<String>,
        audience: String,
        issuer: String,
        issued_at: u64,
        expires_at: u64,
    ) -> Self {
        Self {
            sub: user_id,
            jti: token_id,
            email,
            tier,
            scope,
            aud: audience,
            iss: issuer,
            iat: issued_at,
            exp: expires_at,
        }
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.exp < now
    }
}

impl RefreshTokenClaims {
    /// Create new refresh token claims
    pub fn new(user_id: String, token_id: String, issued_at: u64, expires_at: u64) -> Self {
        Self {
            sub: user_id,
            jti: token_id,
            iat: issued_at,
            exp: expires_at,
            remember_me: false, // Default to session-only
        }
    }

    /// Create new refresh token claims with remember_me option
    pub fn new_with_remember(
        user_id: String,
        token_id: String,
        issued_at: u64,
        expires_at: u64,
        remember_me: bool,
    ) -> Self {
        Self {
            sub: user_id,
            jti: token_id,
            iat: issued_at,
            exp: expires_at,
            remember_me,
        }
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.exp < now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_access_token_claims_structure() {
        let jti = Uuid::new_v4().to_string();
        let claims = AccessTokenClaims::new(
            "user-123".to_string(),
            jti.clone(),
            "user@example.com".to_string(),
            "premium".to_string(),
            vec!["read".to_string(), "write".to_string()],
            "qck.sh".to_string(),
            "qck.sh".to_string(),
            1640995200, // 2022-01-01 00:00:00 UTC
            1640998800, // 2022-01-01 01:00:00 UTC
        );

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.jti, jti);
        assert_eq!(claims.email, "user@example.com");
        assert_eq!(claims.tier, "premium");
        assert_eq!(claims.scope, vec!["read".to_string(), "write".to_string()]);
        assert_eq!(claims.aud, "qck.sh");
        assert_eq!(claims.iss, "qck.sh");
        assert_eq!(claims.iat, 1640995200);
        assert_eq!(claims.exp, 1640998800);
    }

    #[test]
    fn test_refresh_token_claims_structure() {
        let jti = Uuid::new_v4().to_string();
        let claims =
            RefreshTokenClaims::new("user-456".to_string(), jti.clone(), 1640995200, 1641600000);

        assert_eq!(claims.sub, "user-456");
        assert_eq!(claims.jti, jti);
        assert_eq!(claims.iat, 1640995200);
        assert_eq!(claims.exp, 1641600000);
    }

    #[test]
    fn test_access_token_serialization() {
        let jti = Uuid::new_v4().to_string();
        let claims = AccessTokenClaims::new(
            "user-789".to_string(),
            jti,
            "test@example.com".to_string(),
            "enterprise".to_string(),
            vec!["admin".to_string(), "read".to_string(), "write".to_string()],
            "qck.sh".to_string(),
            "qck.sh".to_string(),
            1640995200,
            1640998800,
        );

        // Test serialization
        let json = serde_json::to_string(&claims).expect("Should serialize");
        let deserialized: AccessTokenClaims =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(claims, deserialized);
    }

    #[test]
    fn test_refresh_token_serialization() {
        let jti = Uuid::new_v4().to_string();
        let claims = RefreshTokenClaims::new("user-101".to_string(), jti, 1640995200, 1641600000);

        // Test serialization
        let json = serde_json::to_string(&claims).expect("Should serialize");
        let deserialized: RefreshTokenClaims =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(claims, deserialized);
    }

    #[test]
    fn test_token_expiry_check() {
        // Create expired token (1 second ago)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired_claims = AccessTokenClaims::new(
            "user-expired".to_string(),
            Uuid::new_v4().to_string(),
            "expired@example.com".to_string(),
            "basic".to_string(),
            vec!["read".to_string()],
            "qck.sh".to_string(),
            "qck.sh".to_string(),
            now - 3600, // 1 hour ago
            now - 1,    // 1 second ago
        );

        assert!(expired_claims.is_expired(), "Token should be expired");

        // Create valid token (expires in 1 hour)
        let valid_claims = AccessTokenClaims::new(
            "user-valid".to_string(),
            Uuid::new_v4().to_string(),
            "valid@example.com".to_string(),
            "premium".to_string(),
            vec!["read".to_string(), "write".to_string()],
            "qck.sh".to_string(),
            "qck.sh".to_string(),
            now,
            now + 3600, // 1 hour from now
        );

        assert!(!valid_claims.is_expired(), "Token should not be expired");
    }

    #[test]
    fn test_claims_exact_field_count() {
        // Verify AccessTokenClaims has exactly 6 fields (including jti)
        let claims = AccessTokenClaims::new(
            "test".to_string(),
            "test-jti".to_string(),
            "test@example.com".to_string(),
            "free".to_string(),
            vec!["read".to_string()],
            "qck.sh".to_string(),
            "qck.sh".to_string(),
            0,
            0,
        );

        let json_value = serde_json::to_value(&claims).expect("Should serialize");
        let obj = json_value.as_object().expect("Should be object");

        assert_eq!(
            obj.len(),
            9,
            "AccessTokenClaims should have exactly 9 fields"
        );
        assert!(obj.contains_key("sub"));
        assert!(obj.contains_key("jti"));
        assert!(obj.contains_key("email"));
        assert!(obj.contains_key("tier"));
        assert!(obj.contains_key("scope"));
        assert!(obj.contains_key("aud"));
        assert!(obj.contains_key("iss"));
        assert!(obj.contains_key("iat"));
        assert!(obj.contains_key("exp"));

        // Verify RefreshTokenClaims has exactly 5 fields (added remember_me)
        let refresh_claims =
            RefreshTokenClaims::new("test".to_string(), "test-jti".to_string(), 0, 0);

        let json_value = serde_json::to_value(&refresh_claims).expect("Should serialize");
        let obj = json_value.as_object().expect("Should be object");

        assert_eq!(
            obj.len(),
            5,
            "RefreshTokenClaims should have exactly 5 fields"
        );
        assert!(obj.contains_key("sub"));
        assert!(obj.contains_key("jti"));
        assert!(obj.contains_key("iat"));
        assert!(obj.contains_key("exp"));
        assert!(obj.contains_key("remember_me"));
    }

    #[test]
    fn test_refresh_token_with_remember_me() {
        let jti = Uuid::new_v4().to_string();

        // Test new_with_remember with remember_me=true
        let claims_with_remember = RefreshTokenClaims::new_with_remember(
            "user-123".to_string(),
            jti.clone(),
            1640995200,
            1641600000,
            true,
        );

        assert_eq!(claims_with_remember.sub, "user-123");
        assert_eq!(claims_with_remember.jti, jti);
        assert_eq!(claims_with_remember.iat, 1640995200);
        assert_eq!(claims_with_remember.exp, 1641600000);
        assert_eq!(claims_with_remember.remember_me, true);

        // Test new_with_remember with remember_me=false
        let jti2 = Uuid::new_v4().to_string();
        let claims_without_remember = RefreshTokenClaims::new_with_remember(
            "user-456".to_string(),
            jti2.clone(),
            1640995200,
            1641600000,
            false,
        );

        assert_eq!(claims_without_remember.remember_me, false);

        // Test that new() defaults to false
        let jti3 = Uuid::new_v4().to_string();
        let claims_default = RefreshTokenClaims::new(
            "user-789".to_string(),
            jti3,
            1640995200,
            1641600000,
        );

        assert_eq!(claims_default.remember_me, false);

        // Test serialization preserves remember_me flag
        let json = serde_json::to_string(&claims_with_remember).expect("Should serialize");
        let deserialized: RefreshTokenClaims =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(deserialized.remember_me, true);
    }
}
