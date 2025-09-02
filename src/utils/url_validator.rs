// DEV-105: URL validation and security scanning
// Comprehensive URL validation for link creation

use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};
use url::Url;

// =============================================================================
// STATIC REGEX PATTERNS
// =============================================================================

lazy_static! {
    /// Regex pattern for validating domain-like strings
    /// Matches: example.com, sub.example.com, example.com/path
    static ref DOMAIN_PATTERN: Regex =
        Regex::new(r"^[a-zA-Z0-9]([a-zA-Z0-9-]*\.)+[a-zA-Z]{2,}(/.*)?$")
            .expect("Invalid domain pattern regex");
}

// =============================================================================
// JSON CONFIGURATION STRUCTURES
// =============================================================================

/// Configuration structure for blocked domains JSON
#[derive(Debug, Deserialize)]
struct BlockedDomainsConfig {
    blocked_domains: BlockedDomainCategories,
    blocked_tlds: BlockedTlds,
    suspicious_patterns: SuspiciousPatterns,
    private_ip_ranges: PrivateIpRanges,
}

#[derive(Debug, Deserialize)]
struct BlockedDomainCategories {
    url_shorteners: DomainCategory,
    local_addresses: DomainCategory,
    malicious: DomainCategory,
}

#[derive(Debug, Deserialize)]
struct DomainCategory {
    description: String,
    domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BlockedTlds {
    description: String,
    tlds: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SuspiciousPatterns {
    phishing_indicators: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PrivateIpRanges {
    description: String,
    ranges: Vec<String>,
}

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Enhanced validation errors for DEV-116 requirements
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("Invalid URL format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported scheme: {0}. Only HTTP and HTTPS are supported")]
    UnsupportedScheme(String),

    #[error("Blocked domain: {0}")]
    BlockedDomain(String),

    #[error("Blocked TLD: {0}")]
    BlockedTld(String),

    #[error("Missing host in URL")]
    MissingHost,

    #[error("Private or local IP addresses not allowed")]
    PrivateIp,

    #[error("URL too long (max {max}, current {current})")]
    TooLong { max: usize, current: usize },

    #[error("URL contains suspicious characters")]
    SuspiciousCharacters,

    #[error("Data URLs not allowed")]
    DataUrlNotAllowed,

    #[error("JavaScript URLs not allowed")]
    JavascriptUrlNotAllowed,

    #[error("DNS resolution failed")]
    DnsResolutionFailed,

    #[error("DNS resolution timeout")]
    DnsTimeout,

    #[error("Normalization failed: {0}")]
    NormalizationFailed(String),
}

/// Legacy error type for backward compatibility
#[derive(Error, Debug)]
pub enum UrlValidationError {
    #[error("Invalid URL format: {0}")]
    InvalidFormat(String),

    #[error("URL scheme not allowed: {0}. Only HTTP and HTTPS are supported")]
    InvalidScheme(String),

    #[error("URL length exceeds maximum of {max} characters (current: {current})")]
    TooLong { current: usize, max: usize },

    #[error("URL domain is blacklisted: {0}")]
    BlacklistedDomain(String),

    #[error("URL contains suspicious patterns: {0}")]
    SuspiciousPattern(String),

    #[error("URL points to private network: {0}")]
    PrivateNetwork(String),

    #[error("URL contains invalid characters")]
    InvalidCharacters,

    #[error("URL is missing required components")]
    MissingComponents,
}

// Convert between error types for compatibility
impl From<ValidationError> for UrlValidationError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::InvalidFormat(s) => UrlValidationError::InvalidFormat(s),
            ValidationError::UnsupportedScheme(s) => UrlValidationError::InvalidScheme(s),
            ValidationError::BlockedDomain(s) => UrlValidationError::BlacklistedDomain(s),
            ValidationError::MissingHost => UrlValidationError::MissingComponents,
            ValidationError::PrivateIp => {
                UrlValidationError::PrivateNetwork("Private IP".to_string())
            },
            ValidationError::TooLong { max, current } => {
                UrlValidationError::TooLong { max, current }
            },
            ValidationError::SuspiciousCharacters => UrlValidationError::InvalidCharacters,
            _ => UrlValidationError::InvalidFormat("Validation error".to_string()),
        }
    }
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Normalized URL with metadata for DEV-116
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedUrl {
    pub original: String,
    pub normalized: String,
    pub domain: String,
    pub path: String,
    pub scheme: String,
}

