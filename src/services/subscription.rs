// Subscription Tier Management Service
// DEV-115: Subscription tier integration for rate limiting

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::services::rate_limit::RateLimitConfig;

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Debug, Error)]
pub enum SubscriptionError {
    #[error("Unknown subscription tier: {0}")]
    UnknownTier(String),

    #[error("Invalid tier configuration")]
    InvalidConfig,
}

// =============================================================================
// CONSTANTS
// =============================================================================

/// Rate limit for API endpoints when user doesn't have API access
const RESTRICTED_API_LIMIT: u32 = 10;

// =============================================================================
// SUBSCRIPTION TIER DEFINITIONS
// =============================================================================

/// Subscription tier with associated limits and features
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionTier {
    /// Tier name (free, personal, pro, enterprise, api)
    pub name: String,

    /// Display name for the tier
    pub display_name: String,

    /// Rate limiting configuration for this tier
    pub rate_limits: RateLimitConfig,

    /// Feature flags enabled for this tier
    pub features: Vec<String>,

    /// Maximum number of links this tier can create
    pub max_links: Option<u32>,

    /// Maximum number of custom domains
    pub max_custom_domains: u32,

    /// Whether this tier has analytics access
    pub has_analytics: bool,

    /// Whether this tier has API access
    pub has_api_access: bool,

    /// Priority level for support (higher = better)
    pub support_priority: u8,
}

impl SubscriptionTier {
    /// Create a free tier subscription
    pub fn free() -> Self {
        Self {
            name: "free".to_string(),
            display_name: "Free".to_string(),
            rate_limits: RateLimitConfig {
                max_requests: 100,
                window_seconds: 3600, // 1 hour
                burst_limit: Some(5),
                block_duration: 300, // 5 minutes
                distributed: true,
            },
            features: vec!["basic_links".to_string()],
            max_links: Some(100),
            max_custom_domains: 0,
            has_analytics: false,
            has_api_access: false,
            support_priority: 1,
        }
    }

    /// Create a pro tier subscription
    pub fn pro() -> Self {
        Self {
            name: "pro".to_string(),
            display_name: "Pro".to_string(),
            rate_limits: RateLimitConfig {
                max_requests: 1000,
                window_seconds: 3600,
                burst_limit: Some(20),
                block_duration: 60, // 1 minute
                distributed: true,
            },
            features: vec![
                "basic_links".to_string(),
                "qr_codes".to_string(),
                "advanced_analytics".to_string(),
                "custom_domains".to_string(),
                "api_access".to_string(),
                "link_expiration".to_string(),
            ],
            max_links: Some(10000),
            max_custom_domains: 5,
            has_analytics: true,
            has_api_access: true,
            support_priority: 3,
        }
    }

    /// Create a personal tier subscription (individual users)
    pub fn personal() -> Self {
        Self {
            name: "personal".to_string(),
            display_name: "Personal".to_string(),
            rate_limits: RateLimitConfig {
                max_requests: 1000,    // Between basic and pro
                window_seconds: 3600,  // 1 hour
                burst_limit: Some(25), // Moderate burst
                block_duration: 90,    // 1.5 minutes cooldown
                distributed: true,
            },
            features: vec![
                "basic_links".to_string(),
                "qr_codes".to_string(),
                "basic_analytics".to_string(),
                "custom_aliases".to_string(),
            ],
            max_links: Some(5000), // Personal usage limit
            max_custom_domains: 2, // Few personal domains
            has_analytics: true,
            has_api_access: false, // No API for personal
            support_priority: 2,
        }
    }

    /// Create an enterprise tier subscription
    pub fn enterprise() -> Self {
        Self {
            name: "enterprise".to_string(),
            display_name: "Enterprise".to_string(),
            rate_limits: RateLimitConfig {
                max_requests: 15000, // Higher than pro
                window_seconds: 3600,
                burst_limit: Some(300), // Large burst capacity
                block_duration: 30,     // Short cooldown
                distributed: true,
            },
            features: vec![
                "basic_links".to_string(),
                "qr_codes".to_string(),
                "advanced_analytics".to_string(),
                "custom_domains".to_string(),
                "api_access".to_string(),
                "link_expiration".to_string(),
                "white_label".to_string(),
                "sso".to_string(),
                "priority_support".to_string(),
                "bulk_operations".to_string(),
                "team_management".to_string(),
            ],
            max_links: None,        // Unlimited
            max_custom_domains: 50, // Enterprise domains
            has_analytics: true,
            has_api_access: true,
            support_priority: 5,
        }
    }

