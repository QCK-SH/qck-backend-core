// Production handlers only - test endpoints removed
// DEV-113: JWT Token Validation handlers
// DEV-102: Login API endpoint
// DEV-68: Link Management API handlers
// DEV-105: Link management handlers

pub mod auth;
pub mod docs; // Modular documentation structure
pub mod links;
pub mod onboarding;
pub mod redirect;

use crate::app::AppState;
use axum::{
    routing::{get, post},
    Router,
};

// Authentication routes
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/refresh", post(auth::refresh_token))
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::get_current_user))
        .route("/validate", post(auth::validate_token))
        // Email verification endpoints (DEV-103)
        .route("/verify-email", post(auth::verify_email))
        .route("/resend-verification", post(auth::resend_verification))
        .route("/verification-status", get(auth::verification_status))
        // Password reset endpoints (DEV-106)
        .route("/forgot-password", post(auth::forgot_password))
        .route("/reset-password", post(auth::reset_password))
}

// Onboarding routes
pub fn onboarding_routes() -> Router<AppState> {
    Router::new()
        .route("/select-plan", post(onboarding::select_plan))
        .route("/status", get(onboarding::get_status))
}
