// Authentication endpoints OpenAPI documentation

use serde_json::json;

/// Register endpoint documentation
pub fn register_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Register a new user account",
            "description": "Creates a new user account with email and password authentication.\n\n**Password Requirements:**\n- Minimum 8 characters\n- At least one uppercase letter\n- At least one lowercase letter\n- At least one number\n- At least one special character\n\n**Rate Limiting:** 5 requests per minute per IP",
            "operationId": "registerUser",
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/RegisterRequest"
                        },
                        "example": {
                            "email": "user@example.com",
                            "password": "SecureP@ssw0rd123!",
                            "password_confirmation": "SecureP@ssw0rd123!",
                            "full_name": "John Doe",
                            "company_name": "Acme Corp",
                            "accept_terms": true
                        }
                    }
                }
            },
            "responses": {
                "201": {
                    "description": "User successfully registered",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/RegisterResponse"
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad Request - Validation failed"
                },
                "409": {
                    "description": "Conflict - Email already exists"
                },
                "429": {
                    "description": "Too Many Requests"
                }
            }
        }
    })
}

/// Login endpoint documentation
pub fn login_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Authenticate user and get JWT tokens",
            "description": "Login with email and password to receive access and refresh tokens.\n\n**Security Features:**\n- IP-based rate limiting (configurable per environment)\n- Email-based rate limiting to prevent brute force\n- Account lockout after failed attempts\n- Optional remember_me for extended token expiry\n- Device fingerprinting for security tracking\n\n**Rate Limiting:**\n- Development: 10 attempts/IP/minute, 20 attempts/email/hour\n- Staging: 7 attempts/IP/minute, 15 attempts/email/hour\n- Production: 5 attempts/IP/minute, 10 attempts/email/hour\n\n**Account Lockout:**\n- Development: 10 failed attempts = 5 minute lockout\n- Staging: 7 failed attempts = 15 minute lockout\n- Production: 5 failed attempts = 30 minute lockout",
            "operationId": "loginUser",
            "parameters": [
                {
                    "name": "User-Agent",
                    "in": "header",
                    "description": "Browser/client user agent string for device fingerprinting and audit logging",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "example": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
                    }
                }
            ],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/LoginRequest"
                        },
                        "example": {
                            "email": "user@example.com",
                            "password": "SecureP@ssw0rd123!",
                            "remember_me": false
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Login successful",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LoginResponse"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - Invalid credentials",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "403": {
                    "description": "Forbidden - Email not verified or account inactive",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "423": {
                    "description": "Locked - Account locked due to too many failed attempts",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too Many Requests - Rate limit exceeded",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "500": {
                    "description": "Internal Server Error"
                }
            }
        }
    })
}

/// Refresh token endpoint documentation
pub fn refresh_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Refresh access token using refresh token",
            "description": "Exchange a valid refresh token for a new access token and optionally a new refresh token.\n\n**Token Rotation:**\n- Each refresh token can only be used once\n- A new refresh token is issued with each refresh (rotation)\n- Old refresh tokens are automatically revoked\n- Token family tracking detects token reuse attacks\n\n**Security Features:**\n- Automatic token rotation for enhanced security\n- Device fingerprinting validation using headers\n- Token family tracking to detect stolen tokens\n- Automatic revocation of compromised token families\n\n**Headers for Enhanced Security:**\n- `User-Agent` (standard): Browser/client identification for device fingerprinting\n- `x-client-timezone` (optional): Client timezone for enhanced fingerprinting\n- `x-client-screen-resolution` (optional): Screen resolution for device tracking\n- `x-client-language` (optional): Client language preference\n\n**Rate Limiting:**\n- 10 requests per minute per token family",
            "operationId": "refreshToken",
            "parameters": [
                {
                    "name": "User-Agent",
                    "in": "header",
                    "description": "Browser/client user agent string for device fingerprinting",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "example": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
                    }
                },
                {
                    "name": "x-client-timezone",
                    "in": "header",
                    "description": "Client timezone for enhanced device fingerprinting (optional)",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "example": "America/New_York"
                    }
                },
                {
                    "name": "x-client-screen-resolution",
                    "in": "header",
                    "description": "Client screen resolution for device tracking (optional)",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "example": "1920x1080"
                    }
                },
                {
                    "name": "x-client-language",
                    "in": "header",
                    "description": "Client language preference (optional)",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "example": "en-US"
                    }
                }
            ],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/RefreshTokenRequest"
                        },
                        "example": {
                            "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Token refresh successful",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/RefreshTokenResponse"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - Invalid or expired refresh token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "403": {
                    "description": "Forbidden - Token has been revoked or reused",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too Many Requests - Rate limit exceeded"
                },
                "500": {
                    "description": "Internal Server Error"
                }
            }
        }
    })
}