    /// Create an API tier subscription (API-focused customers)
    pub fn api() -> Self {
        Self {
            name: "api".to_string(),
            display_name: "API".to_string(),
            rate_limits: RateLimitConfig {
                max_requests: 50000, // Very high for API usage
                window_seconds: 3600,
                burst_limit: Some(1000), // Large burst for API calls
                block_duration: 10,      // Very short cooldown
                distributed: true,
            },
            features: vec![
                "basic_links".to_string(),
                "api_access".to_string(),
                "advanced_analytics".to_string(),
                "bulk_operations".to_string(),
                "webhooks".to_string(),
                "rate_limit_headers".to_string(),
                "api_keys".to_string(),
                "developer_tools".to_string(),
            ],
            max_links: None,        // Unlimited for API usage
            max_custom_domains: 10, // Moderate domains
            has_analytics: true,
            has_api_access: true,
            support_priority: 4, // High priority for API customers
        }
    }

    /// Check if this tier has a specific feature
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.contains(&feature.to_string())
    }

    /// Get the effective rate limit for a specific endpoint
    pub fn get_endpoint_rate_limit(&self, endpoint: &str) -> RateLimitConfig {
        let mut config = self.rate_limits.clone();

        // Apply endpoint-specific adjustments based on tier
        match endpoint {
            path if path.starts_with("/api/links") => {
                // Link creation endpoints get tier-specific limits
                config
            },
            path if path.starts_with("/api/analytics") => {
                if !self.has_analytics {
                    // No analytics access - very restrictive
                    config.max_requests = 0;
                }
                config
            },
            path if path.starts_with("/api/") => {
                if !self.has_api_access {
                    // No API access - very restrictive
                    config.max_requests = RESTRICTED_API_LIMIT;
                    config.window_seconds = 3600;
                    config.burst_limit = None;
                }
                config
            },
            _ => config,
        }
    }
}

// =============================================================================
// SUBSCRIPTION SERVICE
// =============================================================================

/// Service for managing subscription tiers and their associated limits
pub struct SubscriptionService {
    tiers: HashMap<String, SubscriptionTier>,
}

impl Default for SubscriptionService {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionService {
    /// Create a new subscription service with all your planned tiers
    pub fn new() -> Self {
        let mut tiers = HashMap::new();

        // Your specific tier structure: free, personal, pro, enterprise, API
        let free_tier = SubscriptionTier::free();
        let personal_tier = SubscriptionTier::personal();
        let pro_tier = SubscriptionTier::pro();
        let enterprise_tier = SubscriptionTier::enterprise();
        let api_tier = SubscriptionTier::api();

        tiers.insert(free_tier.name.clone(), free_tier);
        tiers.insert(personal_tier.name.clone(), personal_tier);
        tiers.insert(pro_tier.name.clone(), pro_tier);
        tiers.insert(enterprise_tier.name.clone(), enterprise_tier);
        tiers.insert(api_tier.name.clone(), api_tier);

        Self { tiers }
    }

    /// Get subscription tier by name
    pub fn get_tier(&self, tier_name: &str) -> Result<&SubscriptionTier, SubscriptionError> {
        self.tiers
            .get(tier_name)
            .ok_or_else(|| SubscriptionError::UnknownTier(tier_name.to_string()))
    }

    /// Get all available tiers
    pub fn get_all_tiers(&self) -> Vec<&SubscriptionTier> {
        self.tiers.values().collect()
    }

    /// Check if a tier can access a specific feature
    pub fn can_access_feature(&self, tier_name: &str, feature: &str) -> bool {
        self.get_tier(tier_name)
            .map(|tier| tier.has_feature(feature))
            .unwrap_or(false)
    }

    /// Get rate limit configuration for a user's tier and endpoint
    pub fn get_user_rate_limit(
        &self,
        tier_name: &str,
        endpoint: &str,
    ) -> Result<RateLimitConfig, SubscriptionError> {
        let tier = self.get_tier(tier_name)?;
        Ok(tier.get_endpoint_rate_limit(endpoint))
    }

