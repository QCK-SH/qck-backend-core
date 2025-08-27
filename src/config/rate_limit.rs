// Centralized Rate Limiting Configuration
// DEV-115: Configuration management for endpoint-specific and tier-based rate limits

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::services::rate_limit::RateLimitConfig;

/// Global rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Default configuration for unspecified endpoints
    pub default: RateLimitConfig,

    /// Endpoint-specific configurations
    pub endpoints: HashMap<String, RateLimitConfig>,

    /// Subscription tier configurations
    pub tiers: HashMap<String, RateLimitConfig>,

    /// Global settings
    pub global: GlobalRateLimitSettings,
}

/// Global rate limiting settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalRateLimitSettings {
    /// Enable distributed rate limiting across instances
    pub distributed: bool,

    /// Default block duration when no specific duration is set
    pub default_block_duration: u32,

    /// Performance monitoring settings
    pub monitoring: MonitoringSettings,

    /// Emergency settings
    pub emergency: EmergencySettings,
}

/// Monitoring configuration for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    /// Enable performance metrics collection
    pub enable_metrics: bool,

    /// Latency warning threshold in milliseconds
    pub latency_warning_threshold_ms: u64,

    /// Enable analytics event collection
    pub enable_analytics: bool,

    /// Sample rate for analytics (0.0 to 1.0)
    pub analytics_sample_rate: f64,
}

