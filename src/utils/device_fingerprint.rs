// Device fingerprinting utility for enhanced security
// Generates a unique device identifier from client characteristics

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;

/// Generate a device fingerprint from client characteristics
///
/// Creates a 64-character hash based on:
/// - User agent string
/// - IP address
/// - Client timezone (if provided via x-client-timezone header)
/// - Screen resolution (if provided via x-client-screen-resolution header)
/// - Language preferences (from x-client-language or Accept-Language headers)
/// - Encoding capabilities (from Accept-Encoding header)
pub fn generate_device_fingerprint(
    user_agent: &Option<String>,
    addr: &SocketAddr,
    client_timezone: &Option<String>,
    client_screen_res: &Option<String>,
    client_language: &Option<String>,
    headers: &HeaderMap,
) -> Option<String> {
    let ua = user_agent.as_ref()?;

    let mut hasher = Sha256::new();

    // Include user agent
    hasher.update(ua.as_bytes());

    // Include IP address
    hasher.update(addr.ip().to_string().as_bytes());

    // Include client timezone if provided
    if let Some(tz) = client_timezone {
        hasher.update(tz.as_bytes());
    }

    // Include screen resolution if provided
    if let Some(sr) = client_screen_res {
        hasher.update(sr.as_bytes());
    }

    // Include client language if provided
    if let Some(lang) = client_language {
        hasher.update(lang.as_bytes());
    } else {
        // Fallback to Accept-Language header if custom header not present
        if let Some(lang) = headers.get("accept-language") {
            if let Ok(lang_str) = lang.to_str() {
                hasher.update(lang_str.as_bytes());
            }
        }
    }

    // Include Accept-Encoding header if present
    if let Some(encoding) = headers.get("accept-encoding") {
        if let Ok(enc_str) = encoding.to_str() {
            hasher.update(enc_str.as_bytes());
        }
    }

    // Generate a 64-character fingerprint (full SHA-256 hex)
    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_device_fingerprint_generation() {
        let mut headers = HeaderMap::new();
        headers.insert("accept-language", HeaderValue::from_static("en-US"));
        headers.insert("accept-encoding", HeaderValue::from_static("gzip, deflate"));

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let user_agent = Some("Mozilla/5.0".to_string());
        let timezone = Some("America/New_York".to_string());
        let screen_res = Some("1920x1080".to_string());
        let language = Some("en-US".to_string());

        let fingerprint = generate_device_fingerprint(
            &user_agent,
            &addr,
            &timezone,
            &screen_res,
            &language,
            &headers,
        );

        assert!(fingerprint.is_some());
        let fp = fingerprint.unwrap();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_device_fingerprint_without_user_agent() {
        let headers = HeaderMap::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let user_agent = None;

        let fingerprint =
            generate_device_fingerprint(&user_agent, &addr, &None, &None, &None, &headers);

        assert!(fingerprint.is_none());
    }

    #[test]
    fn test_device_fingerprint_consistency() {
        let mut headers = HeaderMap::new();
        headers.insert("accept-encoding", HeaderValue::from_static("gzip"));

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 3000);
        let user_agent = Some("Chrome/120.0".to_string());

        let fp1 = generate_device_fingerprint(&user_agent, &addr, &None, &None, &None, &headers);

        let fp2 = generate_device_fingerprint(&user_agent, &addr, &None, &None, &None, &headers);

        assert_eq!(fp1, fp2, "Same inputs should produce same fingerprint");
    }
}
