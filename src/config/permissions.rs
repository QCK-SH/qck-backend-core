// Permission configuration for QCK Backend
// DEV-113: Centralized permission mappings for subscription tiers

use std::collections::HashMap;

/// Permission configuration for subscription tiers
pub struct PermissionConfig;

impl PermissionConfig {
    /// Get permissions for a subscription tier
    pub fn get_tier_permissions(tier: &str) -> Vec<String> {
        match tier {
            "enterprise" => vec![
                "admin".to_string(),
                "premium".to_string(),
                "basic".to_string(),
                "links:unlimited".to_string(),
                "analytics:advanced".to_string(),
                "domains:custom".to_string(),
                "api:unlimited".to_string(),
                "teams:manage".to_string(),
                "billing:manage".to_string(),
            ],
            "premium" => vec![
                "premium".to_string(),
                "basic".to_string(),
                "links:1000".to_string(),
                "analytics:basic".to_string(),
                "domains:5".to_string(),
                "api:10000".to_string(),
                "teams:view".to_string(),
            ],
            "basic" => vec![
                "basic".to_string(),
                "links:100".to_string(),
                "analytics:limited".to_string(),
                "api:1000".to_string(),
            ],
            _ => vec![
                "free".to_string(),
                "links:10".to_string(),
                "api:100".to_string(),
            ], // free tier
        }
    }

    /// Get rate limits for a subscription tier
    pub fn get_tier_rate_limits(tier: &str) -> TierRateLimits {
        match tier {
            "enterprise" => TierRateLimits {
                requests_per_minute: 1000,
                requests_per_hour: 50000,
                burst_capacity: 100,
            },
            "premium" => TierRateLimits {
                requests_per_minute: 100,
                requests_per_hour: 5000,
                burst_capacity: 50,
            },
            "basic" => TierRateLimits {
                requests_per_minute: 60,
                requests_per_hour: 1000,
                burst_capacity: 20,
            },
            _ => TierRateLimits {
                requests_per_minute: 10,
                requests_per_hour: 100,
                burst_capacity: 5,
            },
        }
    }

    /// Get feature flags for a subscription tier
    pub fn get_tier_features(tier: &str) -> HashMap<String, bool> {
        let mut features = HashMap::new();

        match tier {
            "enterprise" => {
                features.insert("custom_domains".to_string(), true);
                features.insert("advanced_analytics".to_string(), true);
                features.insert("api_access".to_string(), true);
                features.insert("team_management".to_string(), true);
                features.insert("bulk_operations".to_string(), true);
                features.insert("white_label".to_string(), true);
                features.insert("sso".to_string(), true);
            },
            "premium" => {
                features.insert("custom_domains".to_string(), true);
                features.insert("advanced_analytics".to_string(), false);
                features.insert("api_access".to_string(), true);
                features.insert("team_management".to_string(), false);
                features.insert("bulk_operations".to_string(), true);
                features.insert("white_label".to_string(), false);
                features.insert("sso".to_string(), false);
            },
            "basic" => {
                features.insert("custom_domains".to_string(), false);
                features.insert("advanced_analytics".to_string(), false);
                features.insert("api_access".to_string(), true);
                features.insert("team_management".to_string(), false);
                features.insert("bulk_operations".to_string(), false);
                features.insert("white_label".to_string(), false);
                features.insert("sso".to_string(), false);
            },
            _ => {
                features.insert("custom_domains".to_string(), false);
                features.insert("advanced_analytics".to_string(), false);
                features.insert("api_access".to_string(), false);
                features.insert("team_management".to_string(), false);
                features.insert("bulk_operations".to_string(), false);
                features.insert("white_label".to_string(), false);
                features.insert("sso".to_string(), false);
            },
        }

        features
    }
}

/// Rate limit configuration for a tier
#[derive(Debug, Clone)]
pub struct TierRateLimits {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub burst_capacity: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_permissions() {
        let enterprise_perms = PermissionConfig::get_tier_permissions("enterprise");
        assert!(enterprise_perms.contains(&"admin".to_string()));
        assert!(enterprise_perms.contains(&"links:unlimited".to_string()));

        let free_perms = PermissionConfig::get_tier_permissions("free");
        assert!(free_perms.contains(&"free".to_string()));
        assert!(free_perms.contains(&"links:10".to_string()));
    }

    #[test]
    fn test_tier_rate_limits() {
        let enterprise_limits = PermissionConfig::get_tier_rate_limits("enterprise");
        assert_eq!(enterprise_limits.requests_per_minute, 1000);
        assert_eq!(enterprise_limits.burst_capacity, 100);

        let free_limits = PermissionConfig::get_tier_rate_limits("free");
        assert_eq!(free_limits.requests_per_minute, 10);
        assert_eq!(free_limits.burst_capacity, 5);
    }

    #[test]
    fn test_tier_features() {
        let enterprise_features = PermissionConfig::get_tier_features("enterprise");
        assert_eq!(enterprise_features.get("custom_domains"), Some(&true));
        assert_eq!(enterprise_features.get("sso"), Some(&true));

        let free_features = PermissionConfig::get_tier_features("free");
        assert_eq!(free_features.get("custom_domains"), Some(&false));
        assert_eq!(free_features.get("api_access"), Some(&false));
    }
}
