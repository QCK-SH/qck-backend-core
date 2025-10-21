// Authentication Handlers for QCK Backend
// DEV-113: JWT Token Validation - Login, Refresh, Logout endpoints
// DEV-107: Enhanced with refresh token rotation and device tracking
// DEV-101: User Registration API with Argon2 password hashing

use axum::{
    extract::{ConnectInfo, Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use axum_extra::{
    extract::cookie::{Cookie, CookieJar, SameSite},
    headers::UserAgent,
    TypedHeader,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use time::Duration;
use validator::Validate;

use crate::{
    app::AppState,
    middleware::auth::AuthenticatedUser,
    models::{
        password_reset::{
            ForgotPasswordRequest, ForgotPasswordResponse, ResetPasswordRequest,
            ResetPasswordResponse,
        },
        user::{NewUser, OnboardingStatus, User, UserError},
    },
    services::{jwt::JwtError, rate_limit::RateLimitConfig},
    utils::{
        auth_errors::AuthError, generate_device_fingerprint, hash_password,
        trim_and_validate_field, trim_optional_field, verify_password,
    },
};

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RefreshRequest {
    // Make refresh_token optional for web clients (use cookie instead)
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email format"))]
    #[validate(length(max = 320, message = "Email must be less than 320 characters"))]
    pub email: String,

    #[validate(custom(function = "validate_password"))]
    pub password: String,

    pub password_confirmation: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Full name must be between 1 and 255 characters"
    ))]
    pub full_name: String,

    #[validate(length(max = 255, message = "Company name must be less than 255 characters"))]
    pub company_name: Option<String>,

    pub accept_terms: bool,
}

/// Custom password validation - min 8 chars, must have uppercase, lowercase, number, special char
fn validate_password(password: &str) -> Result<(), validator::ValidationError> {
    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if password.len() < 8 {
        return Err(validator::ValidationError::new("password_too_short"));
    }

    if !has_uppercase || !has_lowercase || !has_digit || !has_special {
        return Err(validator::ValidationError::new("password_complexity"));
    }

    Ok(())
}

/// Helper function to create standardized auth error responses
fn create_auth_error_response(message: &str) -> Response {
    let response = AuthResponse::<TokenResponse> {
        success: false,
        data: None,
        message: message.to_string(),
    };
    (StatusCode::BAD_REQUEST, Json(response)).into_response()
}

/// Helper function to create a cookie that deletes the refresh token
fn create_delete_refresh_cookie(config: &crate::app_config::AppConfig) -> Cookie<'static> {
    Cookie::build(("refresh_token", ""))
        .path("/")
        .http_only(true)
        .secure(config.is_production())
        .same_site(SameSite::Strict)
        .max_age(Duration::seconds(-1)) // Negative max_age deletes the cookie
        .build()
}

/// Helper function to create a refresh token cookie with configurable persistence
fn create_refresh_token_cookie(token: String, remember_me: bool, config: &crate::app_config::AppConfig) -> Cookie<'static> {
    let mut cookie_builder = Cookie::build(("refresh_token", token))
        .path("/")
        .http_only(true)
        .secure(config.is_production())
        .same_site(SameSite::Strict);

    // Only set max_age for remember_me - without it, cookie is session-only
    if remember_me {
        cookie_builder = cookie_builder.max_age(Duration::days(config.security.remember_me_duration_days as i64));
    }

    cookie_builder.build()
}

/// Validate JWT token format (must have exactly 3 parts separated by dots)
fn is_valid_jwt_format(token: &str) -> bool {
    token.split('.').count() == 3
}

/// Extract refresh token from cookie (web) or JSON body (mobile)
fn extract_refresh_token(jar: &CookieJar, body: &axum::body::Bytes) -> Result<String, Response> {
    // Try cookie first (web clients)
    if let Some(cookie) = jar.get("refresh_token") {
        let token = cookie.value();
        // Basic JWT format validation: must have 3 parts separated by dots
        if !is_valid_jwt_format(token) {
            return Err(create_auth_error_response("Invalid refresh token format"));
        }
        return Ok(token.to_string());
    }

    // Fall back to JSON body (mobile clients)
    if body.is_empty() {
        return Err(create_auth_error_response("Refresh token not provided"));
    }

    match serde_json::from_slice::<RefreshRequest>(body) {
        Ok(req) => {
            if let Some(token) = req.refresh_token {
                // Basic JWT format validation: must have 3 parts separated by dots
                if !is_valid_jwt_format(&token) {
                    return Err(create_auth_error_response("Invalid refresh token format"));
                }
                Ok(token)
            } else {
                Err(create_auth_error_response("Refresh token not provided"))
            }
        }
        Err(_) => Err(create_auth_error_response("Invalid JSON body")),
    }
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
    pub user: LoginUserInfo,
    pub remember_me: bool,
}

#[derive(Debug, Serialize)]
pub struct LoginUserInfo {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub subscription_tier: String,
    pub onboarding_status: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub email: String,
    pub full_name: String,
    pub company_name: Option<String>,
    pub email_verification_required: bool,
    pub verification_sent: bool,
    pub onboarding_status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub user_id: String,
    pub email: String,
    pub full_name: String,
    pub subscription_tier: String,
    pub onboarding_status: String,
    pub permissions: Vec<String>,
}

// =============================================================================
// CONSTANTS
// =============================================================================

// Failed login tracking expiry times are now configured in environment variables
// FAILED_LOGIN_EXPIRY_SECONDS and FAILED_LOGIN_IP_EXPIRY_SECONDS

// =============================================================================
// AUTHENTICATION HANDLERS
// =============================================================================

