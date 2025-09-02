// API Documentation handlers - modular structure
pub mod auth;
pub mod health;
pub mod links;
pub mod onboarding;
pub mod schemas;
pub mod swagger_ui;

use axum::{
    extract::OriginalUri,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{self, json};

/// Serve OpenAPI JSON specification at /v1/docs/openapi.json
pub async fn serve_openapi_spec() -> Response {
    let spec = build_openapi_spec();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&spec).unwrap_or_default(),
    )
        .into_response()
}

/// Redirect /docs to /docs/ for proper relative path resolution
pub async fn redirect_to_docs(original_uri: OriginalUri) -> impl IntoResponse {
    let mut path = original_uri.0.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, path)]).into_response()
}

/// Re-export swagger UI handler
pub use swagger_ui::serve_swagger_ui;

/// Build the complete OpenAPI specification
fn build_openapi_spec() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "QCK Backend API",
            "description": "URL shortener platform API with user authentication and link management",
            "version": "1.0.0",
            "contact": {
                "name": "QCK Development Team",
                "email": "dev@qck.sh"
            }
        },
        "servers": [
            {
                "url": "http://localhost:10110",
                "description": "Development server (local)"
            },
            {
                "url": "https://s_api.qck.sh",
                "description": "Staging server"
            },
            {
                "url": "https://api.qck.sh",
                "description": "Production server"
            }
        ],
        "tags": [
            {
                "name": "Authentication",
                "description": "User authentication and registration"
            },
            {
                "name": "Links",
                "description": "URL shortening and link management operations"
            },
            {
                "name": "Onboarding",
                "description": "User onboarding flow and plan selection"
            },
            {
                "name": "Health",
                "description": "Service health checks"
            }
        ],
        "paths": {
            "/v1/auth/register": auth::register_endpoint(),
            "/v1/auth/login": auth::login_endpoint(),
            "/v1/auth/refresh": auth::refresh_endpoint(),
            "/v1/auth/verify-email": auth::verify_email_endpoint(),
            "/v1/auth/resend-verification": auth::resend_verification_endpoint(),
            "/v1/auth/verification-status": auth::verification_status_endpoint(),
            "/v1/auth/forgot-password": auth::forgot_password_endpoint(),
            "/v1/auth/reset-password": auth::reset_password_endpoint(),
            "/v1/links": json!({
                "post": links::create_link_endpoint()["post"],
                "get": links::list_links_endpoint()["get"]
            }),
            "/v1/links/bulk": json!({
                "post": links::bulk_create_links_endpoint()["post"]
            }),
            "/v1/links/{id}": json!({
                "get": links::get_link_endpoint()["get"],
                "put": links::update_link_endpoint()["put"],
                "delete": links::delete_link_endpoint()["delete"]
            }),
            "/v1/links/{id}/stats": json!({
                "get": links::get_link_stats_endpoint()["get"]
            }),
            "/v1/onboarding/select-plan": onboarding::select_plan_endpoint(),
            "/v1/onboarding/status": onboarding::onboarding_status_endpoint(),
            "/v1/health": health::health_endpoint(),
        },
        "components": {
            "schemas": merge_schemas(),
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT",
                    "description": "JWT access token obtained from login or refresh endpoints"
                }
            }
        }
    })
}

/// Merge all schemas into a single JSON object
fn merge_schemas() -> serde_json::Value {
    let mut all_schemas = schemas::all_schemas();

    // Merge link-specific schemas
    if let serde_json::Value::Object(ref mut map) = all_schemas {
        if let serde_json::Value::Object(link_schemas_map) = links::link_schemas() {
            for (key, value) in link_schemas_map {
                map.insert(key, value);
            }
        }
    }

    all_schemas
}