/// Emergency rate limiting settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencySettings {
    /// Emergency rate limit (overrides all other limits)
    pub emergency_limit: Option<u32>,

    /// Emergency limit window in seconds
    pub emergency_window: u32,

    /// Whitelist of IP addresses that bypass rate limits
    pub whitelist_ips: Vec<String>,

    /// Blacklist of IP addresses with permanent blocks
    pub blacklist_ips: Vec<String>,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        let mut endpoints = HashMap::new();
        let mut tiers = HashMap::new();

        // Define endpoint-specific configurations

        // Authentication endpoints - very strict
        endpoints.insert(
            "/api/auth/login".to_string(),
            RateLimitConfig {
                max_requests: 5,
                window_seconds: 900, // 15 minutes
                burst_limit: None,
                block_duration: 1800, // 30 minutes
                distributed: true,
            },
        );

        endpoints.insert(
            "/api/auth/register".to_string(),
            RateLimitConfig {
                max_requests: 3,
                window_seconds: 3600, // 1 hour
                burst_limit: None,
                block_duration: 3600, // 1 hour
                distributed: true,
            },
        );

        endpoints.insert(
            "/api/auth/refresh".to_string(),
            RateLimitConfig {
                max_requests: 10,
                window_seconds: 300, // 5 minutes
                burst_limit: Some(3),
                block_duration: 300,
                distributed: true,
            },
        );

        // Link management endpoints - tiered by subscription
        endpoints.insert(
            "/api/links".to_string(),
            RateLimitConfig {
                max_requests: 100, // Base rate, overridden by tier
                window_seconds: 3600,
                burst_limit: Some(10),
                block_duration: 60,
                distributed: true,
            },
        );

        endpoints.insert(
            "/api/links/bulk".to_string(),
            RateLimitConfig {
                max_requests: 10, // Bulk operations are more expensive
                window_seconds: 3600,
                burst_limit: Some(2),
                block_duration: 300,
                distributed: true,
            },
        );

        // Redirect endpoints - very high limits for performance
        endpoints.insert(
            "/r/*".to_string(),
            RateLimitConfig {
                max_requests: 10000,
                window_seconds: 60,
                burst_limit: Some(100),
                block_duration: 60,
                distributed: false, // Single instance handles redirects efficiently
            },
        );

        // Analytics endpoints
        endpoints.insert(
            "/api/analytics/*".to_string(),
            RateLimitConfig {
                max_requests: 100,
                window_seconds: 300, // 5 minutes
                burst_limit: Some(20),
                block_duration: 120,
                distributed: true,
            },
        );

        // Admin endpoints - moderate limits
        endpoints.insert(
            "/api/admin/*".to_string(),
            RateLimitConfig {
                max_requests: 500,
                window_seconds: 3600,
                burst_limit: Some(50),
                block_duration: 300,
                distributed: true,
            },
        );

        // Health check endpoint - very high limit (effectively unlimited)
        endpoints.insert(
            "/v1/health".to_string(),
            RateLimitConfig {
                max_requests: 1_000_000, // Very high limit (1M requests per minute)
                window_seconds: 60,
                burst_limit: None,
                block_duration: 0, // No blocking
                distributed: false,
            },
        );

        // Define subscription tier configurations

        // Your Subscription Tier Rate Limit Configuration
        // Easily configurable for future pricing/feature planning

        // FREE TIER - Conservative limits for cost control
        tiers.insert(
            "free".to_string(),
            RateLimitConfig {
                max_requests: 100,    // CONFIGURABLE: Starting point for free users
                window_seconds: 3600, // 1 hour window
                burst_limit: Some(5), // Small burst allowance
                block_duration: 300,  // 5 minutes cooldown
                distributed: true,
            },
        );

        // PERSONAL TIER - For individual users
        tiers.insert(
            "personal".to_string(),
            RateLimitConfig {
                max_requests: 1000,    // CONFIGURABLE: 10x free tier
                window_seconds: 3600,  // 1 hour window
                burst_limit: Some(25), // Moderate burst
                block_duration: 90,    // 1.5 minutes cooldown
                distributed: true,
            },
        );

        // PRO TIER - For businesses and power users
        tiers.insert(
            "pro".to_string(),
            RateLimitConfig {
                max_requests: 5000,     // CONFIGURABLE: 5x personal tier
                window_seconds: 3600,   // 1 hour window
                burst_limit: Some(100), // Good burst capacity
                block_duration: 60,     // 1 minute cooldown
                distributed: true,
            },
        );

        // ENTERPRISE TIER - For large organizations
        tiers.insert(
            "enterprise".to_string(),
            RateLimitConfig {
                max_requests: 15000,    // CONFIGURABLE: 3x pro tier
                window_seconds: 3600,   // 1 hour window
                burst_limit: Some(300), // Large burst capacity
                block_duration: 30,     // Short cooldown
                distributed: true,
            },
        );

        // API TIER - For API-heavy customers
        tiers.insert(
            "api".to_string(),
            RateLimitConfig {
                max_requests: 50000,     // CONFIGURABLE: Very high for API usage
                window_seconds: 3600,    // 1 hour window
                burst_limit: Some(1000), // Large burst for API calls
                block_duration: 10,      // Very short cooldown
                distributed: true,
            },
        );

        // Default configuration for unspecified endpoints
        let default = RateLimitConfig {
            max_requests: 1000,
            window_seconds: 3600,
            burst_limit: Some(20),
            block_duration: 300,
            distributed: true,
        };

        // Global settings
        let global = GlobalRateLimitSettings {
            distributed: true,
            default_block_duration: 300, // 5 minutes
            monitoring: MonitoringSettings {
                enable_metrics: true,
                latency_warning_threshold_ms: 5, // DEV-115 requirement: <5ms
                enable_analytics: true,
                analytics_sample_rate: 1.0, // 100% sampling for development
            },
            emergency: EmergencySettings {
                emergency_limit: None, // No emergency limit by default
                emergency_window: 60,
                whitelist_ips: vec!["127.0.0.1".to_string(), "::1".to_string()], // Localhost always whitelisted
                blacklist_ips: vec![], // No IPs blacklisted by default
            },
        };

        Self {
            default,
            endpoints,
            tiers,
            global,
        }
    }
}

impl RateLimitingConfig {
    /// Load configuration from environment variables or use defaults
    pub fn from_env() -> Self {
        // TODO: Load from environment variables in a future iteration
        // For now, return default configuration
        Self::default()
    }