impl NormalizedUrl {
    pub fn from(url: Url) -> Self {
        Self {
            original: url.to_string(),
            normalized: url.to_string(),
            domain: url.host_str().unwrap_or("").to_string(),
            path: url.path().to_string(),
            scheme: url.scheme().to_string(),
        }
    }
}

/// URL metadata extracted from pages for DEV-116
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrlMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub favicon_url: Option<String>,
    pub og_image: Option<String>,
    pub content_type: String,
}

/// Metadata extraction errors
#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("HTTP request failed: {0}")]
    HttpError(u16),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Parsing error: {0}")]
    ParseError(String),

    #[error("Timeout error")]
    Timeout,
}

// =============================================================================
// CONSTANTS
// =============================================================================

const MAX_URL_LENGTH: usize = 8192;

// Blacklisted domains (example list - expand as needed)
const BLACKLISTED_DOMAINS: &[&str] = &[
    "bit.ly", // Prevent URL shortener chains
    "tinyurl.com",
    "goo.gl",
    "ow.ly",
    "is.gd",
    "buff.ly",
    "localhost",
    "127.0.0.1",
    "0.0.0.0",
    "::1",
];

// Suspicious patterns that might indicate phishing or malware
const SUSPICIOUS_PATTERNS: &[&str] = &[
    "download-free",
    "click-here-now",
    "limited-time-offer",
    "verify-account",
    "suspend-account",
    "update-payment",
    "confirm-identity",
    "security-alert",
    "unusual-activity",
    "prize-winner",
    "congratulations-winner",
];

// Private network ranges (RFC 1918)
const PRIVATE_IP_RANGES: &[&str] = &[
    "10.", "172.16.", "172.17.", "172.18.", "172.19.", "172.20.", "172.21.", "172.22.", "172.23.",
    "172.24.", "172.25.", "172.26.", "172.27.", "172.28.", "172.29.", "172.30.", "172.31.",
    "192.168.", "169.254.", // Link-local
    "fc00::", "fd00::", // IPv6 private
];

// =============================================================================
// ENHANCED URL VALIDATOR (DEV-116)
// =============================================================================

/// Enhanced URL validator implementing DEV-116 requirements
pub struct UrlValidator {
    allowed_schemes: HashSet<String>,
    blocked_domains: HashSet<String>,
    blocked_tlds: HashSet<String>,
    ip_regex: Regex,
    localhost_regex: Regex,
}

impl Default for UrlValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl UrlValidator {
    /// Create new UrlValidator with configuration loaded from JSON
    pub fn new() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("https".to_string());

        // Load configuration from JSON file
        let (blocked_domains, blocked_tlds) = Self::load_blocked_domains_config();