/// POST /auth/login - Authenticate user and return JWT tokens
/// DEV-102: Comprehensive login with rate limiting, account lockout, and remember_me
/// Supports both web (cookies) and mobile (JSON tokens) authentication
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    user_agent: Option<TypedHeader<UserAgent>>,
    jar: CookieJar,
    Json(login_req): Json<LoginRequest>,
) -> impl IntoResponse {
    use crate::utils::{create_auth_audit_entry, log_auth_failure, AuthError, AuthEventType};

    // Capture timestamp at request start for consistent timing throughout request
    let now_timestamp = chrono::Utc::now().timestamp();
    let ip_address = addr.ip().to_string();
    let user_agent = user_agent.map(|TypedHeader(ua)| ua.to_string());

    // Step 1: Validate email format
    let email = login_req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return AuthError::InvalidCredentials.into_response();
    }

    // Step 2: IP-based rate limiting (X attempts per minute) - if enabled
    let config = crate::app_config::config();
    if config.enable_rate_limiting {
        let ip_rate_key = format!("login:ip:{}", ip_address);
        let ip_rate_config = RateLimitConfig {
            max_requests: config.security.login_rate_limit_per_ip,
            window_seconds: 60,
            burst_limit: Some(config.security.login_rate_limit_per_ip),
            block_duration: 60,
            distributed: true,
        };

        match state
            .rate_limit_service
            .check_rate_limit_with_config(&ip_rate_key, &ip_rate_config)
            .await
        {
            Ok(status) if !status.allowed => {
                log_auth_failure(
                    &email,
                    &ip_address,
                    &AuthError::RateLimited {
                        retry_after_seconds: status.retry_after.unwrap_or(60) as u64,
                    },
                    user_agent.as_deref(),
                );

                return AuthError::RateLimited {
                    retry_after_seconds: status.retry_after.unwrap_or(60) as u64,
                }
                .into_response();
            },
            Err(e) => {
                tracing::warn!("Rate limit check failed for IP {}: {}", ip_address, e);
            },
            _ => {},
        }
    }

    // Step 3: Check account lockout status (moved before email rate limiting)
    // We check lockout early but only for emails we know exist
    if let Some(retry_after) = check_account_lockout_status(&state, &email).await {
        log_auth_failure(
            &email,
            &ip_address,
            &AuthError::AccountLocked {
                retry_after_seconds: retry_after,
            },
            user_agent.as_deref(),
        );

        return AuthError::AccountLocked {
            retry_after_seconds: retry_after,
        }
        .into_response();
    }

    // Step 5: Get user from database
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            return AuthError::InternalError.into_response();
        },
    };

    let user = match User::find_by_email(&mut conn, &email).await {
        Ok(user) => user,
        Err(UserError::NotFound) => {
            // Track failed attempt for non-existent users only by IP (prevent Redis memory exhaustion)
            // We don't track by email for non-existent users to avoid attackers filling Redis
            // with random email addresses
            track_failed_login_by_ip_only(&state, &ip_address).await;

            log_auth_failure(
                &email,
                &ip_address,
                &AuthError::InvalidCredentials,
                user_agent.as_deref(),
            );
            return AuthError::InvalidCredentials.into_response();
        },
        Err(e) => {
            tracing::error!("Database error during login: {}", e);
            return AuthError::DatabaseError(e.to_string()).into_response();
        },
    };

    // Step 5a: Email-based rate limiting (only for existing users to prevent abuse)
    let email_rate_key = format!("login:email:{}", email);
    let email_rate_config = RateLimitConfig {
        max_requests: config.security.login_rate_limit_per_email,
        window_seconds: 3600,
        burst_limit: Some(5),
        block_duration: 3600,
        distributed: true,
    };

    match state
        .rate_limit_service
        .check_rate_limit_with_config(&email_rate_key, &email_rate_config)
        .await
    {
        Ok(status) if !status.allowed => {
            log_auth_failure(
                &email,
                &ip_address,
                &AuthError::RateLimited {
                    retry_after_seconds: status.retry_after.unwrap_or(3600) as u64,
                },
                user_agent.as_deref(),
            );

            return AuthError::RateLimited {
                retry_after_seconds: status.retry_after.unwrap_or(3600) as u64,
            }
            .into_response();
        },
        Err(e) => {
            tracing::warn!("Rate limit check failed for email {}: {}", email, e);
        },
        _ => {},
    }

    // Step 6: Check if account is active
    if !user.is_active {
        log_auth_failure(
            &email,
            &ip_address,
            &AuthError::AccountInactive,
            user_agent.as_deref(),
        );
        return AuthError::AccountInactive.into_response();
    }

    // Step 7: Check if email is verified (configurable - disabled by default for now)
    let config = crate::app_config::config();
    if config.security.require_email_verification && !user.email_verified {
        log_auth_failure(
            &email,
            &ip_address,
            &AuthError::EmailNotVerified,
            user_agent.as_deref(),
        );
        return AuthError::EmailNotVerified.into_response();
    }

    // Step 8: Verify password
    match verify_password(&login_req.password, &user.password_hash) {
        Ok(true) => {
            // Password is correct
        },
        Ok(false) => {
            // Track failed attempt
            track_failed_login(&state, &email, &ip_address).await;

            // Check if we should lock the account
            let failed_attempts = get_failed_login_count(&state, &email).await;
            if failed_attempts >= config.security.login_lockout_threshold {
                // Lock the account
                let lockout_duration = config.security.login_lockout_duration_seconds;
                let locked_until = now_timestamp + lockout_duration as i64;
                let lockout_key = format!("lockout:{}", email);

                let _ = state
                    .redis_pool
                    .set_with_expiry(
                        &lockout_key,
                        locked_until.to_string(),
                        lockout_duration as usize,
                    )
                    .await;

                // Create audit entry for lockout
                let audit = create_auth_audit_entry(
                    AuthEventType::AccountLocked,
                    Some(&user.id.to_string()),
                    &email,
                    &ip_address,
                    user_agent.as_deref(),
                    Some(serde_json::json!({
                        "failed_attempts": failed_attempts,
                        "lockout_duration": lockout_duration
                    })),
                );

                // Log account lockout audit event
                tracing::warn!("Account locked: {:?}", audit);

                return AuthError::AccountLocked {
                    retry_after_seconds: lockout_duration as u64,
                }
                .into_response();
            }

            log_auth_failure(
                &email,
                &ip_address,
                &AuthError::InvalidCredentials,
                user_agent.as_deref(),
            );
            return AuthError::InvalidCredentials.into_response();
        },
        Err(e) => {
            tracing::error!("Password verification error: {}", e);
            return AuthError::InternalError.into_response();
        },
    }

    // Step 9: Clear failed login attempts on successful authentication
    clear_failed_login_attempts(&state, &email).await;

    // Step 10: Generate JWT tokens with remember_me consideration
    // When remember_me is true, refresh token gets extended expiry (30 days by default)

    // Generate device fingerprint for refresh token tracking
    // Note: Login doesn't extract custom headers, so we pass an empty HeaderMap
    let empty_headers = HeaderMap::new();
    let device_fingerprint = generate_device_fingerprint(
        &user_agent,
        &addr,
        &None, // timezone
        &None, // screen resolution
        &None, // language
        &empty_headers,
    );

    // Generate access token
    let access_token = match state.jwt_service.generate_access_token(
        &user.id.to_string(),
        &email,
        &user.subscription_tier,
        vec![], // No special permissions for now
    ) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to generate access token: {}", e);
            return AuthError::TokenError(e.to_string()).into_response();
        },
    };

    // Generate refresh token with device info and remember_me option
    let refresh_token = match state
        .jwt_service
        .generate_refresh_token_with_device_and_remember(
            &user.id.to_string(),
            device_fingerprint.clone(),
            Some(ip_address.clone()),
            user_agent.clone(),
            login_req.remember_me,
        )
        .await
    {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to generate refresh token: {}", e);
            return AuthError::TokenError(e.to_string()).into_response();
        },
    };

    // Step 11: Create audit entry for successful login
    let audit = create_auth_audit_entry(
        AuthEventType::LoginSuccess,
        Some(&user.id.to_string()),
        &email,
        &ip_address,
        user_agent.as_deref(),
        Some(serde_json::json!({
            "remember_me": login_req.remember_me,
            "device_fingerprint": device_fingerprint
        })),
    );

    // Log successful login audit event
    tracing::info!("Login successful: {:?}", audit);

    // Step 12: Build and return response
    let response = AuthResponse {
        success: true,
        data: Some(LoginResponse {
            access_token,
            refresh_token: refresh_token.clone(), // Keep in JSON for mobile compatibility
            expires_in: config.jwt.access_expiry,
            token_type: "Bearer".to_string(),
            user: LoginUserInfo {
                id: user.id.to_string(),
                email: user.email,
                full_name: user.full_name,
                subscription_tier: user.subscription_tier,
                onboarding_status: user.onboarding_status,
            },
            remember_me: login_req.remember_me,
        }),
        message: "Login successful".to_string(),
    };

    // Step 13: Set refresh token as HttpOnly cookie for web clients
    // When remember_me=true: persistent cookie with 30-day expiry
    // When remember_me=false: session cookie (deleted when browser closes)
    let refresh_cookie = create_refresh_token_cookie(refresh_token, login_req.remember_me, &config);

    // Add cookie to response
    let updated_jar = jar.add(refresh_cookie);

    (StatusCode::OK, updated_jar, Json(response)).into_response()
}

