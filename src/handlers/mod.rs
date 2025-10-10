// Production handlers only - test endpoints removed
// DEV-113: JWT Token Validation handlers
// DEV-102: Login API endpoint
// DEV-68: Link Management API handlers
// DEV-105: Link management handlers

pub mod auth;
pub mod docs; // Modular documentation structure
pub mod links;
pub mod redirect;

use crate::app::AppState;
use axum::{
    routing::{get, post},
    Router,
};

// Public authentication routes (no auth required)
pub fn public_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/refresh", post(auth::refresh_token))
        .route("/forgot-password", post(auth::forgot_password))
        .route("/reset-password", post(auth::reset_password))
}

// Protected authentication routes (require JWT auth middleware)
pub fn protected_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::get_current_user))
        .route("/validate", post(auth::validate_token))
}