/// Verify email endpoint documentation
pub fn verify_email_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Verify email address with 6-digit code",
            "description": "Verifies a user's email address using a 6-digit verification code sent via email.\n\n**Security Features:**\n- Code expires after 10 minutes\n- Maximum 5 verification attempts per code\n- Rate limiting to prevent brute force\n- Codes are stored securely in Redis",
            "operationId": "verifyEmail",
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/VerifyEmailRequest"
                        },
                        "example": {
                            "email": "user@example.com",
                            "code": "123456"
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Email verified successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/VerifyEmailResponse"
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad Request - Invalid code or code expired",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Not Found - User not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too Many Requests - Too many verification attempts",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Resend verification email endpoint documentation
pub fn resend_verification_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Resend email verification code",
            "description": "Resends a new 6-digit verification code to the user's email address.\n\n**Rate Limiting:**\n- Maximum 3 resends per day\n- 60-second cooldown between resends\n- Previous codes are invalidated when new code is sent",
            "operationId": "resendVerification",
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/ResendVerificationRequest"
                        },
                        "example": {
                            "email": "user@example.com"
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Verification code resent successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ResendVerificationResponse"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Not Found - User not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "409": {
                    "description": "Conflict - Email already verified",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too Many Requests - Resend limit exceeded or cooldown active",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Verification status endpoint documentation
pub fn verification_status_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Authentication"],
            "summary": "Check email verification status",
            "description": "Checks whether a user's email address has been verified.\n\n**No authentication required** - Public endpoint for checking verification status.",
            "operationId": "verificationStatus",
            "parameters": [
                {
                    "name": "email",
                    "in": "query",
                    "description": "Email address to check verification status",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "format": "email",
                        "example": "user@example.com"
                    }
                }
            ],
            "responses": {
                "200": {
                    "description": "Verification status retrieved",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/VerificationStatusResponse"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Not Found - User not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Forgot password endpoint documentation
pub fn forgot_password_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Request password reset link",
            "description": "Initiates password reset process by sending a secure reset link to the user's email.\n\n**Security Features:**\n- 256-bit entropy tokens with SHA-256 hashing\n- 15-minute token expiration for security\n- Single-use tokens that are consumed after use\n- Email enumeration prevention (always returns success)\n- IP-based rate limiting to prevent abuse\n- Audit logging with IP address and user agent\n\n**Rate Limiting:**\n- Maximum 3 requests per hour per IP address\n- Prevents automated attacks and spam\n\n**Privacy:**\n- Always returns success response regardless of email existence\n- Protects user privacy by not revealing account existence",
            "operationId": "forgotPassword",
            "parameters": [
                {
                    "name": "User-Agent",
                    "in": "header",
                    "description": "Browser/client user agent string for audit logging",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "example": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
                    }
                }
            ],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/ForgotPasswordRequest"
                        },
                        "example": {
                            "email": "user@example.com"
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Password reset email sent (or email not found - security)",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ForgotPasswordResponse"
                            },
                            "example": {
                                "success": true,
                                "message": "If an account with that email exists, we've sent a password reset link.",
                                "data": null
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad Request - Invalid email format",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too Many Requests - Rate limit exceeded",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ForgotPasswordResponse"
                            },
                            "example": {
                                "success": false,
                                "message": "Too many password reset attempts. Please try again later.",
                                "data": null
                            }
                        }
                    }
                },
                "500": {
                    "description": "Internal Server Error"
                }
            }
        }
    })
}

