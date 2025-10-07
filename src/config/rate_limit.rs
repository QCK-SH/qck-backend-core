// Centralized Rate Limiting Configuration (OSS)
// OSS version: Single configurable rate limit for all users (self-hosted)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::services::rate_limit::RateLimitConfig;

/// Global rate limiting configuration for OSS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Default configuration for all users and endpoints
    pub default: RateLimitConfig,

    /// Endpoint-specific configurations (auth, redirects, etc.)
    pub endpoints: HashMap<String, RateLimitConfig>,

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

    /// Sampling rate for analytics events (0.0-1.0)
    pub analytics_sample_rate: f64,
}

/// Emergency configuration for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencySettings {
    /// Emergency limit to apply globally (overrides all other limits)
    pub emergency_limit: Option<u32>,

    /// Emergency limit window in seconds
    pub emergency_window: u32,

    /// IP addresses to whitelist
    pub whitelist_ips: Vec<String>,

    /// IP addresses to blacklist
    pub blacklist_ips: Vec<String>,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        let mut endpoints = HashMap::new();

        // Auth endpoints - stricter limits
        let auth_config = RateLimitConfig {
            max_requests: std::env::var("RATE_LIMIT_AUTH_MAX")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            window_seconds: std::env::var("RATE_LIMIT_AUTH_WINDOW")
                .unwrap_or_else(|_| "900".to_string())
                .parse()
                .unwrap_or(900), // 15 minutes
            burst_limit: None,
            block_duration: 1800, // 30 minutes
            distributed: true,
        };

        endpoints.insert("/api/auth/login".to_string(), auth_config.clone());
        endpoints.insert("/api/auth/register".to_string(), auth_config.clone());
        endpoints.insert("/api/auth/forgot-password".to_string(), auth_config);

        // Password reset - very strict
        endpoints.insert(
            "/api/auth/reset-password".to_string(),
            RateLimitConfig {
                max_requests: 3,
                window_seconds: 3600, // 1 hour
                burst_limit: None,
                block_duration: 3600,
                distributed: true,
            },
        );

        // Token refresh - moderate limits
        endpoints.insert(
            "/api/auth/refresh".to_string(),
            RateLimitConfig {
                max_requests: 30,
                window_seconds: 3600,
                burst_limit: Some(5),
                block_duration: 600,
                distributed: true,
            },
        );

        // Links endpoints - configurable for self-hosted
        endpoints.insert(
            "/api/links".to_string(),
            RateLimitConfig {
                max_requests: std::env::var("RATE_LIMIT_LINKS_MAX")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
                window_seconds: std::env::var("RATE_LIMIT_LINKS_WINDOW")
                    .unwrap_or_else(|_| "3600".to_string())
                    .parse()
                    .unwrap_or(3600),
                burst_limit: Some(100),
                block_duration: 300,
                distributed: true,
            },
        );

        // Analytics endpoints
        endpoints.insert(
            "/api/analytics".to_string(),
            RateLimitConfig {
                max_requests: 500,
                window_seconds: 3600,
                burst_limit: Some(50),
                block_duration: 60,
                distributed: true,
            },
        );

        // Public endpoints (redirects) - very high limits
        endpoints.insert(
            "/redirect".to_string(),
            RateLimitConfig {
                max_requests: 100000,   // Very high for redirects
                window_seconds: 3600,
                burst_limit: Some(1000),
                block_duration: 0, // No blocking
                distributed: false,
            },
        );

        // Health check - no limits
        endpoints.insert(
            "/health".to_string(),
            RateLimitConfig {
                max_requests: 100000,
                window_seconds: 60,
                burst_limit: None,
                block_duration: 0,
                distributed: false,
            },
        );

        // Default configuration for all users (OSS - configurable)
        let default = RateLimitConfig {
            max_requests: std::env::var("RATE_LIMIT_DEFAULT_MAX")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .unwrap_or(10000),
            window_seconds: std::env::var("RATE_LIMIT_DEFAULT_WINDOW")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            burst_limit: Some(
                std::env::var("RATE_LIMIT_DEFAULT_BURST")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .unwrap_or(100),
            ),
            block_duration: std::env::var("RATE_LIMIT_DEFAULT_BLOCK")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),
            distributed: true,
        };

        // Global settings
        let global = GlobalRateLimitSettings {
            distributed: std::env::var("RATE_LIMIT_DISTRIBUTED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            default_block_duration: 300,
            monitoring: MonitoringSettings {
                enable_metrics: true,
                latency_warning_threshold_ms: 5,
                enable_analytics: true,
                analytics_sample_rate: 1.0,
            },
            emergency: EmergencySettings {
                emergency_limit: None,
                emergency_window: 60,
                whitelist_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
                blacklist_ips: vec![],
            },
        };

        Self {
            default,
            endpoints,
            global,
        }
    }
}

