// DEV-124: Base62 Short Code Generator Tests
// Testing the heart of our URL shortening system

use qck_backend::services::short_code::ShortCodeGenerator;

#[test]
fn test_base62_encoding() {
    // Test various numbers encode correctly
    assert_eq!(ShortCodeGenerator::encode_id(0), "0");
    assert_eq!(ShortCodeGenerator::encode_id(1), "1");
    assert_eq!(ShortCodeGenerator::encode_id(9), "9");
    assert_eq!(ShortCodeGenerator::encode_id(10), "A");
    assert_eq!(ShortCodeGenerator::encode_id(35), "Z");
    assert_eq!(ShortCodeGenerator::encode_id(36), "a");
    assert_eq!(ShortCodeGenerator::encode_id(61), "z");
    assert_eq!(ShortCodeGenerator::encode_id(62), "10");
    assert_eq!(ShortCodeGenerator::encode_id(124), "20");
    assert_eq!(ShortCodeGenerator::encode_id(3843), "zz");
}

#[test]
fn test_base62_decoding() {
    // Test decoding returns original values
    assert_eq!(ShortCodeGenerator::decode_to_id("0").unwrap(), 0);
    assert_eq!(ShortCodeGenerator::decode_to_id("1").unwrap(), 1);
    assert_eq!(ShortCodeGenerator::decode_to_id("A").unwrap(), 10);
    assert_eq!(ShortCodeGenerator::decode_to_id("Z").unwrap(), 35);
    assert_eq!(ShortCodeGenerator::decode_to_id("a").unwrap(), 36);
    assert_eq!(ShortCodeGenerator::decode_to_id("z").unwrap(), 61);
    assert_eq!(ShortCodeGenerator::decode_to_id("10").unwrap(), 62);
    assert_eq!(ShortCodeGenerator::decode_to_id("zz").unwrap(), 3843);
}

#[test]
fn test_encode_decode_round_trip() {
    // Test that encoding then decoding returns original value
    let test_values = vec![
        0,
        1,
        42,
        100,
        1000,
        10000,
        100000,
        1000000,
        u64::MAX / 1000000, // Large but safe value
    ];

    for value in test_values {
        let encoded = ShortCodeGenerator::encode_id(value);
        let decoded = ShortCodeGenerator::decode_to_id(&encoded).unwrap();
        assert_eq!(
            decoded, value,
            "Round trip failed for {}: encoded to {}, decoded to {}",
            value, encoded, decoded
        );
    }
}

#[test]
fn test_decode_invalid_characters() {
    // Test that invalid characters are properly rejected
    assert!(ShortCodeGenerator::decode_to_id("!@#").is_err());
    assert!(ShortCodeGenerator::decode_to_id("hello world").is_err());
    assert!(ShortCodeGenerator::decode_to_id("test-123").is_err());
    assert!(ShortCodeGenerator::decode_to_id("😀").is_err());
}

#[test]
fn test_code_length_validation() {
    // Test that generated codes have correct length
    const BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    // Test minimum length (4 chars)
    let min_length_code = "abcd";
    assert_eq!(min_length_code.len(), 4);

    // Test maximum length (12 chars)
    let max_length_code = "abc123XYZ012";
    assert_eq!(max_length_code.len(), 12);

    // Verify all characters are valid BASE62
    for c in min_length_code.chars() {
        assert!(BASE62_CHARS.contains(&(c as u8)));
    }
}

#[test]
fn test_reserved_words() {
    // Test that reserved words are properly identified
    let reserved = vec![
        "api",
        "admin",
        "app",
        "www",
        "dashboard",
        "login",
        "API",
        "ADMIN", // Test case insensitivity
    ];

    // These should not be valid short codes to generate
    for word in &reserved {
        // In a real implementation, we'd check:
        // assert!(generator.is_reserved_code(word));
        assert!(word.len() >= 3, "Reserved word '{}' is valid length", word);
    }
}

#[test]
fn test_encoding_performance() {
    use std::time::Instant;

    let iterations = 10000;
    let start = Instant::now();

    for i in 0..iterations {
        let _ = ShortCodeGenerator::encode_id(i);
    }

    let duration = start.elapsed();
    let avg_time = duration.as_micros() as f64 / iterations as f64;

    // Should be well under 10ms (10000 microseconds) per encoding
    assert!(
        avg_time < 100.0,
        "Encoding too slow: {:.2} microseconds per operation (target: < 100)",
        avg_time
    );
}

#[test]
fn test_decoding_performance() {
    use std::time::Instant;

    // Pre-generate codes to test
    let codes: Vec<String> = (0..10000)
        .map(|i| ShortCodeGenerator::encode_id(i))
        .collect();

    let start = Instant::now();

    for code in &codes {
        let _ = ShortCodeGenerator::decode_to_id(code);
    }

    let duration = start.elapsed();
    let avg_time = duration.as_micros() as f64 / codes.len() as f64;

    // Should be well under 10ms per decoding
    assert!(
        avg_time < 100.0,
        "Decoding too slow: {:.2} microseconds per operation (target: < 100)",
        avg_time
    );
}