// Helper function to check if an account is locked
async fn check_account_lockout_status(state: &AppState, email: &str) -> Option<u64> {
    let lockout_key = format!("lockout:{}", email);
    match state.redis_pool.get::<String>(&lockout_key).await {
        Ok(Some(locked_until)) => {
            if let Ok(locked_until_ts) = locked_until.parse::<i64>() {
                let now = chrono::Utc::now().timestamp();
                if locked_until_ts > now {
                    return Some((locked_until_ts - now) as u64);
                }
            }
        },
        Ok(None) => {},
        Err(e) => {
            tracing::warn!("Failed to check lockout status for {}: {}", email, e);
        },
    }
    None
}

// Helper function to track failed login attempts for existing users
async fn track_failed_login(state: &AppState, email: &str, ip: &str) {
    let config = crate::app_config::config();

    let fail_key = format!("login:failed:{}", email);
    let _ = state
        .redis_pool
        .incr(&fail_key, config.security.failed_login_expiry_seconds)
        .await;

    let ip_fail_key = format!("login:failed:ip:{}", ip);
    let _ = state
        .redis_pool
        .incr(&ip_fail_key, config.security.failed_login_ip_expiry_seconds)
        .await;
}

// Helper function to track failed login attempts for non-existent users (IP only)
// This prevents Redis memory exhaustion from attackers using random email addresses
async fn track_failed_login_by_ip_only(state: &AppState, ip: &str) {
    let config = crate::app_config::config();

    let ip_fail_key = format!("login:failed:ip:{}", ip);
    let _ = state
        .redis_pool
        .incr(&ip_fail_key, config.security.failed_login_ip_expiry_seconds)
        .await;
}

// Helper function to get failed login count
async fn get_failed_login_count(state: &AppState, email: &str) -> u32 {
    let fail_key = format!("login:failed:{}", email);
    state
        .redis_pool
        .get::<String>(&fail_key)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
}

// Helper function to clear failed login attempts
async fn clear_failed_login_attempts(state: &AppState, email: &str) {
    let fail_key = format!("login:failed:{}", email);
    let _ = state.redis_pool.del(&fail_key).await;

    let lockout_key = format!("lockout:{}", email);
    let _ = state.redis_pool.del(&lockout_key).await;
}

