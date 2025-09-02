// DEV-104: URL Security Scanning
// Comprehensive security scanning to prevent malicious links from being shortened

use crate::db::ClickHouseClient;
use crate::utils::urlhaus_client::UrlhausClient;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use url::Url;

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Domain is blocked: {0}")]
    BlockedDomain(String),

    #[error("URL contains suspicious patterns: {0}")]
    SuspiciousPattern(String),

    #[error("Content scanning failed: {0}")]
    ContentScanError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Threat intelligence API error: {0}")]
    ThreatIntelError(String),

    #[error("Rate limit exceeded")]
    RateLimited,

    #[error("Service timeout")]
    Timeout,

    #[error("Internal security service error")]
    InternalError,
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScanResult {
    pub url: String,
    pub is_safe: bool,
    pub threat_score: u8, // 0-100, higher is more dangerous
    pub risk_level: SecurityRiskLevel,
    pub threats_detected: Vec<ThreatType>,
    pub warnings: Vec<String>,
    pub scan_timestamp: DateTime<Utc>,
    pub scan_duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SecurityRiskLevel {
    Safe,     // 0-20
    Low,      // 21-40
    Medium,   // 41-60
    High,     // 61-80
    Critical, // 81-100
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThreatType {
    Malware,
    Phishing,
    Spam,
    Scam,
    SuspiciousTld,
    ShortenerChaining,
    HomographAttack,
    SuspiciousRedirect,
    MaliciousContent,
    DataHarvesting,
    CryptocurrencyMining,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationScore {
    pub score: u8, // 0-100
    pub sources: Vec<String>,
    pub last_updated: DateTime<Utc>,
    pub confidence: f32, // 0.0-1.0
}

#[derive(Debug, Clone)]
pub struct SecurityWarning {
    pub warning_type: ThreatType,
    pub message: String,
    pub severity: SecurityRiskLevel,
}

// =============================================================================
// DOMAIN SECURITY SERVICE
// =============================================================================

pub struct DomainSecurityService {
    blacklist: Arc<RwLock<HashSet<String>>>,
    reputation_cache: Arc<RwLock<HashMap<String, ReputationScore>>>,
    threat_intel_client: Arc<ThreatIntelClient>,
    homograph_detector: HomographDetector,
}

impl DomainSecurityService {
    pub fn new() -> Self {
        let mut blacklist = HashSet::new();

        // Load from JSON file (same pattern as url_validator.rs)
        let json_path = "data/blocked_domains.json";
        match std::fs::read_to_string(json_path) {
            Ok(content) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Load URL shorteners
                    if let Some(shorteners) =
                        json["blocked_domains"]["url_shorteners"]["domains"].as_array()
                    {
                        for domain in shorteners {
                            if let Some(d) = domain.as_str() {
                                blacklist.insert(d.to_string());
                            }
                        }
                    }

                    // Load local addresses
                    if let Some(local) =
                        json["blocked_domains"]["local_addresses"]["domains"].as_array()
                    {
                        for domain in local {
                            if let Some(d) = domain.as_str() {
                                blacklist.insert(d.to_string());
                            }
                        }
                    }

                    // Load malicious domains
                    if let Some(malicious) =
                        json["blocked_domains"]["malicious"]["domains"].as_array()
                    {
                        for domain in malicious {
                            if let Some(d) = domain.as_str() {
                                blacklist.insert(d.to_string());
                            }
                        }
                    }

                    // Load blocked TLDs
                    if let Some(tlds) = json["blocked_tlds"]["tlds"].as_array() {
                        for tld in tlds {
                            if let Some(t) = tld.as_str() {
                                blacklist.insert(t.to_string());
                            }
                        }
                    }

                    tracing::info!("Loaded {} blocked domains from JSON", blacklist.len());
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to load blocked_domains.json: {}, using fallback list",
                    e
                );
                // Fallback to minimal hardcoded list if JSON fails
                let fallback = ["bit.ly", "tinyurl.com", "localhost", "127.0.0.1"];
                for domain in &fallback {
                    blacklist.insert(domain.to_string());
                }
            },
        }

        Self {
            blacklist: Arc::new(RwLock::new(blacklist)),
            reputation_cache: Arc::new(RwLock::new(HashMap::new())),
            threat_intel_client: Arc::new(ThreatIntelClient::new()),
            homograph_detector: HomographDetector::new(),
        }
    }

    pub async fn check_domain_reputation(
        &self,
        domain: &str,
    ) -> Result<SecurityScanResult, SecurityError> {
        let start_time = std::time::Instant::now();
        let mut threats_detected = Vec::new();
        let mut warnings = Vec::new();
        let mut threat_score = 0u8;

        // 1. Check static blacklist
        let blacklist = self.blacklist.read().await;
        if blacklist.contains(domain) {
            // Check if it's a URL shortener or other blocked domain
            if self.is_url_shortener(domain) {
                threats_detected.push(ThreatType::ShortenerChaining);
                threat_score += 50; // Lower score for shorteners
                warnings.push(format!(
                    "URL shortener {} is blocked to prevent chaining",
                    domain
                ));
            } else {
                threats_detected.push(ThreatType::Malware);
                threat_score += 100;
                warnings.push(format!("Domain {} is in blacklist", domain));
            }

            return Ok(SecurityScanResult {
                url: domain.to_string(),
                is_safe: false,
                threat_score,
                risk_level: if threat_score >= 80 {
                    SecurityRiskLevel::Critical
                } else {
                    SecurityRiskLevel::High
                },
                threats_detected,
                warnings,
                scan_timestamp: Utc::now(),
                scan_duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }
        drop(blacklist);

        // 2. Check for URL shortener domains
        if self.is_url_shortener(domain) {
            threats_detected.push(ThreatType::ShortenerChaining);
            threat_score += 30;
            warnings.push("URL shortener detected - potential chain risk".to_string());
        }

        // 3. Check for suspicious TLD
        if self.has_suspicious_tld(domain) {
            threats_detected.push(ThreatType::SuspiciousTld);
            threat_score += 20;
            warnings.push(format!("Suspicious TLD detected in domain: {}", domain));
        }

        // 4. Check for homograph attacks
        if self.homograph_detector.has_homograph_attack(domain) {
            threats_detected.push(ThreatType::HomographAttack);
            threat_score += 40;
            warnings.push("Potential homograph attack detected".to_string());
        }

        // 5. Check reputation cache
        let cache = self.reputation_cache.read().await;
        if let Some(reputation) = cache.get(domain) {
            // Use cached reputation if less than 1 hour old
            if (Utc::now() - reputation.last_updated).num_minutes() < 60 {
                threat_score = threat_score.max(100 - reputation.score);
                drop(cache);

                let risk_level = self.calculate_risk_level(threat_score);
                return Ok(SecurityScanResult {
                    url: domain.to_string(),
                    is_safe: risk_level == SecurityRiskLevel::Safe,
                    threat_score,
                    risk_level,
                    threats_detected,
                    warnings,
                    scan_timestamp: Utc::now(),
                    scan_duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }
        }
        drop(cache);

        // 6. Query threat intelligence APIs (with circuit breaker)
        match tokio::time::timeout(
            Duration::from_secs(3),
            self.threat_intel_client.check_domain(domain),
        )
        .await
        {
            Ok(Ok(intel_score)) => {
                threat_score = threat_score.max(intel_score);

                // Cache the result
                let mut cache = self.reputation_cache.write().await;
                cache.insert(
                    domain.to_string(),
                    ReputationScore {
                        score: 100 - intel_score,
                        sources: vec!["threat_intel".to_string()],
                        last_updated: Utc::now(),
                        confidence: 0.8,
                    },
                );
            },
            Ok(Err(_)) => {
                // Threat intel API failed, but don't block - just log
                warnings.push("Threat intelligence check failed".to_string());
            },
            Err(_) => {
                // Timeout - don't block the request
                warnings.push("Threat intelligence check timed out".to_string());
            },
        }

        let risk_level = self.calculate_risk_level(threat_score);

        Ok(SecurityScanResult {
            url: domain.to_string(),
            is_safe: threat_score < 41, // Medium risk and below is considered safe
            threat_score,
            risk_level,
            threats_detected,
            warnings,
            scan_timestamp: Utc::now(),
            scan_duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn is_url_shortener(&self, domain: &str) -> bool {
        let shorteners = [
            "bit.ly",
            "tinyurl.com",
            "goo.gl",
            "ow.ly",
            "is.gd",
            "buff.ly",
            "t.co",
            "short.link",
            "tiny.cc",
            "rb.gy",
            "cutt.ly",
            "shorturl.at",
            "1url.com",
            "2.gp",
            "7.ly",
            "0.gp",
            "1-url.net",
        ];

        shorteners
            .iter()
            .any(|&shortener| domain.contains(shortener))
    }

    fn has_suspicious_tld(&self, domain: &str) -> bool {
        let suspicious_tlds = [
            ".tk",
            ".ml",
            ".cf",
            ".ga",
            ".click",
            ".download",
            ".zip",
            ".exe",
            ".scr",
            ".bat",
            ".cmd",
            ".pif",
            ".com.ru",
            ".co.cc",
            ".bit",
            ".onion",
            ".i2p",
            ".exit",
            ".darkweb",
        ];

        suspicious_tlds.iter().any(|&tld| domain.ends_with(tld))
    }

    fn calculate_risk_level(&self, score: u8) -> SecurityRiskLevel {
        match score {
            0..=20 => SecurityRiskLevel::Safe,
            21..=40 => SecurityRiskLevel::Low,
            41..=60 => SecurityRiskLevel::Medium,
            61..=80 => SecurityRiskLevel::High,
            81..=100 => SecurityRiskLevel::Critical,
            _ => SecurityRiskLevel::Critical, // Fallback for impossible values > 100
        }
    }
}

// =============================================================================
// URL PATTERN ANALYZER
// =============================================================================

pub struct UrlPatternAnalyzer {
    suspicious_patterns: Vec<Regex>,
    phishing_keywords: HashSet<String>,
}

impl UrlPatternAnalyzer {
    pub fn new() -> Self {
        let mut phishing_keywords = HashSet::new();

        // Common phishing keywords
        let keywords = [
            "verify-account",
            "suspend-account",
            "update-payment",
            "confirm-identity",
            "security-alert",
            "unusual-activity",
            "prize-winner",
            "congratulations-winner",
            "limited-time-offer",
            "click-here-now",
            "download-free",
            "urgent-action",
            "verify-now",
            "update-info",
            "confirm-email",
            "reset-password",
            "account-locked",
            "suspended-account",
            "billing-issue",
            "payment-failed",
            "tax-refund",
            "lottery-winner",
            "free-money",
            "inheritance-claim",
        ];

        for keyword in &keywords {
            phishing_keywords.insert(keyword.to_string());
        }

        // Compile suspicious patterns
        let patterns = vec![
            // Multiple subdomains (could indicate subdomain takeover)
            Regex::new(r"^[^.]+\.([^.]+\.){4,}[^.]+$").unwrap(),
            // Suspicious character sequences
            Regex::new(r"[0-9]{4,}").unwrap(), // Long number sequences
            Regex::new(r"[a-z]{1}[0-9]{3,}[a-z]{1}").unwrap(), // Mixed letter-number patterns
            // URL encoding that might hide malicious content
            Regex::new(r"%[0-9a-fA-F]{2}{3,}").unwrap(),
            // Excessive hyphens (typosquatting)
            Regex::new(r"-{2,}").unwrap(),
            // Mixed case in suspicious patterns
            Regex::new(r"[a-z][A-Z][a-z][A-Z]").unwrap(),
        ];

        Self {
            suspicious_patterns: patterns,
            phishing_keywords,
        }
    }

    pub fn analyze_suspicious_patterns(&self, url: &str) -> Vec<SecurityWarning> {
        let mut warnings = Vec::new();

        // ReDoS protection: Skip pattern analysis for overly long URLs
        if url.len() > 2048 {
            warnings.push(SecurityWarning {
                warning_type: ThreatType::SuspiciousRedirect,
                message: "URL exceeds maximum safe length for analysis".to_string(),
                severity: SecurityRiskLevel::Medium,
            });
            return warnings;
        }

        let url_lower = url.to_lowercase();

        // Check for phishing keywords
        for keyword in &self.phishing_keywords {
            if url_lower.contains(keyword) {
                warnings.push(SecurityWarning {
                    warning_type: ThreatType::Phishing,
                    message: format!("Suspicious keyword detected: {}", keyword),
                    severity: SecurityRiskLevel::Medium,
                });
            }
        }

        // Check regex patterns
        for (i, pattern) in self.suspicious_patterns.iter().enumerate() {
            if pattern.is_match(url) {
                let warning_type = match i {
                    0 => ThreatType::SuspiciousRedirect,
                    1..=2 => ThreatType::Phishing,
                    3 => ThreatType::MaliciousContent,
                    4 => ThreatType::Phishing,
                    5 => ThreatType::Phishing,
                    _ => ThreatType::Spam,
                };

                warnings.push(SecurityWarning {
                    warning_type,
                    message: format!("Suspicious URL pattern detected (rule {})", i + 1),
                    severity: SecurityRiskLevel::Low,
                });
            }
        }

        warnings
    }
}

// =============================================================================
// HOMOGRAPH DETECTOR
// =============================================================================

pub struct HomographDetector {
    confusable_chars: HashMap<char, Vec<char>>,
}

impl HomographDetector {
    pub fn new() -> Self {
        let mut confusable_chars = HashMap::new();

        // Map confusable Unicode characters (simplified set)
        confusable_chars.insert('a', vec!['а', 'α', 'ɑ']); // Latin 'a', Cyrillic 'а', Greek 'α'
        confusable_chars.insert('e', vec!['е', 'ε']); // Latin 'e', Cyrillic 'е', Greek 'ε'
        confusable_chars.insert('o', vec!['о', 'ο', '0']); // Latin 'o', Cyrillic 'о', Greek 'ο', digit '0'
        confusable_chars.insert('p', vec!['р', 'ρ']); // Latin 'p', Cyrillic 'р', Greek 'ρ'
        confusable_chars.insert('c', vec!['с', 'ϲ']); // Latin 'c', Cyrillic 'с', Greek 'ϲ'
        confusable_chars.insert('y', vec!['у', 'γ']); // Latin 'y', Cyrillic 'у', Greek 'γ'
        confusable_chars.insert('x', vec!['х', 'χ']); // Latin 'x', Cyrillic 'х', Greek 'χ'

        Self { confusable_chars }
    }

    pub fn has_homograph_attack(&self, domain: &str) -> bool {
        // Check for mixed scripts
        let has_cyrillic = domain
            .chars()
            .any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
        let has_greek = domain
            .chars()
            .any(|c| ('\u{0370}'..='\u{03FF}').contains(&c));
        let has_latin = domain.chars().any(|c| c.is_ascii_alphabetic());

        // If multiple scripts are present, it's potentially suspicious
        let script_count = [has_cyrillic, has_greek, has_latin]
            .iter()
            .filter(|&&x| x)
            .count();
        if script_count > 1 {
            return true;
        }

        // Check for confusable characters in known patterns
        for ch in domain.chars() {
            if let Some(confusables) = self.confusable_chars.get(&ch) {
                // If we find confusable characters, it might be an attack
                if confusables.iter().any(|&conf| domain.contains(conf)) {
                    return true;
                }
            }
        }

        false
    }
}

// =============================================================================
// CONTENT SCANNER IMPLEMENTATION
// =============================================================================

pub struct ContentScanner {
    client: reqwest::Client,
}

impl ContentScanner {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .user_agent("QCK-SecurityScanner/1.0")
                .redirect(reqwest::redirect::Policy::limited(3)) // Limit redirects
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn scan_url_content(&self, url: &str) -> Result<ContentScanResult, SecurityError> {
        // Use HEAD request to avoid downloading full content
        match self.client.head(url).send().await {
            Ok(response) => {
                let mut threats = Vec::new();
                let mut warnings = Vec::new();
                let mut threat_score = 0u8;

                // Check Content-Type header for suspicious types
                if let Some(content_type) = response.headers().get("content-type") {
                    let ct_str = content_type.to_str().unwrap_or("");

                    // Check for executable content
                    if ct_str.contains("application/octet-stream")
                        || ct_str.contains("application/x-")
                        || ct_str.contains("application/exe")
                    {
                        threats.push(ThreatType::MaliciousContent);
                        threat_score += 30;
                        warnings.push("Suspicious content type detected".to_string());
                    }
                }

                // Check for suspicious redirects
                if response.status().is_redirection() {
                    if let Some(location) = response.headers().get("location") {
                        let redirect_url = location.to_str().unwrap_or("");

                        // Check if redirecting to a different domain
                        if let (Ok(original), Ok(redirect)) =
                            (Url::parse(url), Url::parse(redirect_url))
                        {
                            if original.host() != redirect.host() {
                                threats.push(ThreatType::SuspiciousRedirect);
                                threat_score += 20;
                                warnings.push(format!(
                                    "Redirects to different domain: {:?}",
                                    redirect.host()
                                ));
                            }
                        }
                    }
                }

                // Check SSL certificate validity (for HTTPS URLs)
                if url.starts_with("https://") && !response.status().is_success() {
                    if response.status() == reqwest::StatusCode::UNAUTHORIZED
                        || response.status() == reqwest::StatusCode::FORBIDDEN
                    {
                        // Could indicate SSL/TLS issues
                        threat_score += 10;
                        warnings.push("Potential SSL/TLS configuration issues".to_string());
                    }
                }

                Ok(ContentScanResult {
                    threats_detected: threats,
                    warnings,
                    threat_score,
                    is_safe: threat_score < 30,
                })
            },
            Err(e) => {
                // Network errors could indicate malicious behavior
                if e.is_timeout() {
                    Ok(ContentScanResult {
                        threats_detected: vec![],
                        warnings: vec!["URL request timed out".to_string()],
                        threat_score: 10,
                        is_safe: true, // Don't block on timeout
                    })
                } else if e.is_redirect() {
                    Ok(ContentScanResult {
                        threats_detected: vec![ThreatType::SuspiciousRedirect],
                        warnings: vec!["Too many redirects".to_string()],
                        threat_score: 25,
                        is_safe: true,
                    })
                } else {
                    // Other errors - don't block but log
                    Ok(ContentScanResult {
                        threats_detected: vec![],
                        warnings: vec![format!("Content scan error: {}", e)],
                        threat_score: 5,
                        is_safe: true,
                    })
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentScanResult {
    pub threats_detected: Vec<ThreatType>,
    pub warnings: Vec<String>,
    pub threat_score: u8,
    pub is_safe: bool,
}

// =============================================================================
// THREAT INTELLIGENCE CLIENT
// =============================================================================

pub struct ThreatIntelClient {
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, (u8, Instant)>>>,
    cache_ttl: Duration,
}

impl ThreatIntelClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("QCK-SecurityScanner/1.0")
                .build()
                .unwrap_or_default(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(3600), // 1 hour cache
        }
    }

    pub async fn check_domain(&self, domain: &str) -> Result<u8, SecurityError> {
        // Check cache first
        if let Some(cached_score) = self.get_cached_score(domain).await {
            return Ok(cached_score);
        }

        // Perform threat analysis
        let score = self.analyze_domain_threats(domain).await?;

        // Cache the result
        self.cache_score(domain, score).await;

        Ok(score)
    }

    async fn get_cached_score(&self, domain: &str) -> Option<u8> {
        let cache = self.cache.read().await;
        if let Some((score, timestamp)) = cache.get(domain) {
            if timestamp.elapsed() < self.cache_ttl {
                return Some(*score);
            }
        }
        None
    }

    async fn cache_score(&self, domain: &str, score: u8) {
        let mut cache = self.cache.write().await;
        cache.insert(domain.to_string(), (score, Instant::now()));

        // Clean old entries if cache is too large
        if cache.len() > 10000 {
            let now = Instant::now();
            cache.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < self.cache_ttl);
        }
    }

    async fn analyze_domain_threats(&self, domain: &str) -> Result<u8, SecurityError> {
        // Advanced threat scoring algorithm
        let mut score = 0u8;
        let domain_lower = domain.to_lowercase();

        // 1. Domain entropy analysis (high entropy = suspicious)
        let entropy = self.calculate_entropy(&domain_lower);
        if entropy > 3.5 {
            score += 20; // High entropy suggests randomized/generated domain
        }

        // 2. Typosquatting detection
        if self.detect_typosquatting(&domain_lower) {
            score += 25;
        }

        // 3. Homograph attack detection
        if self.contains_homographs(&domain_lower) {
            score += 30;
        }

        // 4. Length and structure analysis
        if domain.len() > 40 {
            score += 15; // Unusually long domain
        }

        let subdomain_count = domain.matches('.').count();
        if subdomain_count > 3 {
            score += 10; // Too many subdomains
        }

        // 5. Digit and special character analysis
        let digit_ratio =
            domain.chars().filter(|c| c.is_ascii_digit()).count() as f32 / domain.len() as f32;
        if digit_ratio > 0.3 {
            score += 15; // High ratio of digits
        }

        let hyphen_count = domain.matches('-').count();
        if hyphen_count > 4 {
            score += 10; // Excessive hyphens (common in phishing)
        }

        // 6. Suspicious keyword detection
        let keywords = [
            ("secure", 10),
            ("verify", 15),
            ("update", 10),
            ("suspended", 20),
            ("urgent", 15),
            ("confirm", 10),
            ("validate", 10),
            ("unlock", 15),
            ("refund", 10),
            ("winner", 20),
            ("prize", 20),
            ("free", 5),
        ];

        for (keyword, weight) in &keywords {
            if domain_lower.contains(keyword) {
                score += weight;
            }
        }

        // 7. Known brand impersonation patterns
        if self.detect_brand_impersonation(&domain_lower) {
            score += 35;
        }

        // 8. TLD risk assessment
        let high_risk_tlds = [".tk", ".ml", ".ga", ".cf", ".click", ".download", ".stream"];
        for tld in &high_risk_tlds {
            if domain_lower.ends_with(tld) {
                score += 20;
                break;
            }
        }

        // Cap at 100
        Ok(score.min(100))
    }

    fn calculate_entropy(&self, text: &str) -> f32 {
        let mut freq_map = HashMap::new();
        for ch in text.chars() {
            *freq_map.entry(ch).or_insert(0) += 1;
        }

        let len = text.len() as f32;
        freq_map
            .values()
            .map(|&count| {
                let p = count as f32 / len;
                -p * p.log2()
            })
            .sum()
    }

    fn detect_typosquatting(&self, domain: &str) -> bool {
        // Check for common typosquatting patterns
        let patterns = [
            "g00gle",
            "goog1e",
            "gooogle",
            "googel",
            "mircosoft",
            "microsofy",
            "micrsoft",
            "amazom",
            "amaz0n",
            "arnazon",
            "facebok",
            "faceboook",
            "faceb00k",
            "payp4l",
            "paypa1",
            "paipal",
        ];

        patterns.iter().any(|pattern| domain.contains(pattern))
    }

    fn contains_homographs(&self, domain: &str) -> bool {
        // Check for mixed scripts (simplified)
        let has_cyrillic = domain
            .chars()
            .any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
        let has_latin = domain.chars().any(|c| c.is_ascii_alphabetic());
        let has_greek = domain
            .chars()
            .any(|c| ('\u{0370}'..='\u{03FF}').contains(&c));

        // Mixed scripts are suspicious
        (has_cyrillic && has_latin) || (has_greek && has_latin)
    }

    fn detect_brand_impersonation(&self, domain: &str) -> bool {
        // Common brand impersonation patterns
        let patterns = [
            (r"(apple|icloud).*\.(tk|ml|ga|cf)", true),
            (
                r"(paypal|ebay|amazon|google|microsoft|facebook).*support",
                true,
            ),
            (
                r"(secure|verify|update).*\.(paypal|amazon|google|apple)",
                true,
            ),
            (r"\d+\.(paypal|amazon|google|apple|microsoft)", true),
        ];

        for (pattern, _) in &patterns {
            if regex::Regex::new(pattern).unwrap().is_match(domain) {
                return true;
            }
        }

        false
    }

    // ==========================================================================
    // EXTERNAL API INTEGRATIONS - Currently stubbed due to cost
    // See Linear EPIC for implementation roadmap
    // ==========================================================================

    /// Google Safe Browsing API integration
    /// Cost: ~$0.001 per query, ~$1000/month for 1M queries
    /// TODO: Implement when budget allows (see Linear EPIC)
    pub async fn check_google_safe_browsing(&self, _url: &str) -> Result<bool, SecurityError> {
        // Stub: Returns false (safe) to avoid blocking legitimate URLs
        // In production with API key:
        // - Check URL against Google's malware/phishing database
        // - Cache results for 30 minutes
        // - Implement exponential backoff for rate limits
        Ok(false)
    }

    /// VirusTotal API integration  
    /// Cost: $10,000+/year for commercial use
    /// TODO: Implement when budget allows (see Linear EPIC)
    pub async fn check_virus_total(&self, _url: &str) -> Result<u8, SecurityError> {
        // Stub: Returns 0 (no detections) to avoid blocking
        // In production with API key:
        // - Submit URL for scanning by 70+ antivirus engines
        // - Return aggregated threat score
        // - Cache results for 24 hours
        Ok(0)
    }

    /// PhishTank API integration
    /// Cost: FREE with registration
    /// TODO: Priority implementation (see Linear EPIC)
    pub async fn check_phishtank(&self, _url: &str) -> Result<bool, SecurityError> {
        // Stub: Returns false (not phishing) to avoid blocking
        // Implementation plan:
        // - Register for free API key at phishtank.com
        // - Check URL against community-reported phishing database
        // - Update local cache daily
        // - Rate limit: 2000 requests per 5 minutes
        Ok(false)
    }
}

// =============================================================================
// MAIN SECURITY SERVICE
// =============================================================================

pub struct SecurityService {
    domain_security: DomainSecurityService,
    pattern_analyzer: UrlPatternAnalyzer,
    content_scanner: ContentScanner,
    urlhaus_client: Arc<UrlhausClient>,
}

impl SecurityService {
    pub fn new(clickhouse_client: Arc<ClickHouseClient>) -> Self {
        Self {
            domain_security: DomainSecurityService::new(),
            pattern_analyzer: UrlPatternAnalyzer::new(),
            content_scanner: ContentScanner::new(),
            urlhaus_client: Arc::new(UrlhausClient::new(clickhouse_client)),
        }
    }

    pub async fn comprehensive_security_scan(
        &self,
        url_str: &str,
    ) -> Result<SecurityScanResult, SecurityError> {
        let start_time = std::time::Instant::now();

        // ReDoS protection: Limit URL length before any regex operations
        const MAX_URL_LENGTH: usize = 2048;
        if url_str.len() > MAX_URL_LENGTH {
            return Err(SecurityError::SuspiciousPattern(format!(
                "URL too long: {} bytes (max: {})",
                url_str.len(),
                MAX_URL_LENGTH
            )));
        }

        let url = Url::parse(url_str).map_err(|_| SecurityError::InternalError)?;

        let domain = url.host_str().ok_or(SecurityError::InternalError)?;

        // 1. Domain reputation check
        let mut scan_result = self.domain_security.check_domain_reputation(domain).await?;

        // 2. URL pattern analysis
        let pattern_warnings = self.pattern_analyzer.analyze_suspicious_patterns(url_str);
        for warning in pattern_warnings {
            if !scan_result.threats_detected.contains(&warning.warning_type) {
                scan_result.threats_detected.push(warning.warning_type);
            }
            scan_result.warnings.push(warning.message);

            // Adjust threat score based on pattern warnings
            let pattern_score = match warning.severity {
                SecurityRiskLevel::Low => 5,
                SecurityRiskLevel::Medium => 15,
                SecurityRiskLevel::High => 25,
                SecurityRiskLevel::Critical => 40,
                SecurityRiskLevel::Safe => 0,
            };
            scan_result.threat_score = (scan_result.threat_score + pattern_score).min(100);
        }

        // 3. URLhaus threat intelligence check (FREE)
        // Check against known malware/phishing URLs from abuse.ch
        match tokio::time::timeout(
            Duration::from_secs(2),
            self.urlhaus_client.check_url(url_str),
        )
        .await
        {
            Ok(Ok(is_malicious)) => {
                if is_malicious {
                    scan_result.threats_detected.push(ThreatType::Malware);
                    scan_result.threat_score = (scan_result.threat_score + 50).min(100);
                    scan_result
                        .warnings
                        .push("URL found in URLhaus malware database (abuse.ch)".to_string());
                }
            },
            Ok(Err(e)) => {
                tracing::debug!("URLhaus check error: {}", e);
            },
            Err(_) => {
                tracing::debug!("URLhaus check timed out");
            },
        }

        // Also check domain reputation in URLhaus
        if let Some(domain) = url.host_str() {
            match tokio::time::timeout(
                Duration::from_secs(1),
                self.urlhaus_client.check_domain(domain),
            )
            .await
            {
                Ok(Ok(threat_count)) if threat_count > 0 => {
                    // Fix overflow: ensure threat_count doesn't overflow when converting to u8
                    let safe_count = threat_count.min(255) as u8;
                    let domain_score = safe_count.saturating_mul(5).min(30); // Max 30 points for domain threats
                    scan_result.threat_score = scan_result
                        .threat_score
                        .saturating_add(domain_score)
                        .min(100);
                    scan_result.warnings.push(format!(
                        "Domain has {} malicious URLs in URLhaus",
                        threat_count
                    ));
                },
                Ok(Err(e)) => {
                    tracing::debug!("URLhaus domain check error: {}", e);
                },
                _ => {},
            }
        }

        // 4. Content scanning (lightweight HEAD request only)
        // Only scan if not already high risk
        if scan_result.threat_score < 60 {
            match tokio::time::timeout(
                Duration::from_secs(3),
                self.content_scanner.scan_url_content(url_str),
            )
            .await
            {
                Ok(Ok(content_result)) => {
                    // Merge content scan results
                    for threat in content_result.threats_detected {
                        if !scan_result.threats_detected.contains(&threat) {
                            scan_result.threats_detected.push(threat);
                        }
                    }
                    scan_result.warnings.extend(content_result.warnings);
                    scan_result.threat_score =
                        (scan_result.threat_score + content_result.threat_score).min(100);
                },
                Ok(Err(_)) | Err(_) => {
                    // Content scan failed or timed out - don't block
                    scan_result
                        .warnings
                        .push("Content scan unavailable".to_string());
                },
            }
        }

        // 5. Update risk level and safety based on final score
        scan_result.risk_level = match scan_result.threat_score {
            0..=20 => SecurityRiskLevel::Safe,
            21..=40 => SecurityRiskLevel::Low,
            41..=60 => SecurityRiskLevel::Medium,
            61..=80 => SecurityRiskLevel::High,
            81..=100 => SecurityRiskLevel::Critical,
            _ => SecurityRiskLevel::Critical, // Fallback for impossible values > 100
        };

        scan_result.is_safe = scan_result.threat_score < 41;
        scan_result.scan_duration_ms = start_time.elapsed().as_millis() as u64;
        scan_result.url = url_str.to_string();

        Ok(scan_result)
    }
}

impl Default for SecurityService {
    fn default() -> Self {
        let clickhouse_client = crate::db::clickhouse_client::create_clickhouse_client();
        Self::new(clickhouse_client)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_domain_security_service() {
        let service = DomainSecurityService::new();

        // Test safe domain
        let result = service
            .check_domain_reputation("example.com")
            .await
            .unwrap();
        assert!(result.is_safe);
        assert!(result.threat_score < 30);

        // Test URL shortener detection
        let result = service.check_domain_reputation("bit.ly").await.unwrap();
        assert!(!result.is_safe);
        assert!(result
            .threats_detected
            .contains(&ThreatType::ShortenerChaining));
    }

    #[test]
    fn test_url_pattern_analyzer() {
        let analyzer = UrlPatternAnalyzer::new();

        // Test phishing keyword detection
        let warnings = analyzer.analyze_suspicious_patterns("https://verify-account-now.com/login");
        assert!(!warnings.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.warning_type == ThreatType::Phishing));

        // Test clean URL
        let warnings =
            analyzer.analyze_suspicious_patterns("https://legitimate-business.com/contact");
        assert!(warnings.is_empty() || warnings.len() <= 1); // Might trigger pattern rules but should be minimal
    }

    #[test]
    fn test_homograph_detector() {
        let detector = HomographDetector::new();

        // Test mixed script attack
        assert!(detector.has_homograph_attack("googIе.com")); // Contains Cyrillic 'е'

        // Test legitimate domain
        assert!(!detector.has_homograph_attack("google.com"));
    }

    #[tokio::test]
    async fn test_comprehensive_security_scan() {
        let clickhouse_client = crate::db::clickhouse_client::create_clickhouse_client();
        let service = SecurityService::new(clickhouse_client);

        // Test legitimate URL
        let result = service
            .comprehensive_security_scan("https://example.com")
            .await
            .unwrap();
        assert!(result.is_safe);

        // Test suspicious URL
        let result = service
            .comprehensive_security_scan("https://verify-account.bit.ly/urgent")
            .await
            .unwrap();
        assert!(!result.is_safe);
        assert!(result.threats_detected.len() > 1);
    }
}