impl RateLimitingConfig {
    /// Load configuration from environment variables or use defaults
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Get rate limit configuration for an endpoint
    pub fn get_endpoint_config(&self, endpoint: &str) -> RateLimitConfig {
        // Check for exact match
        if let Some(config) = self.endpoints.get(endpoint) {
            return config.clone();
        }

        // Check for prefix match
        for (path, config) in &self.endpoints {
            if endpoint.starts_with(path) {
                return config.clone();
            }
        }

        // Return default
        self.default.clone()
    }

    /// Get specialized rate limit configuration by type
    pub fn get_specialized_config(&self, config_type: &str) -> RateLimitConfig {
        match config_type {
            "refresh" => RateLimitConfig {
                max_requests: 10,
                window_seconds: 3600,
                burst_limit: Some(3),
                block_duration: 600,
                distributed: true,
            },
            "validation" => RateLimitConfig {
                max_requests: 100,
                window_seconds: 3600,
                burst_limit: Some(10),
                block_duration: 300,
                distributed: true,
            },
            _ => self.default.clone(),
        }
    }

    /// Check if distributed rate limiting is enabled
    pub fn is_distributed(&self) -> bool {
        self.global.distributed
    }

    /// Check if an IP is whitelisted
    pub fn is_ip_whitelisted(&self, ip: &str) -> bool {
        self.global.emergency.whitelist_ips.contains(&ip.to_string())
    }

    /// Check if an IP is blacklisted
    pub fn is_ip_blacklisted(&self, ip: &str) -> bool {
        self.global.emergency.blacklist_ips.contains(&ip.to_string())
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

        // Check default config
        assert_eq!(config.default.window_seconds, 3600);

        // Check auth endpoint
        let auth_config = config.get_endpoint_config("/api/auth/login");
        assert!(auth_config.max_requests <= 10); // Stricter for auth

        // Check redirect endpoint
        let redirect_config = config.get_endpoint_config("/redirect");
        assert_eq!(redirect_config.max_requests, 100000); // Very high for redirects
    }

    #[test]
    fn test_endpoint_matching() {
        let config = RateLimitingConfig::default();

        // Exact match
        let auth_config = config.get_endpoint_config("/api/auth/login");
        assert!(auth_config.max_requests <= 10);

        // Prefix match
        let links_config = config.get_endpoint_config("/api/links/123");
        assert_eq!(links_config.window_seconds, 3600);

        // Default fallback
        let unknown_config = config.get_endpoint_config("/api/unknown");
        assert_eq!(unknown_config.max_requests, config.default.max_requests);
    }

    #[test]
    fn test_ip_lists() {
        let config = RateLimitingConfig::default();

        // Localhost is whitelisted by default
        assert!(config.is_ip_whitelisted("127.0.0.1"));
        assert!(config.is_ip_whitelisted("::1"));

        // Random IP not whitelisted
        assert!(!config.is_ip_whitelisted("192.168.1.1"));

        // No IPs blacklisted by default
        assert!(!config.is_ip_blacklisted("192.168.1.1"));
    }

    #[test]
    fn test_specialized_configs() {
        let config = RateLimitingConfig::default();

        let refresh_config = config.get_specialized_config("refresh");
        assert_eq!(refresh_config.max_requests, 10);

        let validation_config = config.get_specialized_config("validation");
        assert_eq!(validation_config.max_requests, 100);
    }
}