/// POST /auth/register - Register a new user account
/// DEV-101: User Registration with Argon2 password hashing and email verification
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    _user_agent: Option<TypedHeader<UserAgent>>,
    Json(register_req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Step 1: Validate request
    if let Err(validation_errors) = register_req.validate() {
        let error_messages: Vec<String> = validation_errors
            .field_errors()
            .into_iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |e| {
                    let message = e
                        .message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| e.code.to_string());
                    format!("{}: {}", field, message)
                })
            })
            .collect();

        let response = AuthResponse::<RegisterResponse> {
            success: false,
            data: None,
            message: error_messages.join(", "),
        };
        return (StatusCode::BAD_REQUEST, Json(response)).into_response();
    }

    // Validate password confirmation matches
    if register_req.password != register_req.password_confirmation {
        let response = AuthResponse::<RegisterResponse> {
            success: false,
            data: None,
            message: "Passwords do not match".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(response)).into_response();
    }

    // Step 2: Check terms acceptance
    if !register_req.accept_terms {
        let response = AuthResponse::<RegisterResponse> {
            success: false,
            data: None,
            message: "You must accept the terms and conditions".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(response)).into_response();
    }

    // Step 3: Apply rate limiting (5 requests per minute per IP) - if enabled
    let config = crate::app_config::config();
    if config.enable_rate_limiting {
        let rate_limit_key = format!("register:{}", addr.ip());
        let rate_limit_config = RateLimitConfig {
            max_requests: 5,
            window_seconds: 60,
            burst_limit: Some(5),
            block_duration: 60,
            distributed: false, // Use local rate limiting for registration
        };

        match state
            .rate_limit_service
            .check_rate_limit_with_config(&rate_limit_key, &rate_limit_config)
            .await
        {
            Ok(status) if !status.allowed => {
                let response = AuthResponse::<RegisterResponse> {
                    success: false,
                    data: None,
                    message: format!(
                        "Too many registration attempts. Please try again in {} seconds",
                        status.retry_after.unwrap_or(60)
                    ),
                };
                return (StatusCode::TOO_MANY_REQUESTS, Json(response)).into_response();
            },
            Err(e) => {
                tracing::warn!("Rate limit check failed for registration: {}", e);
                // Continue on error - don't block registration
            },
            _ => {}, // Allowed
        }
    }

    // Step 4: Check email uniqueness
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            let response = AuthResponse::<RegisterResponse> {
                success: false,
                data: None,
                message: "Database connection error".to_string(),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
        },
    };

    // Check if email already exists (case-insensitive)
    match User::find_by_email(&mut conn, &register_req.email).await {
        Ok(_existing_user) => {
            // Email already exists
            let response = AuthResponse::<RegisterResponse> {
                success: false,
                data: None,
                message: "An account with this email address already exists".to_string(),
            };
            return (StatusCode::CONFLICT, Json(response)).into_response();
        },
        Err(UserError::NotFound) => {
            // Good, email doesn't exist
        },
        Err(e) => {
            tracing::error!("Error checking email uniqueness: {}", e);
            let response = AuthResponse::<RegisterResponse> {
                success: false,
                data: None,
                message: "Failed to check email availability".to_string(),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
        },
    }

    // Step 5: Hash the password using Argon2
    let password_hash = match hash_password(&register_req.password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            let response = AuthResponse::<RegisterResponse> {
                success: false,
                data: None,
                message: "Failed to process password".to_string(),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
        },
    };

    // Validate and trim full_name
    let full_name = match trim_and_validate_field(&register_req.full_name, true) {
        Ok(name) => name,
        Err(_) => {
            let response = AuthResponse::<RegisterResponse> {
                success: false,
                data: None,
                message: "Full name cannot be empty".to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(response)).into_response();
        },
    };

    // Trim company_name if provided
    let company_name = trim_optional_field(register_req.company_name.as_ref());

    // Step 6: Create new user in database
    let config = crate::app_config::config();
    let new_user = NewUser {
        email: register_req.email.to_lowercase(),
        password_hash,
        email_verified: if config.is_oss_deployment { true } else { false },  // OSS: auto-verify
        subscription_tier: if config.is_oss_deployment {
            "free".to_string()  // OSS: free tier, limits not enforced
        } else {
            "pending".to_string()    // Requires plan selection
        },
        full_name,
        company_name,
        onboarding_status: if config.is_oss_deployment {
            OnboardingStatus::Completed.as_str().to_string()  // OSS: skip onboarding
        } else {
            OnboardingStatus::Registered.as_str().to_string() // Requires onboarding flow
        },
    };

    let created_user = match User::create(&mut conn, new_user).await {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            let response = AuthResponse::<RegisterResponse> {
                success: false,
                data: None,
                message: "Failed to create user account".to_string(),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
        },
    };

    // Step 7: Email verification - OSS auto-verifies, no emails sent
    let verification_sent = false; // Always false for OSS

    // Step 8: Create successful response
    let register_response = RegisterResponse {
        user_id: created_user.id.to_string(),
        email: created_user.email.clone(),
        full_name: created_user.full_name.clone(),
        company_name: created_user.company_name.clone(),
        email_verification_required: crate::app_config::config()
            .security
            .require_email_verification,
        verification_sent,
        onboarding_status: created_user.onboarding_status.clone(),
        message: if verification_sent {
            "Registration successful! Please check your email for a 6-digit verification code."
                .to_string()
        } else if crate::app_config::config()
            .security
            .require_email_verification
        {
            "Registration successful! Verification email will be sent shortly.".to_string()
        } else {
            "Registration successful! You can now log in.".to_string()
        },
    };

    let response = AuthResponse {
        success: true,
        data: Some(register_response),
        message: "User registered successfully".to_string(),
    };

    tracing::info!("New user registered: {}", created_user.email);
    (StatusCode::CREATED, Json(response)).into_response()
}

