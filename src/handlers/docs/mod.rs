// API Documentation handlers - modular structure
pub mod auth;
pub mod health;
pub mod links;
pub mod redirect;
pub mod schemas;
pub mod swagger_ui;

use crate::app::AppState;
use crate::app_config::AppConfig;
use axum::{
    extract::{OriginalUri, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{self, json};

/// Serve OpenAPI JSON specification at /v1/docs/openapi.json
pub async fn serve_openapi_spec(State(app_state): State<AppState>) -> Response {
    let spec = build_openapi_spec(app_state.config.as_ref());

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
/// Made public to allow cloud backend to extend with cloud-specific endpoints
pub fn build_openapi_spec(config: &AppConfig) -> serde_json::Value {
    // Determine the API base URL from environment
    let api_url = std::env::var("NEXT_PUBLIC_API_URL").unwrap_or_else(|_| {
        // Fallback based on environment
        match config.environment {
            crate::app_config::Environment::Production => "https://qck.sh/api".to_string(),
            crate::app_config::Environment::Staging => "https://s.qck.sh/api".to_string(),
            _ => format!("http://localhost:{}/api", config.server.api_port),
        }
    });

    // Build server list based on environment
    let mut servers = vec![json!({
        "url": api_url,
        "description": format!("Current server ({})", config.environment)
    })];

    // Add additional servers for reference in non-production environments
    if !config.is_production() {
        servers.extend(vec![
            json!({
                "url": "https://s.qck.sh/api",
                "description": "Staging server"
            }),
            json!({
                "url": "https://qck.sh/api",
                "description": "Production server"
            }),
        ]);
    }

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
        "servers": servers,
        "tags": [
            {
                "name": "Authentication",
                "description": "User authentication and registration (OSS - Auto-verification enabled)"
            },
            {
                "name": "Links",
                "description": "URL shortening and link management operations"
            },
            {
                "name": "Redirect",
                "description": "URL redirection and preview endpoints"
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
            "/v1/auth/logout": auth::logout_endpoint(),
            "/v1/auth/me": auth::get_current_user_endpoint(),
            "/v1/auth/validate": auth::validate_token_endpoint(),
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
            "/{short_code}": redirect::redirect_endpoint(),
            "/{short_code}/preview": redirect::preview_endpoint(),
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
/// Made public to allow cloud backend to extend with cloud-specific schemas
pub fn merge_schemas() -> serde_json::Value {
    let mut all_schemas = schemas::all_schemas();

    // Merge link-specific schemas
    if let serde_json::Value::Object(ref mut map) = all_schemas {
        if let serde_json::Value::Object(link_schemas_map) = links::link_schemas() {
            for (key, value) in link_schemas_map {
                map.insert(key, value);
            }
        }

        // Merge redirect-specific schemas
        if let serde_json::Value::Object(redirect_schemas_map) = redirect::preview_response_schema() {
            for (key, value) in redirect_schemas_map {
                map.insert(key, value);
            }
        }
    }

    all_schemas
}