/// Reset password endpoint documentation
pub fn reset_password_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Reset password using token",
            "description": "Resets user password using a secure token received via email.\n\n**Security Features:**\n- Token validation with expiration checking\n- Single-use token consumption (tokens can't be reused)\n- Password strength validation\n- Password confirmation matching\n- IP-based rate limiting for reset attempts\n- Audit logging of reset activities\n\n**Password Requirements:**\n- Minimum 8 characters\n- Maximum 128 characters\n- Must match confirmation field exactly\n\n**Rate Limiting:**\n- Maximum 5 reset attempts per hour per IP address\n- Prevents brute force attacks on tokens\n\n**Token Security:**\n- Tokens expire after 15 minutes\n- Tokens are single-use only\n- Invalid/expired/used tokens return error",
            "operationId": "resetPassword",
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/ResetPasswordRequest"
                        },
                        "example": {
                            "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
                            "new_password": "NewSecureP@ssw0rd123!",
                            "confirm_password": "NewSecureP@ssw0rd123!"
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Password reset successful",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ResetPasswordResponse"
                            },
                            "example": {
                                "success": true,
                                "message": "Password has been reset successfully. You can now login with your new password.",
                                "data": null
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad Request - Invalid token, expired token, or validation errors",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ResetPasswordResponse"
                            },
                            "examples": {
                                "invalid_token": {
                                    "summary": "Invalid or expired token",
                                    "value": {
                                        "success": false,
                                        "message": "Invalid or expired reset token",
                                        "data": null
                                    }
                                },
                                "password_mismatch": {
                                    "summary": "Password confirmation mismatch",
                                    "value": {
                                        "success": false,
                                        "message": "Password confirmation does not match",
                                        "data": null
                                    }
                                },
                                "validation_error": {
                                    "summary": "Validation errors",
                                    "value": {
                                        "success": false,
                                        "message": "Validation error: Password must be between 8 and 128 characters",
                                        "data": null
                                    }
                                }
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too Many Requests - Too many reset attempts",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ResetPasswordResponse"
                            },
                            "example": {
                                "success": false,
                                "message": "Too many password reset attempts. Please try again later.",
                                "data": null
                            }
                        }
                    }
                },
                "500": {
                    "description": "Internal Server Error"
                }
            }
        }
    })
}

/// Logout endpoint documentation
pub fn logout_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Logout user",
            "description": "Logs out the current user by invalidating their tokens.",
            "operationId": "logout",
            "security": [
                {
                    "bearerAuth": []
                }
            ],
            "responses": {
                "200": {
                    "description": "Logout successful",
                    "content": {
                        "application/json": {
                            "schema": {
                                "type": "object",
                                "properties": {
                                    "success": {
                                        "type": "boolean",
                                        "example": true
                                    },
                                    "message": {
                                        "type": "string",
                                        "example": "Logged out successfully"
                                    }
                                }
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - Invalid or missing token"
                }
            }
        }
    })
}

/// Get current user endpoint documentation
pub fn get_current_user_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Authentication"],
            "summary": "Get current user",
            "description": "Returns information about the currently authenticated user.",
            "operationId": "getCurrentUser",
            "security": [
                {
                    "bearerAuth": []
                }
            ],
            "responses": {
                "200": {
                    "description": "User information retrieved successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/UserResponse"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - Invalid or missing token"
                }
            }
        }
    })
}

/// Validate token endpoint documentation
pub fn validate_token_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Authentication"],
            "summary": "Validate JWT token",
            "description": "Validates a JWT token and returns its status and claims.",
            "operationId": "validateToken",
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["token"],
                            "properties": {
                                "token": {
                                    "type": "string",
                                    "description": "JWT token to validate",
                                    "example": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
                                }
                            }
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Token is valid",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/TokenValidationResponse"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - Invalid or expired token"
                }
            }
        }
    })
}