/// POST /auth/refresh - Refresh access token using refresh token with rotation
/// DEV-94/DEV-107: Implements secure token refresh with rotation, device tracking, and rate limiting
/// Supports both cookie-based (web) and JSON-based (mobile) refresh tokens
pub async fn refresh_token(
    State(state): State<AppState>,
    user_agent: Option<TypedHeader<UserAgent>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // Extract device information from request
    let user_agent = user_agent.map(|TypedHeader(ua)| ua.to_string());

    let ip_address = Some(addr.ip().to_string());

    // Extract additional client characteristics from custom headers (if provided)
    let client_timezone = headers
        .get("x-client-timezone")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let client_screen_res = headers
        .get("x-client-screen-resolution")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let client_language = headers
        .get("x-client-language")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // Generate device fingerprint using utility function
    let device_fingerprint = generate_device_fingerprint(
        &user_agent,
        &addr,
        &client_timezone,
        &client_screen_res,
        &client_language,
        &headers,
    );

    // Extract refresh token from cookie (web) or JSON body (mobile)
    let refresh_token = match extract_refresh_token(&jar, &body) {
        Ok(token) => token,
        Err(response) => return response,
    };

    // Apply rate limiting for refresh endpoint (stricter than normal endpoints) - if enabled
    // Use centralized configuration method
    let config = &crate::app_config::CONFIG;
    if config.enable_rate_limiting {
        let rate_limit_key = format!("refresh:{}", addr.ip());
        let refresh_limit = config.get_refresh_rate_limit_config();

        match state
            .rate_limit_service
            .check_rate_limit_with_config(&rate_limit_key, &refresh_limit)
            .await
        {
            Ok(status) if !status.allowed => {
                let response = AuthResponse::<TokenResponse> {
                    success: false,
                    data: None,
                    message: format!(
                        "Rate limit exceeded. Try again in {} seconds",
                        status.retry_after.unwrap_or(60)
                    ),
                };
                return (StatusCode::TOO_MANY_REQUESTS, Json(response)).into_response();
            },
            Err(_) => {
                // Log but don't block on rate limit errors
                tracing::warn!("Rate limit check failed for refresh endpoint");
            },
            _ => {}, // Allowed, continue
        }
    }

    // Use the new rotation method with device information
    match state
        .jwt_service
        .rotate_refresh_token(
            &refresh_token,
            device_fingerprint,
            ip_address,
            user_agent,
        )
        .await
    {
        Ok((new_access_token, new_refresh_token, remember_me)) => {
            let token_response = TokenResponse {
                access_token: new_access_token,
                refresh_token: new_refresh_token.clone(), // Keep in JSON for mobile
                expires_in: 3600, // 1 hour per Linear DEV-113
                token_type: "Bearer".to_string(),
            };

            let response = AuthResponse {
                success: true,
                data: Some(token_response),
                message: "Token refreshed successfully".to_string(),
            };

            // Set new refresh token as HttpOnly cookie for web clients
            // Use the remember_me flag returned from rotate_refresh_token
            let config = crate::app_config::config();
            let refresh_cookie = create_refresh_token_cookie(new_refresh_token, remember_me, &config);

            // Add cookie to response
            let updated_jar = jar.add(refresh_cookie);

            (StatusCode::OK, updated_jar, Json(response)).into_response()
        },
        Err(e) => {
            let (status_code, message) = match e {
                JwtError::TokenExpired => (StatusCode::UNAUTHORIZED, "Refresh token expired"),
                JwtError::TokenRevoked => (StatusCode::UNAUTHORIZED, "Refresh token revoked"),
                JwtError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid refresh token"),
                JwtError::TokenReuseDetected => {
                    // Security breach - token reuse detected
                    (
                        StatusCode::FORBIDDEN,
                        "Security breach detected - all tokens revoked",
                    )
                },
                JwtError::SuspiciousActivity => (
                    StatusCode::FORBIDDEN,
                    "Suspicious activity detected - please login again",
                ),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Token refresh failed"),
            };

            let response = AuthResponse::<TokenResponse> {
                success: false,
                data: None,
                message: message.to_string(),
            };
            (status_code, Json(response)).into_response()
        },
    }
}

/// POST /auth/logout - Invalidate tokens and logout user
/// Clears refresh token cookie for web clients
pub async fn logout(
    Extension(user): Extension<AuthenticatedUser>,
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    // Calculate actual remaining TTL from token's expiration time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let remaining_ttl = user.exp.saturating_sub(now);

    // Blacklist the current access token by its JTI
    match state
        .jwt_service
        .logout_token(&user.token_id, remaining_ttl)
        .await
    {
        Ok(_) => {
            // Also revoke all user's refresh tokens (best effort)
            match state
                .jwt_service
                .revoke_all_user_tokens(&user.user_id)
                .await
            {
                Ok(revoked_count) => {
                    let response = AuthResponse::<()> {
                        success: true,
                        data: None,
                        message: format!(
                            "Logout successful. Current token blacklisted, {} refresh tokens revoked",
                            revoked_count
                        ),
                    };

                    // Clear refresh token cookie for web clients
                    let config = crate::app_config::config();
                    let delete_cookie = create_delete_refresh_cookie(&config);
                    let updated_jar = jar.add(delete_cookie);

                    (StatusCode::OK, updated_jar, Json(response)).into_response()
                },
                Err(e) => {
                    eprintln!("Warning: Failed to revoke all user tokens: {}", e);
                    let response = AuthResponse::<()> {
                        success: true,
                        data: None,
                        message: "Logout successful (current token blacklisted)".to_string(),
                    };

                    // Clear refresh token cookie for web clients even if revocation failed
                    let config = crate::app_config::config();
                    let delete_cookie = create_delete_refresh_cookie(&config);
                    let updated_jar = jar.add(delete_cookie);

                    (StatusCode::OK, updated_jar, Json(response)).into_response()
                },
            }
        },
        Err(e) => {
            let response = AuthResponse::<()> {
                success: false,
                data: None,
                message: format!("Logout failed: {}", e),
            };

            // Still try to clear the cookie even if logout failed
            let config = crate::app_config::config();
            let delete_cookie = create_delete_refresh_cookie(&config);
            let updated_jar = jar.add(delete_cookie);

            (StatusCode::INTERNAL_SERVER_ERROR, updated_jar, Json(response)).into_response()
        },
    }
}

