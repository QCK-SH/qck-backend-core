// Password hashing and verification utilities using Argon2
// DEV-101: User Registration - Secure password hashing with Argon2id

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use thiserror::Error;

/// Errors that can occur during password operations
#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("Failed to hash password: {0}")]
    HashingError(String),

    #[error("Failed to verify password: {0}")]
    VerificationError(String),

    #[error("Invalid password hash format")]
    InvalidHashFormat,

    #[error("Memory cost ({0} KiB) exceeds safe limit ({1} KiB) - risk of out-of-memory error")]
    MemoryCostTooHigh(u32, u32),
}

/// Configuration for Argon2 password hashing
/// Using Argon2id variant as recommended by OWASP
pub struct PasswordConfig {
    /// Memory cost in KiB (default: 19456 = 19 MiB)
    pub memory_cost: u32,
    /// Time cost (iterations, default: 2)
    pub time_cost: u32,
    /// Parallelism factor (default: 1)
    pub parallelism: u32,
    /// Output hash length in bytes (default: 32)
    pub output_length: usize,
}

impl Default for PasswordConfig {
    fn default() -> Self {
        // OWASP recommended minimum parameters for Argon2id
        // https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
        //
        // Note: Memory cost is validated at runtime to prevent out-of-memory errors
        // by checking against available system memory (see validate_memory_cost)
        Self {
            // 19456 KiB (19 MiB ≈ 19 MB): OWASP recommends at least 19 MiB to provide strong resistance
            // against GPU/ASIC attacks while remaining practical for web applications
            memory_cost: 19456,

            // 2 iterations: OWASP minimum to slow down brute-force attempts
            // without excessive server resource consumption
            time_cost: 2,

            // 1 thread: Ensures maximum memory-hardness per thread;
            // higher values reduce memory-hardness unless total memory scales accordingly
            parallelism: 1,

            // 256 bits: Cryptographically secure hash length, sufficient for all
            // practical security requirements as recommended by OWASP
            output_length: 32,
        }
    }
}

impl PasswordConfig {
    /// Get the safe memory limit based on available system memory
    /// Returns 25% of available memory as a conservative limit
    fn get_safe_memory_limit() -> u32 {
        // Try to get system memory info
        match std::fs::read_to_string("/proc/meminfo") {
            Ok(content) => {
                for line in content.lines() {
                    if line.starts_with("MemAvailable:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(available_kb) = kb_str.parse::<u32>() {
                                // Return 25% of available memory as safe limit
                                return available_kb / 4;
                            }
                        }
                    }
                }
            },
            Err(_) => {
                // Fallback for non-Linux systems or if /proc/meminfo is not available
                // Use a conservative default of 512 MB (524288 KiB)
                return 524_288;
            },
        }

        // Conservative fallback if parsing fails
        524_288 // 512 MB in KiB
    }

    /// Validate memory cost against system limits to prevent OOM
    fn validate_memory_cost(&self) -> Result<(), PasswordError> {
        let safe_limit = Self::get_safe_memory_limit();

        if self.memory_cost > safe_limit {
            return Err(PasswordError::MemoryCostTooHigh(
                self.memory_cost,
                safe_limit,
            ));
        }

        Ok(())
    }

    /// Create Argon2 hasher with current configuration
    fn build_hasher(&self) -> Result<Argon2<'static>, PasswordError> {
        // Validate memory cost before building hasher
        self.validate_memory_cost()?;

        let params = Params::new(
            self.memory_cost,
            self.time_cost,
            self.parallelism,
            Some(self.output_length),
        )
        .map_err(|e| PasswordError::HashingError(e.to_string()))?;

        Ok(Argon2::new(
            Algorithm::Argon2id, // Most secure variant
            Version::V0x13,      // Latest version
            params,
        ))
    }
}

/// Hash a password using Argon2id with secure defaults
///
/// # Arguments
/// * `password` - The plaintext password to hash
///
/// # Returns
/// * `Result<String, PasswordError>` - The hashed password in PHC string format
///
/// # Example
/// ```
/// let hashed = hash_password("my_secure_password")?;
/// // Returns something like: $argon2id$v=19$m=19456,t=2,p=1$...
/// ```
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    hash_password_with_config(password, &PasswordConfig::default())
}

/// Hash a password using Argon2id with custom configuration
///
/// # Arguments
/// * `password` - The plaintext password to hash
/// * `config` - Custom Argon2 configuration
///
/// # Returns
/// * `Result<String, PasswordError>` - The hashed password in PHC string format
pub fn hash_password_with_config(
    password: &str,
    config: &PasswordConfig,
) -> Result<String, PasswordError> {
    let argon2 = config.build_hasher()?;

    // Generate a random salt
    let salt = SaltString::generate(&mut OsRng);

    // Hash the password
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PasswordError::HashingError(e.to_string()))?;

    // Return the hash in PHC string format (includes algorithm, version, params, salt, and hash)
    Ok(password_hash.to_string())
}