    /// Get specialized rate limit configuration by type
    /// Provides a centralized way to access specific rate limiting configurations
    pub fn get_specialized_config(&self, config_type: &str) -> RateLimitConfig {
        match config_type {
            "refresh" => RateLimitConfig {
                max_requests: 10,
                window_seconds: 300, // 5 minutes
                burst_limit: Some(3),
                block_duration: 300,
                distributed: true,
            },
            "login" => self.get_endpoint_config("/api/auth/login").clone(),
            "register" => self.get_endpoint_config("/api/auth/register").clone(),
            "redirect" => self.get_endpoint_config("/r/*").clone(),
            "bulk_operations" => RateLimitConfig {
                max_requests: 10,
                window_seconds: 3600,
                burst_limit: Some(2),
                block_duration: 300,
                distributed: true,
            },
            _ => self.default.clone(),
        }
    }

    /// Get configuration for a specific endpoint
    pub fn get_endpoint_config(&self, endpoint: &str) -> &RateLimitConfig {
        // Direct match
        if let Some(config) = self.endpoints.get(endpoint) {
            return config;
        }

        // Pattern matching for wildcard endpoints
        for (pattern, config) in &self.endpoints {
            if pattern.ends_with("/*") {
                let prefix = &pattern[..pattern.len() - 2];
                if endpoint.starts_with(prefix) {
                    return config;
                }
            }
        }

        // Return default if no match found
        &self.default
    }

    /// Get configuration for a subscription tier
    pub fn get_tier_config(&self, tier: &str) -> &RateLimitConfig {
        self.tiers.get(tier).unwrap_or(&self.default)
    }

    /// Check if an IP address is whitelisted
    pub fn is_whitelisted_ip(&self, ip: &str) -> bool {
        self.global
            .emergency
            .whitelist_ips
            .contains(&ip.to_string())
    }

    /// Check if an IP address is blacklisted
    pub fn is_blacklisted_ip(&self, ip: &str) -> bool {
        self.global
            .emergency
            .blacklist_ips
            .contains(&ip.to_string())
    }

    /// Get emergency rate limit if active
    pub fn get_emergency_limit(&self) -> Option<u32> {
        self.global.emergency.emergency_limit
    }

