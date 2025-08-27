// Validation utilities for string fields

/// Trim and validate string fields
///
/// # Arguments
/// * `field` - The string field to validate
/// * `required` - Whether the field is required (cannot be empty)
///
/// # Returns
/// * `Ok(String)` - The trimmed string if valid
/// * `Err(String)` - Error message if validation fails
pub fn trim_and_validate_field(field: &str, required: bool) -> Result<String, String> {
    let trimmed = field.trim().to_string();
    if trimmed.is_empty() {
        if required {
            Err("Field cannot be empty".to_string())
        } else {
            Ok(trimmed) // For optional fields, empty is valid
        }
    } else {
        Ok(trimmed)
    }
}

/// Trim and optionally validate a string field
///
/// # Arguments
/// * `field` - Optional string field to validate
///
/// # Returns
/// * `None` - If the field is None or empty after trimming
/// * `Some(String)` - The trimmed string if not empty
pub fn trim_optional_field(field: Option<&String>) -> Option<String> {
    field.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