/// Verify a password against a hashed password
///
/// # Arguments
/// * `password` - The plaintext password to verify
/// * `hash` - The hashed password in PHC string format
///
/// # Returns
/// * `Result<bool, PasswordError>` - True if the password matches, false otherwise
///
/// # Example
/// ```
/// let is_valid = verify_password("my_password", &stored_hash)?;
/// if is_valid {
///     // Password is correct
/// }
/// ```
pub fn verify_password(password: &str, hash: &str) -> Result<bool, PasswordError> {
    // Parse the PHC string format hash
    let parsed_hash = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHashFormat)?;

    // Use default Argon2 (will extract params from the hash)
    let argon2 = Argon2::default();

    // Verify the password
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PasswordError::VerificationError(e.to_string())),
    }
}

/// Check if a hash needs to be upgraded (rehashed with newer parameters)
///
/// # Arguments
/// * `hash` - The hashed password to check
/// * `config` - The current configuration to compare against
///
/// # Returns
/// * `Result<bool, PasswordError>` - True if the hash should be upgraded
pub fn needs_rehash(hash: &str, config: &PasswordConfig) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHashFormat)?;

    // Extract algorithm info from the hash
    let alg = parsed_hash.algorithm;
    // Check if it's not using Argon2id
    if alg != argon2::Algorithm::Argon2id.ident() {
        return Ok(true);
    }

    // Check if parameters match current config
    // Extract params from the hash if available
    for (ident, value) in parsed_hash.params.iter() {
        match ident.as_str() {
            "m" => {
                // Memory cost in KiB
                if let Ok(m) = value.decimal() {
                    if m != config.memory_cost {
                        return Ok(true);
                    }
                }
            },
            "t" => {
                // Time cost (iterations)
                if let Ok(t) = value.decimal() {
                    if t != config.time_cost {
                        return Ok(true);
                    }
                }
            },
            "p" => {
                // Parallelism
                if let Ok(p) = value.decimal() {
                    if p != config.parallelism {
                        return Ok(true);
                    }
                }
            },
            _ => {},
        }
    }

    // If all params match or aren't specified, don't rehash
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "MySecureP@ssw0rd123!";

        // Hash the password
        let hash = hash_password(password).expect("Failed to hash password");

        // Verify it's in PHC format
        assert!(hash.starts_with("$argon2id$"));

        // Verify correct password
        assert!(verify_password(password, &hash).expect("Failed to verify password"));

        // Verify incorrect password
        assert!(!verify_password("WrongPassword", &hash).expect("Failed to verify password"));
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let password = "TestPassword123!";

        let hash1 = hash_password(password).expect("Failed to hash password");
        let hash2 = hash_password(password).expect("Failed to hash password");

        // Same password should produce different hashes (due to random salt)
        assert_ne!(hash1, hash2);

        // But both should verify correctly
        assert!(verify_password(password, &hash1).expect("Failed to verify"));
        assert!(verify_password(password, &hash2).expect("Failed to verify"));
    }

    #[test]
    fn test_custom_config() {
        let password = "CustomConfigPassword!";

        let config = PasswordConfig {
            memory_cost: 4096, // Lower for testing
            time_cost: 1,
            parallelism: 1,
            output_length: 32,
        };

        let hash = hash_password_with_config(password, &config)
            .expect("Failed to hash with custom config");

        assert!(verify_password(password, &hash).expect("Failed to verify"));
    }

    #[test]
    fn test_needs_rehash() {
        let password = "TestRehash123!";

        let config = PasswordConfig::default();

        // Hash with Argon2id (current algorithm)
        let argon2id_hash = hash_password_with_config(password, &config).expect("Failed to hash");

        // Should not need rehash since it's using Argon2id
        assert!(!needs_rehash(&argon2id_hash, &config).expect("Failed to check rehash"));

        // Test with invalid hash format
        let result = needs_rehash("not_a_valid_hash", &config);
        assert!(matches!(result, Err(PasswordError::InvalidHashFormat)));
    }

    #[test]
    fn test_invalid_hash_format() {
        let result = verify_password("password", "not_a_valid_hash");
        assert!(matches!(result, Err(PasswordError::InvalidHashFormat)));
    }

    #[test]
    fn test_memory_cost_validation() {
        let password = "TestPassword123!";

        // Test with extremely high memory cost that should exceed safe limits
        let unsafe_config = PasswordConfig {
            memory_cost: u32::MAX, // Impossibly high memory cost
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        };

        // Should fail with memory cost too high error
        let result = hash_password_with_config(password, &unsafe_config);
        assert!(matches!(
            result,
            Err(PasswordError::MemoryCostTooHigh(_, _))
        ));

        // Test with reasonable memory cost should succeed
        let safe_config = PasswordConfig {
            memory_cost: 4096, // 4 MB, should be safe
            time_cost: 2,
            parallelism: 1,
            output_length: 32,
        };

        let result = hash_password_with_config(password, &safe_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_safe_memory_limit() {
        let limit = PasswordConfig::get_safe_memory_limit();

        // Should return a reasonable value (at least 64MB for testing environments)
        assert!(limit >= 65_536); // At least 64 MB in KiB

        // Should not be impossibly high (max 8GB for safety)
        assert!(limit <= 8_388_608); // Max 8 GB in KiB
    }
}