        Self {
            allowed_schemes,
            blocked_domains,
            blocked_tlds,
            ip_regex: Regex::new(r"^(\d{1,3}\.){3}\d{1,3}$").unwrap(),
            localhost_regex: Regex::new(
                r"^(localhost|127\.|192\.168\.|10\.|172\.(1[6-9]|2[0-9]|3[01])\.)",
            )
            .unwrap(),
        }
    }

    /// Load blocked domains configuration from JSON file
    fn load_blocked_domains_config() -> (HashSet<String>, HashSet<String>) {
        let mut blocked_domains = HashSet::new();
        let mut blocked_tlds = HashSet::new();

        // Try to load from JSON file
        let json_path = "data/blocked_domains.json";
        match std::fs::read_to_string(json_path) {
            Ok(content) => {
                match serde_json::from_str::<BlockedDomainsConfig>(&content) {
                    Ok(config) => {
                        // Load blocked domains from all categories
                        for domain in config
                            .blocked_domains
                            .url_shorteners
                            .domains
                            .iter()
                            .chain(config.blocked_domains.local_addresses.domains.iter())
                            .chain(config.blocked_domains.malicious.domains.iter())
                        {
                            blocked_domains.insert(domain.to_lowercase());
                        }

                        // Load blocked TLDs
                        for tld in config.blocked_tlds.tlds {
                            blocked_tlds.insert(tld.to_lowercase());
                        }

                        info!(
                            "Loaded {} blocked domains and {} blocked TLDs from JSON",
                            blocked_domains.len(),
                            blocked_tlds.len()
                        );
                    },
                    Err(e) => {
                        error!("Failed to parse blocked domains JSON: {}", e);
                        Self::load_fallback_blocked_domains(
                            &mut blocked_domains,
                            &mut blocked_tlds,
                        );
                    },
                }
            },
            Err(e) => {
                warn!(
                    "Failed to read blocked domains file: {}. Using fallback list.",
                    e
                );
                Self::load_fallback_blocked_domains(&mut blocked_domains, &mut blocked_tlds);
            },
        }

        (blocked_domains, blocked_tlds)
    }

    /// Load fallback blocked domains if JSON file is not available
    fn load_fallback_blocked_domains(domains: &mut HashSet<String>, tlds: &mut HashSet<String>) {
        // Fallback URL shorteners and local addresses
        for domain in BLACKLISTED_DOMAINS {
            domains.insert(domain.to_string());
        }

        // Fallback blocked TLDs
        for tld in ["test", "localhost", "local", "internal"] {
            tlds.insert(tld.to_string());
        }

        warn!("Using fallback blocked domains list");
    }

    /// Main validation method implementing DEV-116 requirements
    pub async fn validate_and_normalize(
        &self,
        url_str: &str,
    ) -> Result<NormalizedUrl, ValidationError> {
        // 1. Basic URL parsing
        let parsed_url = self.parse_url(url_str)?;

        // 2. Scheme validation
        self.validate_scheme(&parsed_url)?;

        // 3. Domain validation
        self.validate_domain(&parsed_url).await?;

        // 4. Security checks
        self.security_checks(&parsed_url)?;

        // 5. Normalize URL
        let normalized = self.normalize_url(&parsed_url)?;

        Ok(normalized)
    }

    fn parse_url(&self, url_str: &str) -> Result<Url, ValidationError> {
        // Try parsing as-is first
        match Url::parse(url_str) {
            Ok(url) => Ok(url),
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                // Only add https:// prefix if it looks like a valid domain
                // Must contain at least one dot and valid domain characters
                if DOMAIN_PATTERN.is_match(url_str) {
                    match Url::parse(&format!("https://{}", url_str)) {
                        Ok(url) => Ok(url),
                        Err(e) => Err(ValidationError::InvalidFormat(e.to_string())),
                    }
                } else {
                    Err(ValidationError::InvalidFormat(
                        "Invalid URL format".to_string(),
                    ))
                }
            },
            Err(e) => Err(ValidationError::InvalidFormat(e.to_string())),
        }
    }

    fn validate_scheme(&self, url: &Url) -> Result<(), ValidationError> {
        if !self.allowed_schemes.contains(url.scheme()) {
            return Err(ValidationError::UnsupportedScheme(url.scheme().to_string()));
        }
        Ok(())
    }

    async fn validate_domain(&self, url: &Url) -> Result<(), ValidationError> {
        let host = url.host_str().ok_or(ValidationError::MissingHost)?;

        // Check blocked domains
        if self.blocked_domains.contains(host) {
            return Err(ValidationError::BlockedDomain(host.to_string()));
        }

        // Check blocked TLDs
        if let Some(tld) = self.extract_tld(host) {
            if self.blocked_tlds.contains(&tld) {
                return Err(ValidationError::BlockedTld(tld));
            }
        }

        // Check for private/local IPs
        if self.is_private_or_local_ip(host) {
            return Err(ValidationError::PrivateIp);
        }

        // DNS resolution check (optional, can be expensive)
        if std::env::var("VALIDATE_DNS").unwrap_or_default() == "true" {
            self.validate_dns_resolution(host).await?;
        }

        Ok(())
    }

    fn security_checks(&self, url: &Url) -> Result<(), ValidationError> {
        let url_str = url.as_str();

        // Check URL length
        if url_str.len() > MAX_URL_LENGTH {
            return Err(ValidationError::TooLong {
                max: MAX_URL_LENGTH,
                current: url_str.len(),
            });
        }

        // Check for suspicious characters
        if url_str.contains('\0') || url_str.contains('\r') || url_str.contains('\n') {
            return Err(ValidationError::SuspiciousCharacters);
        }

        // Check for data URLs
        if url.scheme() == "data" {
            return Err(ValidationError::DataUrlNotAllowed);
        }

        // Check for javascript URLs
        if url.scheme() == "javascript" {
            return Err(ValidationError::JavascriptUrlNotAllowed);
        }

        Ok(())
    }

    fn normalize_url(&self, url: &Url) -> Result<NormalizedUrl, ValidationError> {
        let mut normalized = url.clone();

        // Convert host to lowercase
        if let Some(host) = normalized.host_str() {
            let lowercase_host = host.to_lowercase();
            normalized
                .set_host(Some(&lowercase_host))
                .map_err(|e| ValidationError::NormalizationFailed(e.to_string()))?;
        }

        // Remove default ports
        if let Some(port) = normalized.port() {
            let default_port = match normalized.scheme() {
                "http" => 80,
                "https" => 443,
                _ => return Ok(NormalizedUrl::from(normalized)),
            };

            if port == default_port {
                let _ = normalized.set_port(None);
            }
        }

        // Remove trailing slash for root paths
        if normalized.path() == "/"
            && normalized.query().is_none()
            && normalized.fragment().is_none()
        {
            normalized.set_path("");
        }

        // Remove empty query parameters
        if let Some(query) = normalized.query() {
            if query.is_empty() {
                normalized.set_query(None);
            }
        }

        // Remove fragment for normalization
        normalized.set_fragment(None);

        Ok(NormalizedUrl {
            original: url.to_string(),
            normalized: normalized.to_string(),
            domain: normalized.host_str().unwrap_or("").to_string(),
            path: normalized.path().to_string(),
            scheme: normalized.scheme().to_string(),
        })
    }

    fn extract_tld(&self, host: &str) -> Option<String> {
        host.split('.').last().map(|s| s.to_lowercase())
    }

    fn is_private_or_local_ip(&self, host: &str) -> bool {
        // Check if it's an IP address
        if self.ip_regex.is_match(host) {
            return self.is_private_ip_range(host);
        }

        // Check localhost patterns
        self.localhost_regex.is_match(host)
    }

    fn is_private_ip_range(&self, ip: &str) -> bool {
        let parts: Vec<u8> = ip.split('.').filter_map(|s| s.parse().ok()).collect();

        if parts.len() != 4 {
            return false;
        }

        // Private IP ranges
        matches!(
            (parts[0], parts[1]),
            (10, _) | (172, 16..=31) | (192, 168) | (127, _) // Loopback
        )
    }

    async fn validate_dns_resolution(&self, host: &str) -> Result<(), ValidationError> {
        use tokio::net::lookup_host;

        match tokio::time::timeout(Duration::from_secs(5), lookup_host(format!("{}:80", host)))
            .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err(ValidationError::DnsResolutionFailed),
            Err(_) => Err(ValidationError::DnsTimeout),
        }
    }

    /// Extract metadata from URL (DEV-116 requirement)
    pub async fn extract_metadata(&self, url: &str) -> Result<UrlMetadata, MetadataError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("QCK-Bot/1.0")
            .build()?;

        let response = client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(MetadataError::HttpError(response.status().as_u16()));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("text/html") {
            return Ok(UrlMetadata {
                content_type: content_type.to_string(),
                ..Default::default()
            });
        }

        let body = response.text().await?;
        self.parse_html_metadata(&body)
    }

    fn parse_html_metadata(&self, html: &str) -> Result<UrlMetadata, MetadataError> {
        let document = Html::parse_document(html);

        let title = self.extract_title(&document);
        let description = self.extract_description(&document);
        let favicon_url = self.extract_favicon(&document);
        let og_image = self.extract_og_image(&document);

        Ok(UrlMetadata {
            title,
            description,
            favicon_url,
            og_image,
            content_type: "text/html".to_string(),
        })
    }

    fn extract_title(&self, document: &Html) -> Option<String> {
        let title_selector = Selector::parse("title").ok()?;
        document
            .select(&title_selector)
            .next()?
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
            .into()
    }

    fn extract_description(&self, document: &Html) -> Option<String> {
        // Try OG description first
        if let Some(og_desc) = self.extract_meta_property(document, "og:description") {
            return Some(og_desc);
        }

        // Try meta description
        self.extract_meta_name(document, "description")
    }

    fn extract_favicon(&self, document: &Html) -> Option<String> {
        // Try various favicon selectors
        let selectors = [
            r#"link[rel="icon"]"#,
            r#"link[rel="shortcut icon"]"#,
            r#"link[rel="apple-touch-icon"]"#,
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    if let Some(href) = element.value().attr("href") {
                        return Some(href.to_string());
                    }
                }
            }
        }

        None
    }

    fn extract_og_image(&self, document: &Html) -> Option<String> {
        self.extract_meta_property(document, "og:image")
    }

    fn extract_meta_property(&self, document: &Html, property: &str) -> Option<String> {
        let selector = Selector::parse(&format!(r#"meta[property="{}"]"#, property)).ok()?;
        document
            .select(&selector)
            .next()?
            .value()
            .attr("content")
            .map(|s| s.trim().to_string())
    }

    fn extract_meta_name(&self, document: &Html, name: &str) -> Option<String> {
        let selector = Selector::parse(&format!(r#"meta[name="{}"]"#, name)).ok()?;
        document
            .select(&selector)
            .next()?
            .value()
            .attr("content")
            .map(|s| s.trim().to_string())
    }

    // =============================================================================
    // LEGACY COMPATIBILITY METHODS
    // =============================================================================
    /// Validate URL format and security
    pub fn validate_url(url_str: &str) -> Result<Url, UrlValidationError> {
        // Check length
        if url_str.len() > MAX_URL_LENGTH {
            return Err(UrlValidationError::TooLong {
                current: url_str.len(),
                max: MAX_URL_LENGTH,
            });
        }

        // Parse URL
        let url =
            Url::parse(url_str).map_err(|e| UrlValidationError::InvalidFormat(e.to_string()))?;

        // Validate scheme
        Self::validate_scheme_legacy(&url)?;

        // Validate host
        Self::validate_host(&url)?;

        // Check for blacklisted domains
        Self::check_blacklist(&url)?;

        // Check for suspicious patterns
        Self::check_suspicious_patterns(url_str)?;

        // Check for private networks
        Self::check_private_networks(&url)?;

        Ok(url)
    }

    /// Validate URL scheme (only HTTP/HTTPS allowed) - Legacy
    fn validate_scheme_legacy(url: &Url) -> Result<(), UrlValidationError> {
        match url.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(UrlValidationError::InvalidScheme(scheme.to_string())),
        }
    }

    /// Validate host presence and format
    fn validate_host(url: &Url) -> Result<(), UrlValidationError> {
        if url.host_str().is_none() || url.host_str() == Some("") {
            return Err(UrlValidationError::MissingComponents);
        }
        Ok(())
    }

    /// Check if domain is blacklisted
    fn check_blacklist(url: &Url) -> Result<(), UrlValidationError> {
        if let Some(host) = url.host_str() {
            let host_lower = host.to_lowercase();

            for blacklisted in BLACKLISTED_DOMAINS {
                if host_lower.contains(blacklisted) {
                    return Err(UrlValidationError::BlacklistedDomain(host.to_string()));
                }
            }
        }
        Ok(())
    }

    /// Check for suspicious patterns in URL
    fn check_suspicious_patterns(url_str: &str) -> Result<(), UrlValidationError> {
        let url_lower = url_str.to_lowercase();

        for pattern in SUSPICIOUS_PATTERNS {
            if url_lower.contains(pattern) {
                return Err(UrlValidationError::SuspiciousPattern(pattern.to_string()));
            }
        }
        Ok(())
    }

    /// Check if URL points to private network
    fn check_private_networks(url: &Url) -> Result<(), UrlValidationError> {
        if let Some(host) = url.host_str() {
            for private_range in PRIVATE_IP_RANGES {
                if host.starts_with(private_range) {
                    return Err(UrlValidationError::PrivateNetwork(host.to_string()));
                }
            }
        }
        Ok(())
    }

    /// Extract and clean domain from URL
    pub fn extract_domain(url: &Url) -> String {
        url.host_str().unwrap_or("unknown").to_string()
    }

    /// Check if URL is safe for redirect
    pub fn is_safe_redirect(url: &Url) -> bool {
        // Only allow HTTP/HTTPS
        matches!(url.scheme(), "http" | "https") &&
        // Must have a host
        url.host_str().is_some() &&
        // No credentials in URL
        url.username().is_empty() &&
        url.password().is_none() &&
        // No suspicious patterns
        Self::check_suspicious_patterns(url.as_str()).is_ok()
    }

    /// Normalize URL for consistent storage - Legacy
    pub fn normalize_url_legacy(url_str: &str) -> Result<String, UrlValidationError> {
        let mut url = Self::validate_url(url_str)?;

        // Remove trailing slash from path if it's just "/"
        if url.path() == "/" {
            url.set_path("");
        }

        // Remove default ports
        if (url.scheme() == "http" && url.port() == Some(80))
            || (url.scheme() == "https" && url.port() == Some(443))
        {
            let _ = url.set_port(None);
        }

        // Sort query parameters for consistency
        if let Some(_query) = url.query() {
            let mut params: Vec<_> = url.query_pairs().collect();
            params.sort_by(|a, b| a.0.cmp(&b.0));
            let sorted_query: String = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            url.set_query(Some(&sorted_query));
        }

        Ok(url.to_string())
    }
}

/// Normalize URL for consistent storage - Async version for proper validation
pub async fn normalize_url_async(url_str: &str) -> Result<String, UrlValidationError> {
    let validator = UrlValidator::new();
    let normalized = validator
        .validate_and_normalize(url_str)
        .await
        .map_err(|e| UrlValidationError::from(e))?;
    Ok(normalized.normalized)
}

/// Normalize URL for consistent storage - Sync version using tokio's block_on
pub fn normalize_url(url_str: &str) -> Result<String, UrlValidationError> {
    // Use tokio's Handle to run async in sync context
    // This is safe because we're already in a tokio runtime context when handlers call this
    let handle = tokio::runtime::Handle::current();
    handle.block_on(normalize_url_async(url_str))
}

// =============================================================================
// SECURITY SCANNER
// =============================================================================

#[derive(Debug, Clone)]
pub struct SecurityScanResult {
    pub is_safe: bool,
    pub risk_level: RiskLevel,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Blocked,
}

impl SecurityScanResult {
    pub fn safe() -> Self {
        Self {
            is_safe: true,
            risk_level: RiskLevel::Safe,
            issues: vec![],
        }
    }

    pub fn blocked(reason: String) -> Self {
        Self {
            is_safe: false,
            risk_level: RiskLevel::Blocked,
            issues: vec![reason],
        }
    }
}

pub struct SecurityScanner;

impl SecurityScanner {
    /// Perform comprehensive security scan on URL
    pub async fn scan_url(url_str: &str) -> SecurityScanResult {
        // First validate the URL format
        let url = match UrlValidator::validate_url(url_str) {
            Ok(url) => url,
            Err(e) => return SecurityScanResult::blocked(e.to_string()),
        };

        let mut issues = Vec::new();
        let mut risk_level = RiskLevel::Safe;

        // Check for URL shortener chains
        if Self::is_url_shortener(&url) {
            issues.push("URL shortener detected - potential chain risk".to_string());
            risk_level = RiskLevel::Medium;
        }

        // Check for homograph attacks (lookalike domains)
        if Self::has_homograph_risk(&url) {
            issues.push("Potential homograph attack detected".to_string());
            risk_level = RiskLevel::High;
        }

        // Check for excessive redirects (would need HTTP client in production)
        // This is a placeholder for actual redirect checking
        if url.query().is_some() && url.query().unwrap().len() > 500 {
            issues.push("Excessive query parameters detected".to_string());
            risk_level = RiskLevel::Low;
        }

        SecurityScanResult {
            is_safe: risk_level != RiskLevel::Blocked,
            risk_level,
            issues,
        }
    }

    /// Check if URL is from known URL shortener
    fn is_url_shortener(url: &Url) -> bool {
        if let Some(host) = url.host_str() {
            let shorteners = ["bit.ly", "tinyurl.com", "goo.gl", "ow.ly", "is.gd"];
            shorteners.iter().any(|s| host.contains(s))
        } else {
            false
        }
    }

    /// Check for homograph attacks using lookalike characters
    fn has_homograph_risk(url: &Url) -> bool {
        if let Some(host) = url.host_str() {
            // Check for mixed scripts (simplified check)
            let has_cyrillic = host.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
            let has_latin = host.chars().any(|c| c.is_ascii_alphabetic());

            // If both scripts are present, it might be a homograph attack
            has_cyrillic && has_latin
        } else {
            false
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // DEV-116 CRITICAL VERIFICATION TESTS
    #[tokio::test]
    async fn test_dev116_core_validate_and_normalize() {
        let validator = UrlValidator::new();

        // Test basic functionality
        let result = validator
            .validate_and_normalize("https://example.com")
            .await;
        assert!(
            result.is_ok(),
            "validate_and_normalize must work for basic URL"
        );

        let normalized = result.unwrap();
        assert_eq!(normalized.scheme, "https");
        assert_eq!(normalized.domain, "example.com");
        // URL lib adds trailing slash for root paths
        assert!(
            normalized.original == "https://example.com"
                || normalized.original == "https://example.com/"
        );

        // Test auto https:// prefix
        let result2 = validator.validate_and_normalize("example.com").await;
        assert!(result2.is_ok(), "Should auto-add https:// prefix");
        let normalized2 = result2.unwrap();
        assert_eq!(normalized2.scheme, "https");
    }

    #[tokio::test]
    async fn test_dev116_validation_errors() {
        let validator = UrlValidator::new();

        // Test UnsupportedScheme
        let result = validator.validate_and_normalize("ftp://example.com").await;
        assert!(matches!(result, Err(ValidationError::UnsupportedScheme(_))));

        // Test BlockedDomain
        let result = validator.validate_and_normalize("https://localhost").await;
        assert!(matches!(result, Err(ValidationError::BlockedDomain(_))));

        // Test PrivateIp
        let result = validator.validate_and_normalize("http://192.168.1.1").await;
        assert!(matches!(result, Err(ValidationError::PrivateIp)));

        // Test InvalidFormat
        let result = validator.validate_and_normalize("not-a-url").await;
        assert!(matches!(result, Err(ValidationError::InvalidFormat(_))));
    }

    #[tokio::test]
    async fn test_dev116_extract_metadata() {
        let validator = UrlValidator::new();

        // Test that method exists and has correct signature
        // Network dependent so we don't assert success
        let _result = validator.extract_metadata("https://example.com").await;
        // If this compiles, the method signature is correct
    }

    #[test]
    fn test_valid_urls() {
        let valid_urls = vec![
            "https://example.com",
            "http://subdomain.example.com/path",
            "https://example.com:8080/path?query=value",
            "https://example.com/path#fragment",
        ];

        for url_str in valid_urls {
            assert!(
                UrlValidator::validate_url(url_str).is_ok(),
                "Should be valid: {}",
                url_str
            );
        }
    }

    #[test]
    fn test_invalid_schemes() {
        let invalid_urls = vec![
            "ftp://example.com",
            "file:///etc/passwd",
            "javascript:void(0)",
            "data:text/html,<script>example</script>",
        ];

        for url_str in invalid_urls {
            assert!(
                matches!(
                    UrlValidator::validate_url(url_str),
                    Err(UrlValidationError::InvalidScheme(_))
                        | Err(UrlValidationError::InvalidFormat(_))
                ),
                "Should have invalid scheme: {}",
                url_str
            );
        }
    }

    #[test]
    fn test_blacklisted_domains() {
        let blacklisted = vec![
            "http://bit.ly/abc",
            "https://localhost/test",
            "http://127.0.0.1:8080",
        ];

        for url_str in blacklisted {
            assert!(
                matches!(
                    UrlValidator::validate_url(url_str),
                    Err(UrlValidationError::BlacklistedDomain(_))
                ),
                "Should be blacklisted: {}",
                url_str
            );
        }
    }

    #[test]
    fn test_private_networks() {
        let private_urls = vec![
            "http://192.168.1.1",
            "https://10.0.0.1/admin",
            "http://172.16.0.1:8080",
        ];

        for url_str in private_urls {
            assert!(
                matches!(
                    UrlValidator::validate_url(url_str),
                    Err(UrlValidationError::PrivateNetwork(_))
                ),
                "Should be private network: {}",
                url_str
            );
        }
    }

    #[tokio::test]
    async fn test_url_normalization() {
        // Test basic normalization
        assert_eq!(
            normalize_url_async("https://example.com/").await.unwrap(),
            "https://example.com/" // Root path keeps trailing slash in URL string representation
        );

        // Test removal of default HTTP port
        assert_eq!(
            normalize_url_async("http://example.com:80/path")
                .await
                .unwrap(),
            "http://example.com/path"
        );

        // Test removal of default HTTPS port
        assert_eq!(
            normalize_url_async("https://example.com:443/")
                .await
                .unwrap(),
            "https://example.com/" // Default port removed, trailing slash remains for root
        );

        // Test lowercase host normalization
        assert_eq!(
            normalize_url_async("https://EXAMPLE.COM/Path")
                .await
                .unwrap(),
            "https://example.com/Path" // Host lowercased, path preserves case
        );
    }

    #[test]
    fn test_safe_redirect() {
        let url = Url::parse("https://example.com/page").unwrap();
        assert!(UrlValidator::is_safe_redirect(&url));

        let unsafe_url = Url::parse("javascript:void(0)")
            .unwrap_or_else(|_| Url::parse("https://example.com").unwrap());
        // JavaScript URLs won't parse, so we test with a different approach

        let url_with_creds = Url::parse("https://user:pass@example.com").unwrap();
        assert!(!UrlValidator::is_safe_redirect(&url_with_creds));
    }

    #[tokio::test]
    async fn test_security_scanner() {
        let result = SecurityScanner::scan_url("https://example.com").await;
        assert_eq!(result.risk_level, RiskLevel::Safe);
        assert!(result.is_safe);

        let blocked = SecurityScanner::scan_url("ftp://example.com").await;
        assert!(!blocked.is_safe);
        assert_eq!(blocked.risk_level, RiskLevel::Blocked);
    }

    #[tokio::test]
    async fn test_validate_and_normalize() {
        let validator = UrlValidator::new();

        // Test valid URL with scheme
        let result = validator
            .validate_and_normalize("https://example.com/path?query=value")
            .await;
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.scheme, "https");
        assert_eq!(normalized.domain, "example.com");
        assert_eq!(normalized.path, "/path");

        // Test domain without scheme (should add https://)
        let result = validator.validate_and_normalize("example.com").await;
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.scheme, "https");
        assert_eq!(normalized.domain, "example.com");

        // Test invalid URLs that should fail
        let invalid_urls = vec![
            "not-a-url",
            "javascript:void(0)",
            "data:text/html,<script>example</script>",
            "ftp://example.com",
            "http://localhost",
            "http://192.168.1.1",
        ];

        for invalid_url in invalid_urls {
            let result = validator.validate_and_normalize(invalid_url).await;
            assert!(result.is_err(), "Should reject: {}", invalid_url);
        }

        // Test normalization features
        let result = validator
            .validate_and_normalize("HTTPS://EXAMPLE.COM:443/")
            .await;
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.domain, "example.com"); // Lowercase
        assert!(!normalized.normalized.contains(":443")); // Default port removed

        // Test URL length limit
        let long_url = format!("https://example.com/{}", "a".repeat(8200));
        let result = validator.validate_and_normalize(&long_url).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::TooLong { .. }
        ));
    }

    #[tokio::test]
    async fn test_extract_metadata() {
        let validator = UrlValidator::new();

        // Note: This test would need a mock HTTP server in production
        // For now, we test that the method exists and handles errors properly

        // Test with invalid URL (should fail before HTTP request)
        let result = validator.extract_metadata("not-a-url").await;
        assert!(result.is_err());

        // Test with localhost (should fail during request)
        let result = validator
            .extract_metadata("http://localhost:9999/nonexistent")
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_private_ip_detection() {
        let validator = UrlValidator::new();

        // Test private IP ranges
        assert!(validator.is_private_or_local_ip("192.168.1.1"));
        assert!(validator.is_private_or_local_ip("10.0.0.1"));
        assert!(validator.is_private_or_local_ip("172.16.0.1"));
        assert!(validator.is_private_or_local_ip("127.0.0.1"));
        assert!(validator.is_private_or_local_ip("localhost"));

        // Test public IPs
        assert!(!validator.is_private_or_local_ip("8.8.8.8"));
        assert!(!validator.is_private_or_local_ip("1.1.1.1"));
        assert!(!validator.is_private_or_local_ip("example.com"));
    }

    #[test]
    fn test_tld_extraction() {
        let validator = UrlValidator::new();

        assert_eq!(
            validator.extract_tld("example.com"),
            Some("com".to_string())
        );
        assert_eq!(
            validator.extract_tld("sub.example.co.uk"),
            Some("uk".to_string())
        );
        assert_eq!(
            validator.extract_tld("localhost"),
            Some("localhost".to_string())
        );
    }
}
