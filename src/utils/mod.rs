// Utility modules for QCK Backend

pub mod audit_logger;
pub mod auth_errors;
pub mod base62;
pub mod custom_alias_validator;
pub mod device_fingerprint;
pub mod link_errors;
pub mod password;
pub mod security_scanner;
pub mod service_error;
pub mod url_validator;
pub mod urlhaus_client;
pub mod validation;

pub use auth_errors::{
    create_auth_audit_entry, log_auth_failure, AuthAuditEntry, AuthError, AuthErrorResponse,
    AuthEventType,
};
pub use device_fingerprint::generate_device_fingerprint;
pub use link_errors::{LinkError, LinkErrorResponse, LinkResult};
pub use password::{hash_password, verify_password, PasswordError};
pub use security_scanner::{
    DomainSecurityService, SecurityError, SecurityRiskLevel,
    SecurityScanResult as SecurityScanResultV2, SecurityService, SecurityWarning, ThreatType,
    UrlPatternAnalyzer,
};
pub use url_validator::{
    normalize_url, normalize_url_async, MetadataError, NormalizedUrl, RiskLevel,
    SecurityScanResult, SecurityScanner, UrlMetadata, UrlValidationError, UrlValidator,
    ValidationError,
};
pub use validation::{trim_and_validate_field, trim_optional_field};
