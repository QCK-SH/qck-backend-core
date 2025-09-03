use axum::{
    body::Body,
    http::{
        header::{self, HeaderValue},
        Method, Request, Response, StatusCode,
    },
    middleware::Next,
};
use tracing::debug;

/// Dynamic CORS middleware that handles wildcard for staging/development
/// while properly supporting credentials
pub async fn dynamic_cors_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    let config = crate::app_config::config();
    
    // Get the origin from the request
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Check if wildcard is configured
    let has_wildcard = config
        .cors_allowed_origins
        .iter()
        .any(|o| o == "*");

    // For staging/dev with wildcard: reflect the origin (allows any origin with credentials)
    // For production or specific origins: check against whitelist
    let allowed_origin = if has_wildcard && !config.is_production() {
        // Staging/Dev mode with wildcard - reflect the origin back
        debug!("CORS: Reflecting origin for staging/dev: {:?}", origin);
        origin.clone()
    } else {
        // Production mode or specific origins - check whitelist
        origin.as_ref().and_then(|req_origin| {
            if config.cors_allowed_origins.contains(req_origin) {
                debug!("CORS: Origin allowed from whitelist: {}", req_origin);
                Some(req_origin.clone())
            } else {
                debug!("CORS: Origin not in whitelist: {}", req_origin);
                None
            }
        })
    };

    // Handle preflight OPTIONS requests
    if req.method() == Method::OPTIONS {
        let mut response = Response::new(Body::empty());
        
        if let Some(allowed) = allowed_origin {
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_str(&allowed).unwrap(),
            );
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
            );
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("content-type, authorization, accept, origin, x-requested-with"),
            );
            response.headers_mut().insert(
                header::ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_static("3600"),
            );
        }
        
        *response.status_mut() = StatusCode::OK;
        return Ok(response);
    }

    // Process the actual request
    let mut response = next.run(req).await;

    // Add CORS headers to the response
    if let Some(allowed) = allowed_origin {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_str(&allowed).unwrap(),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
    }

    Ok(response)
}