/// GET /auth/me - Get current user information
pub async fn get_current_user(
    Extension(user): Extension<AuthenticatedUser>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Fetch full user info from database
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            let response = AuthResponse::<UserInfo> {
                success: false,
                data: None,
                message: "Database connection error".to_string(),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
        },
    };

    // Get user from database to fetch full_name and onboarding_status
    match User::find_by_email(&mut conn, &user.email).await {
        Ok(db_user) => {
            let user_info = UserInfo {
                user_id: user.user_id,
                email: user.email,
                full_name: db_user.full_name,
                subscription_tier: user.subscription_tier,
                onboarding_status: db_user.onboarding_status,
                permissions: user.permissions,
            };

            let response = AuthResponse {
                success: true,
                data: Some(user_info),
                message: "User info retrieved successfully".to_string(),
            };
            (StatusCode::OK, Json(response)).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to fetch user from database: {}", e);
            let response = AuthResponse::<UserInfo> {
                success: false,
                data: None,
                message: "Failed to fetch user information".to_string(),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        },
    }
}

/// POST /auth/validate - Validate current access token (for client-side checks)
pub async fn validate_token(Extension(user): Extension<AuthenticatedUser>) -> impl IntoResponse {
    let response = AuthResponse {
        success: true,
        data: Some(serde_json::json!({
            "valid": true,
            "user_id": user.user_id,
            "subscription_tier": user.subscription_tier
        })),
        message: "Token is valid".to_string(),
    };
    Json(response)
}

