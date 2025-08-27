// Utility modules for QCK Backend

pub mod auth_errors;
pub mod device_fingerprint;
pub mod password;
pub mod validation;

pub use auth_errors::{
    create_auth_audit_entry, log_auth_failure, AuthAuditEntry, AuthError, AuthErrorResponse,
    AuthEventType,
};
pub use device_fingerprint::generate_device_fingerprint;
pub use password::{hash_password, verify_password, PasswordError};
pub use validation::{trim_and_validate_field, trim_optional_field};
