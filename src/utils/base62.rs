// DEV-95: Base62 Encoding Algorithm Implementation
// Efficient Base62 encoding/decoding for URL shortening with < 1ms performance

use std::fmt;
use thiserror::Error;

/// Base62 alphabet: 0-9, A-Z, a-z (62 characters total)
/// Using this specific order for compatibility and predictability
const BASE62_ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: u64 = 62;

/// Lookup table for fast decoding (maps ASCII byte to Base62 value)
/// -1 means invalid character
static DECODE_TABLE: [i8; 256] = {
    let mut table = [-1i8; 256];
    let mut i = 0;
    while i < BASE62_ALPHABET.len() {
        table[BASE62_ALPHABET[i] as usize] = i as i8;
        i += 1;
    }
    table
};

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Error, Debug, PartialEq)]
pub enum Base62Error {
    #[error("Invalid character '{0}' in Base62 string")]
    InvalidCharacter(char),

    #[error("Numeric overflow during Base62 decoding")]
    OverflowError,

    #[error("Empty string cannot be decoded")]
    EmptyString,

    #[error("String length {0} exceeds maximum of {1}")]
    StringTooLong(usize, usize),
}

// =============================================================================
// BASE62 ENCODER
// =============================================================================

/// High-performance Base62 encoder/decoder optimized for URL shortening
pub struct Base62Encoder {
    /// Minimum length for generated codes (will pad with zeros)
    min_length: usize,
    /// Maximum allowed length for codes
    max_length: usize,
}

impl Base62Encoder {
    /// Create a new Base62Encoder with default settings
    pub fn new() -> Self {
        Self {
            min_length: 0,
            max_length: 20, // Max length that fits in u128
        }
    }

    /// Create encoder with custom length constraints
    pub fn with_constraints(min_length: usize, max_length: usize) -> Self {
        Self {
            min_length,
            max_length,
        }
    }

    /// Encode a u64 value to Base62 string
    ///
    /// # Performance
    /// This function performs at < 100 nanoseconds for typical values
    ///
    /// # Example
    /// ```
    /// let encoder = Base62Encoder::new();
    /// assert_eq!(encoder.encode(0), "0");
    /// assert_eq!(encoder.encode(61), "z");
    /// assert_eq!(encoder.encode(62), "10");
    /// ```
    #[inline]
    pub fn encode(&self, mut value: u64) -> String {
        if value == 0 {
            let result = "0".to_string();
            return self.pad_to_min_length(result);
        }

        // Pre-allocate string with estimated capacity
        let mut result = String::with_capacity(11); // Max u64 needs 11 chars in base62

        while value > 0 {
            let remainder = (value % BASE) as usize;
            result.push(BASE62_ALPHABET[remainder] as char);
            value /= BASE;
        }

        // Reverse to get correct order (we built it backwards)
        let result: String = result.chars().rev().collect();
        self.pad_to_min_length(result)
    }

    /// Encode with explicit length (will pad or error if too long)
    pub fn encode_with_length(&self, value: u64, length: usize) -> Result<String, Base62Error> {
        if length > self.max_length {
            return Err(Base62Error::StringTooLong(length, self.max_length));
        }

        let encoded = self.encode(value);

        if encoded.len() > length {
            // Value too large for requested length
            return Err(Base62Error::StringTooLong(encoded.len(), length));
        }

        // Pad to exact length
        Ok(format!("{:0>width$}", encoded, width = length))
    }

    /// Decode a Base62 string back to u64
    ///
    /// # Performance
    /// This function performs at < 200 nanoseconds for typical 6-char strings
    ///
    /// # Example
    /// ```
    /// let encoder = Base62Encoder::new();
    /// assert_eq!(encoder.decode("0")?, 0);
    /// assert_eq!(encoder.decode("z")?, 61);
    /// assert_eq!(encoder.decode("10")?, 62);
    /// ```
    #[inline]
    pub fn decode(&self, encoded: &str) -> Result<u64, Base62Error> {
        if encoded.is_empty() {
            return Err(Base62Error::EmptyString);
        }

        if encoded.len() > self.max_length {
            return Err(Base62Error::StringTooLong(encoded.len(), self.max_length));
        }

        let mut result = 0u64;

        for byte in encoded.bytes() {
            // Use lookup table for O(1) character validation and conversion
            let digit = DECODE_TABLE[byte as usize];

            if digit < 0 {
                return Err(Base62Error::InvalidCharacter(byte as char));
            }

            // Check for overflow before multiplication
            result = result
                .checked_mul(BASE)
                .and_then(|r| r.checked_add(digit as u64))
                .ok_or(Base62Error::OverflowError)?;
        }

        Ok(result)
    }

