// Authentication middleware for protected routes
// Validates JWT tokens and injects AuthenticatedUser into request extensions

use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{header, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::{app::AppState, middleware::auth::AuthenticatedUser};

/// Middleware function that validates JWT tokens and adds AuthenticatedUser to extensions
pub async fn auth_middleware(
    State(app_state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Extract the Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Missing or invalid authorization header"
                })),
            )
                .into_response();
        },
    };

    // Validate the token using JwtService from AppState
    match app_state.jwt_service.validate_access_token(token) {
        Ok(claims) => {
            // Create AuthenticatedUser from claims
            let auth_user = AuthenticatedUser {
                user_id: claims.sub,
                token_id: claims.jti,
                email: claims.email,
                subscription_tier: claims.tier,
                permissions: claims.scope,
                exp: claims.exp,
            };

            // Add AuthenticatedUser to request extensions
            request.extensions_mut().insert(auth_user);

            // Continue to the next handler
            next.run(request).await
        },
        Err(e) => {
            tracing::warn!("JWT validation failed: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid or expired token"
                })),
            )
                .into_response()
        },
    }
}

/// Extractor for AuthenticatedUser from request extensions
/// This allows handlers to use Extension<AuthenticatedUser> in their parameters
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "success": false,
                        "message": "Authentication required"
                    })),
                )
            })
    }
}
