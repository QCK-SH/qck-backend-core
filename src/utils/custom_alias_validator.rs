// DEV-114: Custom Alias Validator as specified in requirements
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref ALIAS_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_-]*$").unwrap();
}

pub struct CustomAliasValidator;

impl CustomAliasValidator {
    /// Validate a custom alias according to business rules
    pub fn validate(alias: &str) -> Result<(), String> {
        // Length validation
        if alias.len() < 3 {
            return Err("Custom alias must be at least 3 characters long".to_string());
        }

        if alias.len() > 50 {
            return Err("Custom alias must be no more than 50 characters long".to_string());
        }

        // Format validation
        if !ALIAS_REGEX.is_match(alias) {
            return Err("Custom alias can only contain letters, numbers, hyphens, and underscores, and must start with a letter or number".to_string());
        }

        // Check for consecutive special characters
        if alias.contains("--")
            || alias.contains("__")
            || alias.contains("-_")
            || alias.contains("_-")
        {
            return Err("Custom alias cannot contain consecutive special characters".to_string());
        }

        // Check if it ends with special character
        if alias.ends_with('-') || alias.ends_with('_') {
            return Err("Custom alias cannot end with a special character".to_string());
        }

        Ok(())
    }
}