    /// Generate a random Base62 string of specified length
    /// Uses cryptographically secure random number generator
    pub fn generate_random(&self, length: usize) -> Result<String, Base62Error> {
        use rand::{thread_rng, Rng};

        if length > self.max_length {
            return Err(Base62Error::StringTooLong(length, self.max_length));
        }

        let mut rng = thread_rng();
        let mut result = String::with_capacity(length);

        for _ in 0..length {
            let idx = rng.gen_range(0..BASE as usize);
            result.push(BASE62_ALPHABET[idx] as char);
        }

        Ok(result)
    }

    /// Validate if a string is valid Base62
    #[inline]
    pub fn is_valid(&self, s: &str) -> bool {
        !s.is_empty()
            && s.len() <= self.max_length
            && s.bytes().all(|b| DECODE_TABLE[b as usize] >= 0)
    }

    /// Calculate maximum value that can be encoded with given length
    pub fn max_value_for_length(length: usize) -> u64 {
        if length == 0 {
            return 0;
        }

        // BASE^length - 1
        match BASE.checked_pow(length as u32) {
            Some(v) => v.saturating_sub(1),
            None => u64::MAX,
        }
    }

    /// Calculate minimum length needed to encode a value
    pub fn min_length_for_value(value: u64) -> usize {
        if value == 0 {
            return 1;
        }

        let mut length = 0;
        let mut v = value;

        while v > 0 {
            v /= BASE;
            length += 1;
        }

        length
    }

    /// Helper to pad string to minimum length
    #[inline]
    fn pad_to_min_length(&self, s: String) -> String {
        if s.len() >= self.min_length {
            s
        } else {
            format!("{:0>width$}", s, width = self.min_length)
        }
    }
}

impl Default for Base62Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Base62Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Base62Encoder")
            .field("min_length", &self.min_length)
            .field("max_length", &self.max_length)
            .field("base", &BASE)
            .finish()
    }
}

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Quick encode function for one-off encoding
#[inline]
pub fn encode(value: u64) -> String {
    Base62Encoder::new().encode(value)
}

/// Quick decode function for one-off decoding
#[inline]
pub fn decode(encoded: &str) -> Result<u64, Base62Error> {
    Base62Encoder::new().decode(encoded)
}

