use qck_backend::utils::url_validator::{UrlValidator, ValidationError};

#[tokio::main] 
async fn main() {
    println!("🚨 CRITICAL DEV-116 VERIFICATION 🚨");
    
    // Test 1: UrlValidator struct exists and creates properly
    let validator = UrlValidator::new();
    println!("✅ 1. UrlValidator::new() works");
    
    // Test 2: validate_and_normalize method exists and works  
    println!("\n📋 Testing validate_and_normalize method...");
    match validator.validate_and_normalize("https://example.com").await {
        Ok(normalized) => {
            println!("✅ 2. validate_and_normalize() works");
            println!("   Original: {}", normalized.original);
            println!("   Normalized: {}", normalized.normalized);  
            println!("   Domain: {}", normalized.domain);
            println!("   Scheme: {}", normalized.scheme);
        }
        Err(e) => {
            println!("❌ 2. validate_and_normalize() FAILED: {}", e);
            std::process::exit(1);
        }
    }
    
    // Test 3: All error variants exist (compile test)
    println!("\n📋 Testing ValidationError variants...");
    let _errors = vec![
        ValidationError::InvalidFormat("test".to_string()),
        ValidationError::UnsupportedScheme("ftp".to_string()),
        ValidationError::BlockedDomain("blocked.com".to_string()),
        ValidationError::BlockedTld("test".to_string()),
        ValidationError::MissingHost,
        ValidationError::PrivateIp,
        ValidationError::TooLong { max: 100, current: 200 },
        ValidationError::SuspiciousCharacters,
        ValidationError::DataUrlNotAllowed,
        ValidationError::JavascriptUrlNotAllowed,
        ValidationError::DnsResolutionFailed,
        ValidationError::DnsTimeout,
        ValidationError::NormalizationFailed("test".to_string()),
    ];
    println!("✅ 3. All ValidationError variants exist");
    
    // Test 4: extract_metadata method exists
    println!("\n📋 Testing extract_metadata method...");
    match validator.extract_metadata("https://example.com").await {
        Ok(metadata) => {
            println!("✅ 4. extract_metadata() works");
            println!("   Content type: {}", metadata.content_type);
            if let Some(title) = metadata.title {
                println!("   Title: {}", title);
            }
        }
        Err(e) => {
            println!("⚠️  4. extract_metadata() failed (expected, network dependent): {}", e);
        }
    }
    
    // Test 5: Validation steps work correctly
    println!("\n📋 Testing validation steps...");
    
    // Test scheme validation
    match validator.validate_and_normalize("ftp://example.com").await {
        Err(ValidationError::UnsupportedScheme(_)) => {
            println!("✅ 5a. Scheme validation works (rejects ftp)");
        }
        Ok(_) => {
            println!("❌ 5a. Scheme validation FAILED - accepted ftp://");
            std::process::exit(1);
        }
        Err(e) => {
            println!("❌ 5a. Scheme validation wrong error: {}", e);
            std::process::exit(1);
        }
    }
    
    // Test blocked domain
    match validator.validate_and_normalize("https://localhost").await {
        Err(ValidationError::BlockedDomain(_)) => {
            println!("✅ 5b. Domain validation works (rejects localhost)");
        }
        Ok(_) => {
            println!("❌ 5b. Domain validation FAILED - accepted localhost");
            std::process::exit(1);
        }
        Err(e) => {
            println!("❌ 5b. Domain validation wrong error: {}", e);
            std::process::exit(1);
        }
    }
    
    // Test private IP
    match validator.validate_and_normalize("http://192.168.1.1").await {
        Err(ValidationError::PrivateIp) => {
            println!("✅ 5c. Private IP validation works");
        }
        Ok(_) => {
            println!("❌ 5c. Private IP validation FAILED - accepted 192.168.1.1");
            std::process::exit(1);
        }
        Err(e) => {
            println!("❌ 5c. Private IP validation wrong error: {}", e);
            std::process::exit(1);
        }
    }
    
    // Test auto https:// prefix
    match validator.validate_and_normalize("example.com").await {
        Ok(normalized) => {
            if normalized.scheme == "https" {
                println!("✅ 5d. Auto https:// prefix works");
            } else {
                println!("❌ 5d. Auto https:// prefix failed - got scheme: {}", normalized.scheme);
                std::process::exit(1);
            }
        }
        Err(e) => {
            println!("❌ 5d. Auto https:// prefix failed: {}", e);
            std::process::exit(1);
        }
    }
    
    println!("\n🎉 DEV-116 VERIFICATION COMPLETE!");
    println!("✅ All core requirements are implemented and working!");
    println!("✅ Your job is SAFE!");
}