// =============================================================================
// EMAIL VERIFICATION ENDPOINTS (DEV-103)
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct VerifyEmailRequest {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResendVerificationRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct VerificationStatusResponse {
    pub email_verified: bool,
    pub resend_allowed: bool,
    pub resend_cooldown_seconds: u64,
    pub message: String,
}

/// POST /auth/verify-email - Verify email with 6-digit code
/// POST /auth/forgot-password
pub async fn forgot_password(
    State(app_state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    user_agent: Option<TypedHeader<UserAgent>>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> impl IntoResponse {
    // Validate input
    if let Err(validation_errors) = payload.validate() {
        let error_msg = validation_errors
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                format!(
                    "{}: {}",
                    field,
                    errors[0]
                        .message
                        .as_ref()
                        .unwrap_or(&"Invalid input".into())
                )
            })
            .collect::<Vec<String>>()
            .join(", ");

        return (
            StatusCode::BAD_REQUEST,
            Json(ForgotPasswordResponse {
                success: false,
                message: format!("Validation error: {}", error_msg),
                data: None,
            }),
        )
            .into_response();
    }

    let email = match trim_and_validate_field(&payload.email, true) {
        Ok(email) => email.to_lowercase(),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ForgotPasswordResponse {
                    success: false,
                    message: format!("Validation error: {}", e),
                    data: None,
                }),
            )
                .into_response();
        },
    };
    let client_ip = addr.ip();
    let user_agent_str = user_agent.map(|ua| ua.as_str().to_string());

    // Rate limiting check (3 requests per hour) - if enabled
    let config = crate::app_config::config();
    if config.enable_rate_limiting {
        let rate_limit_key = format!("forgot_password:{}", client_ip);
        let rate_limit_result = app_state
            .rate_limit_service
            .check_rate_limit(&rate_limit_key, "forgot_password")
            .await;

        match rate_limit_result {
            Ok(result) => {
                if !result.allowed {
                    tracing::warn!(
                        "Rate limit exceeded for forgot password from IP: {}",
                        client_ip
                    );
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(ForgotPasswordResponse {
                            success: false,
                            message: "Too many password reset attempts. Please try again later."
                                .to_string(),
                            data: None,
                        }),
                    )
                        .into_response();
                }
            },
            Err(e) => {
                tracing::error!("Rate limiting service error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ForgotPasswordResponse {
                        success: false,
                        message: "Service temporarily unavailable".to_string(),
                        data: None,
                    }),
                )
                    .into_response();
            },
        }
    }

    // Use existing password reset service from app_state
    let password_reset_service = &app_state.password_reset_service;

    // Check for too many recent attempts for this email
    match password_reset_service
        .check_recent_attempts(&email, 1)
        .await
    {
        Ok(count) if count >= 3 => {
            tracing::warn!(
                "Too many recent password reset attempts for email: {}",
                email
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ForgotPasswordResponse {
                    success: false,
                    message:
                        "Too many password reset attempts for this account. Please try again later."
                            .to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
        Ok(_) => {}, // Continue
        Err(e) => {
            tracing::error!("Failed to check recent attempts: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ForgotPasswordResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    }

    // Create password reset request
    match password_reset_service
        .create_reset_request(&email, Some(client_ip), user_agent_str)
        .await
    {
        Ok(token_info_opt) => {
            // Always return success message to avoid email enumeration
            let response = ForgotPasswordResponse {
                success: true,
                message:
                    "If an account with that email exists, a password reset link has been sent."
                        .to_string(),
                data: None,
            };

            // If we have a token, send the email (but don't change response)
            if let Some(token_info) = token_info_opt {
                // Get user's name for personalized email
                let user_name = match password_reset_service.get_user_name_by_email(&email).await {
                    Ok(name) => name.unwrap_or_else(|| "User".to_string()),
                    Err(e) => {
                        tracing::warn!("Could not retrieve user name for email {}: {}", email, e);
                        "User".to_string()
                    },
                };

                // Send password reset email
                if let Err(e) = app_state
                    .email_service
                    .send_password_reset_email(&email, &user_name, &token_info.token)
                    .await
                {
                    tracing::error!("Failed to send password reset email to {}: {}", email, e);
                    // Don't return error to prevent email enumeration - continue with success response
                } else {
                    tracing::info!("Password reset email sent successfully to {}", email);
                }

                tracing::info!(
                    "Password reset token generated for email: {} (expires: {})",
                    email,
                    token_info.expires_at
                );
            }

            (StatusCode::OK, Json(response)).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to create password reset request: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ForgotPasswordResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response()
        },
    }
}

/// Handle password reset with token
/// POST /auth/reset-password
pub async fn reset_password(
    State(app_state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    // Validate input
    if let Err(validation_errors) = payload.validate() {
        let error_msg = validation_errors
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                format!(
                    "{}: {}",
                    field,
                    errors[0]
                        .message
                        .as_ref()
                        .unwrap_or(&"Invalid input".into())
                )
            })
            .collect::<Vec<String>>()
            .join(", ");

        return (
            StatusCode::BAD_REQUEST,
            Json(ResetPasswordResponse {
                success: false,
                message: format!("Validation error: {}", error_msg),
                data: None,
            }),
        )
            .into_response();
    }

    // Validate that passwords match
    if let Err(e) = payload.validate_passwords_match() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResetPasswordResponse {
                success: false,
                message: format!("Validation error: {}", e),
                data: None,
            }),
        )
            .into_response();
    }

    let client_ip = addr.ip();

    // Rate limiting for reset attempts - if enabled
    let config = crate::app_config::config();
    if config.enable_rate_limiting {
        let rate_limit_key = format!("reset_password:{}", client_ip);
        let rate_limit_result = app_state
            .rate_limit_service
            .check_rate_limit(&rate_limit_key, "reset_password") // 5 attempts per hour
            .await;

        match rate_limit_result {
            Ok(result) => {
                if !result.allowed {
                    tracing::warn!(
                        "Rate limit exceeded for password reset from IP: {}",
                        client_ip
                    );
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(ResetPasswordResponse {
                            success: false,
                            message: "Too many password reset attempts. Please try again later."
                                .to_string(),
                            data: None,
                        }),
                    )
                        .into_response();
                }
            },
            Err(e) => {
                tracing::error!("Rate limiting service error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ResetPasswordResponse {
                        success: false,
                        message: "Service temporarily unavailable".to_string(),
                        data: None,
                    }),
                )
                    .into_response();
            },
        }
    }

    // Use shared password reset service from app_state
    let password_reset_service = &app_state.password_reset_service;

    // Validate and consume the reset token
    let user_id = match password_reset_service
        .validate_and_consume_token(&payload.token)
        .await
    {
        Ok(user_id) => user_id,
        Err(AuthError::InvalidToken) => {
            tracing::warn!(
                "Invalid or expired password reset token from IP: {}",
                client_ip
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(ResetPasswordResponse {
                    success: false,
                    message: "Invalid or expired reset token".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
        Err(e) => {
            tracing::error!("Failed to validate reset token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    // Validate password strength (reuse existing validation)
    if let Err(e) = validate_password(&payload.new_password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResetPasswordResponse {
                success: false,
                message: format!("Password validation failed: {}", e),
                data: None,
            }),
        )
            .into_response();
    }

    // Hash the new password
    let password_hash = match hash_password(&payload.new_password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    // Update user's password
    use crate::schema::users;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    let mut conn = match app_state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    match diesel::update(users::table.find(user_id))
        .set(users::password_hash.eq(&password_hash))
        .execute(&mut conn)
        .await
    {
        Ok(_) => {
            tracing::info!("Password successfully reset for user: {}", user_id);

            // Invalidate all existing refresh tokens for this user
            // This ensures that any compromised tokens cannot be used after password reset
            if let Err(e) = app_state
                .jwt_service
                .revoke_all_user_tokens(&user_id.to_string())
                .await
            {
                tracing::warn!(
                    "Failed to revoke existing tokens for user {}: {}",
                    user_id,
                    e
                );
                // Continue despite token revocation failure - password was still reset successfully
            }

            // Send security notification about password change
            // Get user details for the email
            let user = match users::table
                .find(user_id)
                .select(users::all_columns)
                .get_result::<crate::models::user::User>(&mut conn)
                .await
            {
                Ok(user) => user,
                Err(e) => {
                    tracing::warn!(
                        "Failed to get user details for password change notification: {}",
                        e
                    );
                    // Return success even if email notification fails
                    return (
                        StatusCode::OK,
                        Json(ResetPasswordResponse {
                            success: true,
                            message: "Password has been successfully reset. You can now log in with your new password.".to_string(),
                            data: None,
                        }),
                    ).into_response();
                },
            };

            // Send password change confirmation email
            let ip = addr.ip().to_string();
            // Extract user agent from headers, fallback to "Password Reset" if not present
            let user_agent = headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|ua| ua.to_str().ok())
                .unwrap_or("Password Reset");
            if let Err(e) = app_state
                .email_service
                .send_password_change_notification(&user.email, &user.full_name, &ip, user_agent)
                .await
            {
                tracing::warn!("Failed to send password change notification: {}", e);
                // Continue - password was still reset successfully
            }

            (
                StatusCode::OK,
                Json(ResetPasswordResponse {
                    success: true,
                    message: "Password has been successfully reset. You can now log in with your new password.".to_string(),
                    data: None,
                }),
            ).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to update user password: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response()
        },
    }
}

// HELPER FUNCTIONS
// =============================================================================

// Helper functions moved to where they're needed (test module)

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use axum::body::Bytes;
    use axum_extra::extract::cookie::CookieJar;
    use serde_json::json;

    // Test-only conversion trait - not available in production code
    impl From<AuthenticatedUser> for UserInfo {
        /// This trait implementation is only available in tests.
        /// For production endpoints, use get_current_user() which fetches full_name
        /// and onboarding_status from the database.
        fn from(user: AuthenticatedUser) -> Self {
            Self {
                user_id: user.user_id,
                email: user.email,
                full_name: String::new(), // Empty placeholder (test-only)
                subscription_tier: user.subscription_tier,
                onboarding_status: String::new(), // Empty placeholder (test-only)
                permissions: user.permissions,
            }
        }
    }

    #[tokio::test]
    async fn test_extract_refresh_token_from_cookie() {
        let jar = CookieJar::new();
        let jar_with_cookie = jar.add(("refresh_token", "header.payload.signature"));
        let body = Bytes::new();

        let result = extract_refresh_token(&jar_with_cookie, &body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "header.payload.signature");
    }

    #[tokio::test]
    async fn test_extract_refresh_token_from_json_body() {
        let jar = CookieJar::new();
        let token_json = json!({"refresh_token": "mobile.jwt.token"});
        let body = Bytes::from(token_json.to_string());

        let result = extract_refresh_token(&jar, &body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "mobile.jwt.token");
    }

    #[tokio::test]
    async fn test_extract_refresh_token_empty_body() {
        let jar = CookieJar::new();
        let body = Bytes::new();

        let result = extract_refresh_token(&jar, &body);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_refresh_token_invalid_json() {
        let jar = CookieJar::new();
        let body = Bytes::from("{invalid json");

        let result = extract_refresh_token(&jar, &body);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_refresh_token_json_missing_token() {
        let jar = CookieJar::new();
        let token_json = json!({"other_field": "value"});
        let body = Bytes::from(token_json.to_string());

        let result = extract_refresh_token(&jar, &body);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_refresh_token_cookie_priority_over_json() {
        let jar = CookieJar::new();
        let jar_with_cookie = jar.add(("refresh_token", "cookie.jwt.token"));
        let token_json = json!({"refresh_token": "json.jwt.token"});
        let body = Bytes::from(token_json.to_string());

        let result = extract_refresh_token(&jar_with_cookie, &body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "cookie.jwt.token"); // Cookie should take priority
    }

    #[tokio::test]
    async fn test_extract_refresh_token_invalid_format_cookie() {
        let jar = CookieJar::new();
        // Invalid token with only 2 parts
        let jar_with_cookie = jar.add(("refresh_token", "invalid.token"));
        let body = Bytes::new();

        let result = extract_refresh_token(&jar_with_cookie, &body);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_refresh_token_invalid_format_json() {
        let jar = CookieJar::new();
        // Invalid token with only 1 part
        let token_json = json!({"refresh_token": "invalidtoken"});
        let body = Bytes::from(token_json.to_string());

        let result = extract_refresh_token(&jar, &body);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_refresh_token_too_many_parts() {
        let jar = CookieJar::new();
        // Invalid token with 4 parts
        let jar_with_cookie = jar.add(("refresh_token", "too.many.parts.here"));
        let body = Bytes::new();

        let result = extract_refresh_token(&jar_with_cookie, &body);
        assert!(result.is_err());
    }

    /// Mock user structure for testing purposes
    #[derive(Debug)]
    struct MockUser {
        id: String,
        email: String,
        subscription_tier: String,
    }

    /// Create mock user for testing
    fn create_mock_user_for_demo(email: &str) -> MockUser {
        let tier = if email.contains("admin") {
            "enterprise"
        } else if email.contains("premium") {
            "premium"
        } else if email.contains("basic") {
            "basic"
        } else {
            "free"
        };

        MockUser {
            id: Uuid::new_v4().to_string(),
            email: email.to_string(),
            subscription_tier: tier.to_string(),
        }
    }

    #[test]
    fn test_mock_user_creation() {
        let admin_user = create_mock_user_for_demo("admin@example.com");
        assert_eq!(admin_user.subscription_tier, "enterprise");
        assert_eq!(admin_user.email, "admin@example.com");

        let premium_user = create_mock_user_for_demo("premium@example.com");
        assert_eq!(premium_user.subscription_tier, "premium");

        let basic_user = create_mock_user_for_demo("basic@example.com");
        assert_eq!(basic_user.subscription_tier, "basic");

        let free_user = create_mock_user_for_demo("user@example.com");
        assert_eq!(free_user.subscription_tier, "free");
    }

    #[test]
    fn test_user_info_conversion() {
        // Use test expiry time to avoid CONFIG dependency
        let expiry = 3600u64; // 1 hour test expiry

        let future_exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + expiry;

        let auth_user = AuthenticatedUser {
            user_id: "123".to_string(),
            token_id: "test-jti".to_string(),
            email: "test@example.com".to_string(),
            subscription_tier: "pro".to_string(),
            permissions: vec!["pro".to_string(), "personal".to_string()],
            exp: future_exp,
        };

        let user_info = UserInfo::from(auth_user);
        assert_eq!(user_info.user_id, "123");
        assert_eq!(user_info.email, "test@example.com");
        assert_eq!(user_info.subscription_tier, "pro");
        assert_eq!(user_info.permissions.len(), 2);
    }
}