    /// Check if a user can perform a bulk operation
    pub fn can_perform_bulk_operation(&self, tier_name: &str) -> bool {
        self.can_access_feature(tier_name, "bulk_operations")
    }

    /// Get the maximum number of links for a tier
    pub fn get_max_links(&self, tier_name: &str) -> Result<Option<u32>, SubscriptionError> {
        let tier = self.get_tier(tier_name)?;
        Ok(tier.max_links)
    }

    /// Get tier hierarchy level (higher = better tier)
    pub fn get_tier_level(&self, tier_name: &str) -> Result<u8, SubscriptionError> {
        match tier_name {
            "free" => Ok(1),
            "personal" => Ok(2),
            "pro" => Ok(3),
            "api" => Ok(3), // Same level as pro, different use case
            "enterprise" => Ok(4),
            _ => Err(SubscriptionError::UnknownTier(tier_name.to_string())),
        }
    }

    /// Check if tier A is higher than tier B
    pub fn is_higher_tier(&self, tier_a: &str, tier_b: &str) -> Result<bool, SubscriptionError> {
        let level_a = self.get_tier_level(tier_a)?;
        let level_b = self.get_tier_level(tier_b)?;
        Ok(level_a > level_b)
    }

    /// Get recommended upgrade tier for a user
    pub fn get_upgrade_recommendation(
        &self,
        current_tier: &str,
    ) -> Result<Option<&str>, SubscriptionError> {
        match current_tier {
            "free" => Ok(Some("personal")),
            "personal" => Ok(Some("pro")),
            "pro" => Ok(Some("enterprise")),
            "api" => Ok(Some("enterprise")), // API users can upgrade to enterprise
            "enterprise" => Ok(None),        // Already at the highest tier
            _ => Err(SubscriptionError::UnknownTier(current_tier.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_creation() {
        let free_tier = SubscriptionTier::free();
        assert_eq!(free_tier.name, "free");
        assert_eq!(free_tier.rate_limits.max_requests, 100);
        assert!(!free_tier.has_analytics);

        let enterprise_tier = SubscriptionTier::enterprise();
        assert_eq!(enterprise_tier.name, "enterprise");
        assert_eq!(enterprise_tier.rate_limits.max_requests, 15000);
        assert!(enterprise_tier.has_analytics);
        assert!(enterprise_tier.max_links.is_none()); // Unlimited
    }

    #[test]
    fn test_subscription_service() {
        let service = SubscriptionService::new();

        // Test tier retrieval
        let free_tier = service.get_tier("free").unwrap();
        assert_eq!(free_tier.name, "free");

        // Test unknown tier
        assert!(service.get_tier("unknown").is_err());

        // Test feature access
        assert!(service.can_access_feature("pro", "api_access"));
        assert!(!service.can_access_feature("free", "api_access"));

        // Test tier levels
        assert!(service.is_higher_tier("pro", "personal").unwrap());
        assert!(!service.is_higher_tier("personal", "pro").unwrap());
    }

    #[test]
    fn test_endpoint_rate_limits() {
        let pro_tier = SubscriptionTier::pro();

        // Test link creation endpoint
        let link_limit = pro_tier.get_endpoint_rate_limit("/api/links");
        assert_eq!(link_limit.max_requests, 1000);

        // Test analytics endpoint
        let analytics_limit = pro_tier.get_endpoint_rate_limit("/api/analytics");
        assert_eq!(analytics_limit.max_requests, 1000); // Pro has analytics access

        // Test with free tier (no analytics)
        let free_tier = SubscriptionTier::free();
        let free_analytics_limit = free_tier.get_endpoint_rate_limit("/api/analytics");
        assert_eq!(free_analytics_limit.max_requests, 0); // No access
    }

    #[test]
    fn test_upgrade_recommendations() {
        let service = SubscriptionService::new();

        assert_eq!(
            service.get_upgrade_recommendation("free").unwrap(),
            Some("personal")
        );
        assert_eq!(
            service.get_upgrade_recommendation("personal").unwrap(),
            Some("pro")
        );
        assert_eq!(
            service.get_upgrade_recommendation("pro").unwrap(),
            Some("enterprise")
        );
        assert_eq!(
            service.get_upgrade_recommendation("enterprise").unwrap(),
            None
        );
    }
}