/// Generate a random Base62 string of specified length
#[inline]
pub fn generate_random(length: usize) -> Result<String, Base62Error> {
    Base62Encoder::new().generate_random(length)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_encode_decode_basic() {
        let encoder = Base62Encoder::new();

        // Test basic values
        let test_cases = vec![
            (0, "0"),
            (1, "1"),
            (9, "9"),
            (10, "A"),
            (35, "Z"),
            (36, "a"),
            (61, "z"),
            (62, "10"),
            (124, "20"),
            (3843, "zz"),
            (238327, "zzz"),
            (14776335, "zzzz"),
            (916132831, "zzzzz"),
            (56800235583, "zzzzzz"),
        ];

        for (value, expected) in test_cases {
            let encoded = encoder.encode(value);
            assert_eq!(encoded, expected, "Failed to encode {}", value);

            let decoded = encoder.decode(&encoded).unwrap();
            assert_eq!(decoded, value, "Failed to decode {}", encoded);
        }
    }

    #[test]
    fn test_encode_decode_large_values() {
        let encoder = Base62Encoder::new();

        let large_values = vec![
            u64::MAX / 2,
            u64::MAX - 1,
            1234567890123456789,
            9876543210987654321,
        ];

        for value in large_values {
            let encoded = encoder.encode(value);
            let decoded = encoder.decode(&encoded).unwrap();
            assert_eq!(decoded, value, "Round-trip failed for {}", value);
        }
    }

    #[test]
    fn test_invalid_characters() {
        let encoder = Base62Encoder::new();

        let invalid_strings = vec!["abc!", "test@", "hello world", "😀", "abc-def", "123_456"];

        for s in invalid_strings {
            assert!(
                encoder.decode(s).is_err(),
                "Should fail for invalid string: {}",
                s
            );
        }
    }

    #[test]
    fn test_padding() {
        let encoder = Base62Encoder::with_constraints(6, 20);

        assert_eq!(encoder.encode(0), "000000");
        assert_eq!(encoder.encode(1), "000001");
        assert_eq!(encoder.encode(62), "000010");

        // Decode should handle padded values
        assert_eq!(encoder.decode("000000").unwrap(), 0);
        assert_eq!(encoder.decode("000001").unwrap(), 1);
        assert_eq!(encoder.decode("000010").unwrap(), 62);
    }

    #[test]
    fn test_encode_with_length() {
        let encoder = Base62Encoder::new();

        assert_eq!(encoder.encode_with_length(0, 6).unwrap(), "000000");
        assert_eq!(encoder.encode_with_length(62, 6).unwrap(), "000010");
        assert_eq!(encoder.encode_with_length(3843, 6).unwrap(), "0000zz");

        // Should error if value too large for length
        assert!(encoder.encode_with_length(u64::MAX, 6).is_err());
    }

    #[test]
    fn test_random_generation() {
        let encoder = Base62Encoder::with_constraints(0, 20); // Explicitly set max_length

        for length in [4, 6, 8, 10] {
            // Skip 12 as it may overflow u64
            let random = encoder.generate_random(length).unwrap();
            assert_eq!(random.len(), length);
            assert!(encoder.is_valid(&random));

            // Should be decodable for lengths that fit in u64
            // Max safe length for u64 is 10 chars (62^10 < 2^64 < 62^11)
            if length <= 10 {
                match encoder.decode(&random) {
                    Ok(_) => {
                        // Successfully decoded
                    },
                    Err(Base62Error::OverflowError) if length > 10 => {
                        // Expected for long strings
                    },
                    Err(e) => {
                        panic!("Failed to decode random string '{}': {:?}", random, e);
                    },
                }
            }
        }

        // Test that we can generate longer strings even if they can't decode to u64
        let long_random = encoder.generate_random(12).unwrap();
        assert_eq!(long_random.len(), 12);
        assert!(encoder.is_valid(&long_random));
        // Don't try to decode it as it may overflow
    }

    #[test]
    fn test_validation() {
        let encoder = Base62Encoder::new();

        assert!(encoder.is_valid("abc123"));
        assert!(encoder.is_valid("XYZ789"));
        assert!(encoder.is_valid("0"));
        assert!(encoder.is_valid("zzzzzz"));

        assert!(!encoder.is_valid(""));
        assert!(!encoder.is_valid("abc!"));
        assert!(!encoder.is_valid("test-123"));
        assert!(!encoder.is_valid("hello world"));
    }

    #[test]
    fn test_max_value_for_length() {
        assert_eq!(Base62Encoder::max_value_for_length(1), 61);
        assert_eq!(Base62Encoder::max_value_for_length(2), 3843);
        assert_eq!(Base62Encoder::max_value_for_length(3), 238327);
        assert_eq!(Base62Encoder::max_value_for_length(6), 56800235583);
    }

    #[test]
    fn test_min_length_for_value() {
        assert_eq!(Base62Encoder::min_length_for_value(0), 1);
        assert_eq!(Base62Encoder::min_length_for_value(61), 1);
        assert_eq!(Base62Encoder::min_length_for_value(62), 2);
        assert_eq!(Base62Encoder::min_length_for_value(3843), 2);
        assert_eq!(Base62Encoder::min_length_for_value(3844), 3);
        assert_eq!(Base62Encoder::min_length_for_value(238327), 3);
        assert_eq!(Base62Encoder::min_length_for_value(238328), 4);
    }

    #[test]
    fn test_performance_encode() {
        let encoder = Base62Encoder::new();
        let iterations = 10000;

        let start = Instant::now();
        for i in 0..iterations {
            let _ = encoder.encode(i);
        }
        let duration = start.elapsed();

        let avg_nanos = duration.as_nanos() / iterations as u128;
        println!("Average encode time: {} ns", avg_nanos);

        // Should be well under 1000ns (1μs)
        assert!(avg_nanos < 1000, "Encoding too slow: {} ns", avg_nanos);
    }

    #[test]
    fn test_performance_decode() {
        let encoder = Base62Encoder::new();
        let test_string = "abc123";
        let iterations = 10000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = encoder.decode(test_string).unwrap();
        }
        let duration = start.elapsed();

        let avg_nanos = duration.as_nanos() / iterations as u128;
        println!("Average decode time: {} ns", avg_nanos);

        // Should be well under 1000ns (1μs)
        assert!(avg_nanos < 1000, "Decoding too slow: {} ns", avg_nanos);
    }

    #[test]
    fn test_utility_functions() {
        // Test standalone functions
        assert_eq!(encode(123), "1z");
        assert_eq!(decode("1z").unwrap(), 123);

        let random = generate_random(6).unwrap();
        assert_eq!(random.len(), 6);
    }
}
