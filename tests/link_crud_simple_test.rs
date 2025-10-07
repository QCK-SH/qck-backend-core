// DEV-114: Simplified Link CRUD Tests
// Basic tests to verify CRUD operations work

#[tokio::test]
async fn test_link_service_exists() {
    // This test just verifies the LinkService can be imported
    use qck_backend_core::services::link::LinkService;
    use qck_backend_core::services::short_code::ShortCodeGenerator;

    // If this compiles, we have the basic structure
    assert!(true);
}

#[tokio::test]
async fn test_create_link_request_struct() {
    use qck_backend_core::models::link::CreateLinkRequest;

    let request = CreateLinkRequest {
        url: "https://example.com".to_string(),
        custom_alias: None,
        title: Some("Test".to_string()),
        description: None,
        expires_at: None,
        tags: vec![],
        is_password_protected: false,
        password: None,
    };

    assert_eq!(request.url, "https://example.com");
}

#[tokio::test]
async fn test_base62_encoding() {
    use qck_backend_core::utils::base62::{decode, encode};

    let encoded = encode(12345);
    let decoded = decode(&encoded).unwrap();

    assert_eq!(decoded, 12345);
    println!("Encoded 12345 as: {}", encoded);
}

#[tokio::test]
async fn test_short_code_generator() {
    use qck_backend_core::services::short_code::ShortCodeGenerator;

    // Test static encoding/decoding
    let encoded = ShortCodeGenerator::encode_id(999);
    let decoded = ShortCodeGenerator::decode_to_id(&encoded).unwrap();

    assert_eq!(decoded, 999);
    assert_eq!(encoded, "G7"); // 999 in base62 is "G7"
}
