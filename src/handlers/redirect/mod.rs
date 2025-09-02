// DEV-68: High-performance redirect handler
// This is where the magic happens - turning short codes into destinations!

mod pages;
use pages::processing_page;

use axum::{
    extract::{ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use std::net::SocketAddr;
use std::time::Instant;
use tracing::{info, warn};

use crate::{app::AppState, services::link::LinkService, utils::service_error::ServiceError};

// =============================================================================
// REDIRECT HANDLER
// =============================================================================

/// Handle redirect for short URLs with full click tracking
/// GET /r/:short_code
pub async fn redirect_to_url(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(short_code): Path<String>,
) -> Response {
    let start_time = Instant::now();
    let link_service = LinkService::new(&state);

    // Extract request details for click tracking
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown");

    let referrer = headers.get(header::REFERER).and_then(|v| v.to_str().ok());

    let method = "GET";

    // Process the redirect
    let (_status_code, response) = match link_service.process_redirect(&short_code).await {
        Ok((link_id, original_url)) => {
            info!("Redirecting {} to {}", short_code, original_url);

            // Track click event to ClickHouse (fire-and-forget)
            let response_time = start_time.elapsed().as_millis() as u16;
            link_service.track_click_event(
                link_id,
                addr.ip(),
                user_agent,
                referrer,
                method,
                response_time,
                StatusCode::MOVED_PERMANENTLY.as_u16(),
            );

            // Use permanent redirect (301) for better SEO
            (
                StatusCode::MOVED_PERMANENTLY,
                Redirect::permanent(&original_url).into_response(),
            )
        },
        Err(ServiceError::NotFound) => {
            warn!("Short code not found: {}", short_code);
            (
                StatusCode::NOT_FOUND,
                (
                    StatusCode::NOT_FOUND,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    not_found_page(&short_code),
                )
                    .into_response(),
            )
        },
        Err(ServiceError::Expired) => {
            warn!("Link expired: {}", short_code);
            (
                StatusCode::GONE,
                (
                    StatusCode::GONE,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    expired_page(&short_code),
                )
                    .into_response(),
            )
        },
        Err(ServiceError::Inactive) => {
            warn!("Link inactive or still processing: {}", short_code);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    processing_page(&short_code),
                )
                    .into_response(),
            )
        },
        Err(ServiceError::PasswordRequired) => {
            // In production, this would redirect to a password entry page
            (
                StatusCode::UNAUTHORIZED,
                (
                    StatusCode::UNAUTHORIZED,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    password_required_page(&short_code),
                )
                    .into_response(),
            )
        },
        Err(e) => {
            warn!("Error processing redirect for {}: {:?}", short_code, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An error occurred processing your request",
                )
                    .into_response(),
            )
        },
    };

    response
}

/// Preview a short URL without redirecting
/// GET /r/:short_code/preview
pub async fn preview_url(
    State(state): State<AppState>,
    Path(short_code): Path<String>,
) -> Response {
    let link_service = LinkService::new(&state);

    match link_service.get_link_by_code(&short_code).await {
        Ok(Some(link)) => {
            // Return preview information
            let preview = serde_json::json!({
                "short_code": link.short_code,
                "original_url": link.original_url,
                "created_at": link.created_at,
                "expires_at": link.expires_at,
                "is_active": link.is_active,
            });

            axum::Json(preview).into_response()
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Link not found"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

// =============================================================================
// ERROR PAGES
// =============================================================================

fn not_found_page(short_code: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Link Not Found - QCK</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
        }}
        h1 {{
            font-size: 4rem;
            margin: 0;
            opacity: 0.9;
        }}
        h2 {{
            font-size: 1.5rem;
            margin: 1rem 0;
            font-weight: 400;
        }}
        .code {{
            background: rgba(255, 255, 255, 0.2);
            padding: 0.5rem 1rem;
            border-radius: 8px;
            display: inline-block;
            margin: 1rem 0;
            font-family: monospace;
        }}
        a {{
            color: white;
            text-decoration: none;
            border-bottom: 2px solid white;
            padding-bottom: 2px;
            transition: opacity 0.3s;
        }}
        a:hover {{
            opacity: 0.8;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>404</h1>
        <h2>Link Not Found</h2>
        <div class="code">qck.sh/{}</div>
        <p>This link doesn't exist or may have been removed.</p>
        <p><a href="/">Go to Homepage</a></p>
    </div>
</body>
</html>"#,
        short_code
    )
}

fn expired_page(short_code: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Link Expired - QCK</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
        }}
        h1 {{
            font-size: 4rem;
            margin: 0;
            opacity: 0.9;
        }}
        h2 {{
            font-size: 1.5rem;
            margin: 1rem 0;
            font-weight: 400;
        }}
        .icon {{
            font-size: 4rem;
            margin: 1rem 0;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">⏰</div>
        <h1>Link Expired</h1>
        <p>The link qck.sh/{} has expired and is no longer available.</p>
        <p><a href="/" style="color: white;">Go to Homepage</a></p>
    </div>
</body>
</html>"#,
        short_code
    )
}

fn inactive_page(short_code: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Link Inactive - QCK</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #fa709a 0%, #fee140 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Link Inactive</h1>
        <p>The link qck.sh/{} has been deactivated.</p>
        <p><a href="/" style="color: white;">Go to Homepage</a></p>
    </div>
</body>
</html>"#,
        short_code
    )
}

fn password_required_page(short_code: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Password Required - QCK</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
        }}
        .icon {{
            font-size: 3rem;
            margin: 1rem 0;
        }}
        input {{
            padding: 0.75rem;
            border: none;
            border-radius: 8px;
            width: 200px;
            margin: 1rem 0;
        }}
        button {{
            padding: 0.75rem 2rem;
            border: none;
            border-radius: 8px;
            background: white;
            color: #667eea;
            font-weight: bold;
            cursor: pointer;
            transition: transform 0.2s;
        }}
        button:hover {{
            transform: scale(1.05);
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">🔒</div>
        <h1>Password Required</h1>
        <p>This link is password protected.</p>
        <form action="/{}/unlock" method="POST">
            <input type="password" name="password" placeholder="Enter password" required>
            <br>
            <button type="submit">Unlock</button>
        </form>
    </div>
</body>
</html>"#,
        short_code
    )
}
