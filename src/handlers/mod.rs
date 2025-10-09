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

// Authentication routes (public - no auth required)
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/refresh", post(auth::refresh_token))
        .route("/forgot-password", post(auth::forgot_password))
        .route("/reset-password", post(auth::reset_password))
}