    /// Create a merged configuration for user + endpoint
    pub fn create_user_endpoint_config(&self, tier: &str, endpoint: &str) -> RateLimitConfig {
        let tier_config = self.get_tier_config(tier);
        let endpoint_config = self.get_endpoint_config(endpoint);

        // Use the more restrictive limits between tier and endpoint
        RateLimitConfig {
            max_requests: std::cmp::min(tier_config.max_requests, endpoint_config.max_requests),
            window_seconds: std::cmp::max(
                tier_config.window_seconds,
                endpoint_config.window_seconds,
            ),
            burst_limit: match (tier_config.burst_limit, endpoint_config.burst_limit) {
                (Some(tier_burst), Some(endpoint_burst)) => {
                    Some(std::cmp::min(tier_burst, endpoint_burst))
                },
                (Some(burst), None) | (None, Some(burst)) => Some(burst),
                (None, None) => None,
            },
            block_duration: std::cmp::max(
                tier_config.block_duration,
                endpoint_config.block_duration,
            ),
            distributed: tier_config.distributed || endpoint_config.distributed,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate default configuration
        if self.default.max_requests == 0 {
            return Err("Default max_requests cannot be zero".to_string());
        }

        if self.default.window_seconds == 0 {
            return Err("Default window_seconds cannot be zero".to_string());
        }

        // Validate endpoint configurations
        for (endpoint, config) in &self.endpoints {
            if config.max_requests == 0 {
                return Err(format!("Endpoint {} max_requests cannot be zero", endpoint));
            }

            if config.window_seconds == 0 {
                return Err(format!(
                    "Endpoint {} window_seconds cannot be zero",
                    endpoint
                ));
            }

            if let Some(burst) = config.burst_limit {
                if burst == 0 {
                    return Err(format!("Endpoint {} burst_limit cannot be zero", endpoint));
                }
            }
        }

        // Validate tier configurations
        for (tier, config) in &self.tiers {
            if config.max_requests == 0 {
                return Err(format!("Tier {} max_requests cannot be zero", tier));
            }

            if config.window_seconds == 0 {
                return Err(format!("Tier {} window_seconds cannot be zero", tier));
            }
        }

        // Validate monitoring settings
        if self.global.monitoring.analytics_sample_rate < 0.0
            || self.global.monitoring.analytics_sample_rate > 1.0
        {
            return Err("Analytics sample rate must be between 0.0 and 1.0".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration() {
        let config = RateLimitingConfig::default();

        // Test endpoint configurations
        assert!(config.endpoints.contains_key("/api/auth/login"));
        assert!(config.endpoints.contains_key("/r/*"));
        assert!(config.endpoints.contains_key("/api/links"));

        // Test tier configurations
        assert!(config.tiers.contains_key("free"));
        assert!(config.tiers.contains_key("enterprise"));

        // Test validation
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_endpoint_config_lookup() {
        let config = RateLimitingConfig::default();

        // Direct match
        let login_config = config.get_endpoint_config("/api/auth/login");
        assert_eq!(login_config.max_requests, 5);

        // Wildcard match
        let redirect_config = config.get_endpoint_config("/r/abc123");
        assert_eq!(redirect_config.max_requests, 10000);

        // Default fallback
        let unknown_config = config.get_endpoint_config("/unknown/endpoint");
        assert_eq!(unknown_config.max_requests, 1000);
    }

    #[test]
    fn test_tier_config_lookup() {
        let config = RateLimitingConfig::default();

        let free_config = config.get_tier_config("free");
        assert_eq!(free_config.max_requests, 100);

        let enterprise_config = config.get_tier_config("enterprise");
        assert_eq!(enterprise_config.max_requests, 15000);

        let unknown_config = config.get_tier_config("unknown");
        assert_eq!(unknown_config.max_requests, 1000); // Default
    }

    #[test]
    fn test_merged_user_endpoint_config() {
        let config = RateLimitingConfig::default();

        // Test merging free tier with auth endpoint (should use more restrictive)
        let merged = config.create_user_endpoint_config("free", "/api/auth/login");
        assert_eq!(merged.max_requests, 5); // Auth endpoint is more restrictive

        // Test merging enterprise tier with default endpoint
        let merged = config.create_user_endpoint_config("enterprise", "/api/unknown");
        assert_eq!(merged.max_requests, 1000); // Default endpoint is more restrictive
    }

    #[test]
    fn test_ip_whitelist_blacklist() {
        let config = RateLimitingConfig::default();

        assert!(config.is_whitelisted_ip("127.0.0.1"));
        assert!(config.is_whitelisted_ip("::1"));
        assert!(!config.is_whitelisted_ip("192.168.1.1"));

        assert!(!config.is_blacklisted_ip("127.0.0.1")); // No IPs blacklisted by default
    }

    #[test]
    fn test_configuration_validation() {
        let mut config = RateLimitingConfig::default();

        // Valid configuration should pass
        assert!(config.validate().is_ok());

        // Invalid analytics sample rate should fail
        config.global.monitoring.analytics_sample_rate = 1.5;
        assert!(config.validate().is_err());

        config.global.monitoring.analytics_sample_rate = -0.5;
        assert!(config.validate().is_err());

        // Reset to valid value
        config.global.monitoring.analytics_sample_rate = 0.5;
        assert!(config.validate().is_ok());

        // Invalid endpoint configuration should fail
        config.endpoints.insert(
            "/test".to_string(),
            RateLimitConfig {
                max_requests: 0, // Invalid
                window_seconds: 60,
                burst_limit: None,
                block_duration: 30,
                distributed: true,
            },
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_specialized_config() {
        let config = RateLimitingConfig::default();

        // Test refresh configuration
        let refresh_config = config.get_specialized_config("refresh");
        assert_eq!(refresh_config.max_requests, 10);
        assert_eq!(refresh_config.window_seconds, 300);
        assert_eq!(refresh_config.burst_limit, Some(3));
        assert!(refresh_config.distributed);

        // Test login configuration (should match endpoint config)
        let login_config = config.get_specialized_config("login");
        assert_eq!(login_config.max_requests, 5);

        // Test bulk operations configuration
        let bulk_config = config.get_specialized_config("bulk_operations");
        assert_eq!(bulk_config.max_requests, 10);
        assert_eq!(bulk_config.window_seconds, 3600);

        // Test unknown configuration (should return default)
        let unknown_config = config.get_specialized_config("unknown");
        assert_eq!(unknown_config.max_requests, 1000);
    }
}
