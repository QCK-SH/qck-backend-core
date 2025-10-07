// Permission configuration for QCK Backend (OSS)
// OSS version: No tiers, everyone gets full permissions (self-hosted)

use std::collections::HashMap;

/// Permission configuration for OSS
/// Since this is self-hosted, all users have full access
pub struct PermissionConfig;

impl PermissionConfig {
    /// Get permissions for all users (OSS has no tiers)
    pub fn get_default_permissions() -> Vec<String> {
        vec![
            "admin".to_string(),
            "links:unlimited".to_string(),
            "analytics:full".to_string(),
            "domains:custom".to_string(),
            "api:unlimited".to_string(),
            "teams:manage".to_string(),
            "bulk_operations".to_string(),
        ]
    }

    /// Get features for all users (OSS has all features enabled)
    pub fn get_default_features() -> HashMap<String, bool> {
        let mut features = HashMap::new();

        // All features enabled in OSS
        features.insert("custom_domains".to_string(), true);
        features.insert("advanced_analytics".to_string(), true);
        features.insert("api_access".to_string(), true);
        features.insert("team_management".to_string(), true);
        features.insert("bulk_operations".to_string(), true);
        features.insert("white_label".to_string(), true);
        features.insert("sso".to_string(), true);

        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_permissions() {
        let perms = PermissionConfig::get_default_permissions();
        assert!(perms.contains(&"admin".to_string()));
        assert!(perms.contains(&"links:unlimited".to_string()));
        assert!(perms.contains(&"api:unlimited".to_string()));
    }

    #[test]
    fn test_default_features() {
        let features = PermissionConfig::get_default_features();
        assert_eq!(features.get("custom_domains"), Some(&true));
        assert_eq!(features.get("sso"), Some(&true));
        assert_eq!(features.get("api_access"), Some(&true));
        assert_eq!(features.get("bulk_operations"), Some(&true));
    }
}