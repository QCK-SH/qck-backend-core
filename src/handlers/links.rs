// DEV-105: Link Creation API Endpoints
// DEV-68: Complete CRUD API for link management
// Working together to bring the vision to life!

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rand::{thread_rng, Rng};
use serde_json::json;
use tracing::{error, info, warn};
use uuid::Uuid;
use validator::Validate;

use crate::{
    app::AppState,
    middleware::auth::AuthenticatedUser,
    models::link::{
        CreateLinkRequest, LinkFilter, LinkPagination, ListLinksParams, UpdateLinkRequest,
    },
    services::link::LinkService,
    utils::{link_errors::LinkError, service_error::ServiceError},
};

// =============================================================================
// LINK HANDLERS
// =============================================================================

/// Create a new short link
/// POST /api/v1/links
#[utoipa::path(
    post,
    path = "/v1/links",
    tag = "Links",
    operation_id = "createLink",
    request_body = CreateLinkRequest,
    responses(
        (status = 201, description = "Link created successfully", body = LinkResponse),
        (status = 400, description = "Bad request - validation failed"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 409, description = "Conflict - custom alias already exists"),
        (status = 429, description = "Too many requests - rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_link(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Json(request): Json<CreateLinkRequest>,
) -> impl IntoResponse {
    use crate::models::user::User;

    // Validate request
    if let Err(e) = request.validate() {
        return LinkError::from(e).into_response();
    }

    // Get database connection
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Fetch the actual user from database
    let user = match User::find_by_id(&mut conn, user_uuid).await {
        Ok(user) => user,
        Err(_) => return LinkError::NotFound.into_response(),
    };

    // Check rate limiting based on subscription tier
    use crate::models::user::SubscriptionTier;
    use crate::services::rate_limit::SubscriptionLimits;

    // Parse string subscription tier to enum
    let subscription_tier = user
        .subscription_tier
        .parse::<SubscriptionTier>()
        .unwrap_or(SubscriptionTier::Free); // Default to Free if parsing fails

    let rate_limit_config = SubscriptionLimits::get_tier_config(&subscription_tier, None); // TODO: Add team user count support
    let rate_limit_key = format!("user:{}:link_creation", user.id);
    let rate_limit_check = state
        .rate_limit_service
        .check_rate_limit_with_config(&rate_limit_key, &rate_limit_config)
        .await;

    match rate_limit_check {
        Ok(result) if !result.allowed => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": "Rate limit exceeded",
                    "message": format!("Too many requests. Try again in {} seconds.", result.retry_after.unwrap_or(3600)),
                    "retry_after": result.retry_after.unwrap_or(3600)
                }))
            ).into_response();
        },
        Ok(_) => {
            // Rate limit passed, continue
        },
        Err(e) => {
            // Log error but don't block the request
            tracing::warn!("Rate limit check failed: {}", e);
            // Continue with request - fail open for availability
        },
    }

    // Create link service
    let link_service = LinkService::new(&state);

    // Create the link
    match link_service.create_link(&user, request).await {
        Ok(link_response) => (StatusCode::CREATED, Json(link_response)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Get a specific link by ID
/// GET /api/v1/links/:id
#[utoipa::path(
    get,
    path = "/v1/links/{id}",
    tag = "Links",
    operation_id = "getLink",
    params(
        ("id" = Uuid, Path, description = "Link ID (UUID)", example = "123e4567-e89b-12d3-a456-426614174000")
    ),
    responses(
        (status = 200, description = "Link retrieved successfully", body = LinkResponse),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 403, description = "Forbidden - not the link owner"),
        (status = 404, description = "Link not found")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_link(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(link_id): Path<Uuid>,
) -> impl IntoResponse {
    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Use the service layer with proper soft delete filtering
    let link_service = LinkService::new(&state);

    // Get link with ClickHouse stats included
    match link_service.get_link_with_stats(link_id, user_uuid).await {
        Ok(response) => Json(response).into_response(),
        Err(ServiceError::NotFound) => LinkError::NotFound.into_response(),
        Err(e) => LinkError::DatabaseError(e.to_string()).into_response(),
    }
}

/// Update an existing link
/// PUT /api/v1/links/:id
#[utoipa::path(
    put,
    path = "/v1/links/{id}",
    tag = "Links",
    operation_id = "updateLink",
    params(
        ("id" = Uuid, Path, description = "Link ID (UUID)", example = "123e4567-e89b-12d3-a456-426614174000")
    ),
    request_body = UpdateLinkRequest,
    responses(
        (status = 200, description = "Link updated successfully", body = LinkResponse),
        (status = 400, description = "Bad request - validation failed"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 403, description = "Forbidden - not the link owner"),
        (status = 404, description = "Link not found")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn update_link(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(link_id): Path<Uuid>,
    Json(request): Json<UpdateLinkRequest>,
) -> impl IntoResponse {
    use crate::models::user::User;

    // Validate request if it has fields
    if let Err(e) = request.validate() {
        return LinkError::from(e).into_response();
    }

    // Get database connection
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Fetch the actual user from database
    let user = match User::find_by_id(&mut conn, user_uuid).await {
        Ok(user) => user,
        Err(_) => return LinkError::NotFound.into_response(),
    };

    let link_service = LinkService::new(&state);

    match link_service.update_link(&user, link_id, request).await {
        Ok(link_response) => Json(link_response).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Delete (deactivate) a link
/// DELETE /api/v1/links/:id
#[utoipa::path(
    delete,
    path = "/v1/links/{id}",
    tag = "Links",
    operation_id = "deleteLink",
    params(
        ("id" = Uuid, Path, description = "Link ID (UUID)", example = "123e4567-e89b-12d3-a456-426614174000")
    ),
    responses(
        (status = 204, description = "Link deleted successfully (no content)"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 403, description = "Forbidden - not the link owner"),
        (status = 404, description = "Link not found")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn delete_link(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(link_id): Path<Uuid>,
) -> impl IntoResponse {
    use crate::models::user::User;

    // Get database connection
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Fetch the actual user from database
    let user = match User::find_by_id(&mut conn, user_uuid).await {
        Ok(user) => user,
        Err(_) => return LinkError::NotFound.into_response(),
    };

    let link_service = LinkService::new(&state);

    match link_service.delete_link(&user, link_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

/// List user's links with filtering and pagination
/// GET /api/v1/links
#[utoipa::path(
    get,
    path = "/v1/links",
    tag = "Links",
    operation_id = "listLinks",
    params(
        LinkPagination,
        LinkFilter
    ),
    responses(
        (status = 200, description = "Links retrieved successfully", body = LinkListResponse),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_links(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Query(filter): Query<LinkFilter>,
    Query(pagination): Query<LinkPagination>,
) -> impl IntoResponse {
    use crate::models::user::User;

    // Get database connection
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Fetch the actual user from database
    let user = match User::find_by_id(&mut conn, user_uuid).await {
        Ok(user) => user,
        Err(_) => return LinkError::NotFound.into_response(),
    };

    let link_service = LinkService::new(&state);

    // Create ListLinksParams from filter and pagination
    let params = ListLinksParams {
        page: pagination.page as u32,
        per_page: pagination.per_page as u32,
        sort_by: None, // Can be added to the query if needed
        filter,
    };

    match link_service.get_user_links(&user, params).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Get link statistics
/// GET /api/v1/links/:id/stats
#[utoipa::path(
    get,
    path = "/v1/links/{id}/stats",
    tag = "Links",
    operation_id = "getLinkStats",
    params(
        ("id" = Uuid, Path, description = "Link ID (UUID)", example = "123e4567-e89b-12d3-a456-426614174000")
    ),
    responses(
        (status = 200, description = "Link statistics retrieved successfully"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 403, description = "Forbidden - not the link owner"),
        (status = 404, description = "Link not found")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_link_stats(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(link_id): Path<Uuid>,
) -> impl IntoResponse {
    use crate::schema::links::dsl;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use serde_json::json;

    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Get link and verify ownership
    let link = match dsl::links
        .filter(dsl::id.eq(link_id))
        .filter(dsl::user_id.eq(user_uuid))
        .first::<crate::models::link::Link>(&mut conn)
        .await
    {
        Ok(link) => link,
        Err(diesel::result::Error::NotFound) => return LinkError::NotFound.into_response(),
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Query ClickHouse for real-time analytics using unified service
    let mut total_clicks = 0u64;
    let mut unique_visitors = 0u64;
    let mut bot_clicks = 0u64;
    let mut last_accessed = link.last_accessed_at;

    // Use the unified ClickHouseAnalyticsService if available
    if let Some(ref analytics) = state.clickhouse_analytics {
        if let Some(stats) = analytics.get_link_stats(&link_id).await {
            total_clicks = stats.total_clicks;
            unique_visitors = stats.unique_visitors;
            bot_clicks = stats.bot_clicks;
            if let Some(last_click) = stats.last_accessed_at {
                last_accessed = Some(last_click);
            }
        }
    }

    // Build enhanced statistics response with ClickHouse data
    let stats = json!({
        "short_code": link.short_code,
        "original_url": link.original_url,
        "total_clicks": total_clicks,
        "unique_visitors": unique_visitors,
        "bot_clicks": bot_clicks,
        "human_clicks": total_clicks - bot_clicks,
        "created_at": link.created_at,
        "last_accessed_at": last_accessed,
        "is_active": link.is_active,
        "days_active": (chrono::Utc::now() - link.created_at).num_days(),
        "average_clicks_per_day": if (chrono::Utc::now() - link.created_at).num_days() > 0 {
            total_clicks as f64 / (chrono::Utc::now() - link.created_at).num_days() as f64
        } else {
            total_clicks as f64
        },
        "conversion_rate": if total_clicks > 0 {
            (unique_visitors as f64 / total_clicks as f64 * 100.0).round() / 100.0
        } else {
            0.0
        }
    });

    Json(stats).into_response()
}

/// Check custom alias availability
/// GET /api/v1/links/check-alias/:alias
#[utoipa::path(
    get,
    path = "/v1/links/check-alias/{alias}",
    tag = "Links",
    operation_id = "checkAliasAvailability",
    params(
        ("alias" = String, Path, description = "Custom alias to check")
    ),
    responses(
        (status = 200, description = "Alias is available", body = CheckAliasResponse),
        (status = 409, description = "Alias is taken - suggestions provided", body = CheckAliasResponse),
        (status = 400, description = "Invalid alias format")
    )
)]
pub async fn check_alias_availability(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> impl IntoResponse {
    use crate::services::short_code::ShortCodeGenerator;
    use serde_json::json;

    // Validate alias format
    if alias.is_empty() || alias.len() > 20 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid alias: must be 1-20 characters",
                "available": false
            })),
        )
            .into_response();
    }

    // Check for invalid characters (only allow alphanumeric, dash, underscore, dot)
    let valid_chars = alias
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !valid_chars {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid alias: only letters, numbers, dash, underscore, and dot allowed",
                "available": false
            })),
        )
            .into_response();
    }

    // Initialize short code generator
    let generator =
        ShortCodeGenerator::with_redis(state.diesel_pool.clone(), Some(state.redis_pool.clone()));

    // Check if alias is available
    match generator.is_code_unique(&alias).await {
        Ok(true) => {
            // Alias is available
            (
                StatusCode::OK,
                Json(json!({
                    "available": true,
                    "alias": alias,
                    "message": "This alias is available!"
                })),
            )
                .into_response()
        },
        Ok(false) => {
            // Alias is taken - generate suggestions
            let suggestions = generator.generate_suggestions(&alias).await;

            (
                StatusCode::CONFLICT,
                Json(json!({
                    "available": false,
                    "alias": alias,
                    "message": format!("'{}' is already taken", alias),
                    "suggestions": suggestions,
                    "suggestion_message": "Try one of these available alternatives:"
                })),
            )
                .into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to check alias: {}", e),
                "available": false
            })),
        )
            .into_response(),
    }
}

/// Create custom short link
/// POST /api/v1/links/custom
#[utoipa::path(
    post,
    path = "/v1/links/custom",
    tag = "Links",
    operation_id = "createCustomLink",
    request_body = CreateCustomLinkRequest,
    responses(
        (status = 200, description = "Custom link created", body = CreateCustomLinkResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_custom_link(
    State(state): State<AppState>,
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    use crate::services::short_code::ShortCodeGenerator;
    use serde_json::json;

    // Extract parameters
    let prefix = request.get("prefix").and_then(|v| v.as_str()).unwrap_or("");

    let style = request
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("discord"); // discord, readable, random

    // Initialize generator
    let generator =
        ShortCodeGenerator::with_redis(state.diesel_pool.clone(), Some(state.redis_pool.clone()));

    // Generate code based on style
    let mut generated_code = match style {
        "discord" => {
            // Discord-style: 7-8 character random alphanumeric
            let mut code = String::new();
            let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            let mut rng = thread_rng();
            for _ in 0..7 {
                let idx = rng.gen_range(0..chars.len());
                code.push(chars.chars().nth(idx).unwrap());
            }
            code
        },
        "readable" => {
            // Readable: word-word-number format
            let words = vec![
                "happy", "swift", "bright", "cool", "smart", "quick", "fresh", "bold",
            ];
            let mut rng = thread_rng();
            let word1 = words[rng.gen_range(0..words.len())];
            let word2 = words[rng.gen_range(0..words.len())];
            let num = rng.gen_range(10..99);
            format!("{}-{}-{}", word1, word2, num)
        },
        _ => {
            // Random: Use existing generator
            generator.generate_random_code(6)
        },
    };

    // Add prefix if provided
    if !prefix.is_empty() && prefix.len() <= 10 {
        generated_code = format!("{}-{}", prefix, generated_code);
    }

    // Ensure uniqueness
    let mut attempts = 0;
    while attempts < 10 {
        match generator.is_code_unique(&generated_code).await {
            Ok(true) => break,
            _ => {
                // Regenerate with different random
                let suffix = thread_rng().gen_range(100..999);
                generated_code = format!("{}{}", generated_code, suffix);
                attempts += 1;
            },
        }
    }

    Json(json!({
        "short_code": generated_code,
        "style": style,
        "message": "Custom short link created successfully",
        "available": true,
        "short_url": format!("https://qck.sh/{}", generated_code)
    }))
    .into_response()
}

/// Bulk create links
/// POST /api/v1/links/bulk
#[utoipa::path(
    post,
    path = "/v1/links/bulk",
    tag = "Links",
    operation_id = "bulkCreateLinks",
    request_body(content = Vec<CreateLinkRequest>, description = "Array of link creation requests (max 100)"),
    responses(
        (status = 207, description = "Multi-status - partial success (some links created, some failed)"),
        (status = 400, description = "Bad request - invalid request format or too many links"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 429, description = "Too many requests - rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn bulk_create_links(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Json(requests): Json<Vec<CreateLinkRequest>>,
) -> impl IntoResponse {
    use crate::models::user::User;

    // Validate request count
    if requests.is_empty() {
        return LinkError::BadRequest("No links provided".to_string()).into_response();
    }

    // Limit bulk operations (could be tier-based in future)
    const MAX_BULK_CREATE: usize = 100;
    if requests.len() > MAX_BULK_CREATE {
        return LinkError::BadRequest(format!(
            "Maximum {} links can be created at once",
            MAX_BULK_CREATE
        ))
        .into_response();
    }

    // Get database connection
    let mut conn = match state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse user_id from string to UUID
    let user_uuid = match uuid::Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return LinkError::BadRequest("Invalid user ID format".to_string()).into_response()
        },
    };

    // Fetch the actual user from database
    let user = match User::find_by_id(&mut conn, user_uuid).await {
        Ok(user) => user,
        Err(_) => return LinkError::NotFound.into_response(),
    };

    // Check subscription limits BEFORE processing
    use crate::models::user::SubscriptionTier;
    use crate::schema::links::dsl;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    // Get current active link count
    let current_active_count: i64 = match dsl::links
        .filter(dsl::user_id.eq(user_uuid))
        .filter(dsl::is_active.eq(true))
        .filter(dsl::deleted_at.is_null())
        .count()
        .get_result(&mut conn)
        .await
    {
        Ok(count) => count,
        Err(e) => return LinkError::DatabaseError(e.to_string()).into_response(),
    };

    // Parse subscription tier and check limits
    let subscription_tier = user
        .subscription_tier
        .parse::<SubscriptionTier>()
        .unwrap_or(SubscriptionTier::Free);

    let max_links = subscription_tier.max_active_links(None).unwrap_or(10) as i64;
    let remaining_quota = max_links.saturating_sub(current_active_count);

    // Check if they can create ANY links
    if remaining_quota <= 0 {
        return LinkError::BadRequest(format!(
            "Link limit ({}) reached for {} tier. Cannot create more links.",
            max_links, user.subscription_tier
        ))
        .into_response();
    }

    // Check if they're trying to create more than allowed
    let requested_count = requests.len() as i64;
    if requested_count > remaining_quota {
        return LinkError::BadRequest(format!(
            "Cannot create {} links. You have {} remaining in your {} tier quota.",
            requested_count, remaining_quota, user.subscription_tier
        ))
        .into_response();
    }

    let link_service = LinkService::new(&state);
    let mut results = Vec::new();
    let mut errors = Vec::new();

    info!(
        "Processing bulk creation of {} links for user {}",
        requested_count, user.id
    );

    for (index, request) in requests.into_iter().enumerate() {
        // Validate each request
        if let Err(e) = request.validate() {
            errors.push(serde_json::json!({
                "index": index,
                "error": e.to_string()
            }));
            continue;
        }

        // Create link with timeout to prevent hanging
        let create_result = tokio::time::timeout(
            std::time::Duration::from_secs(10), // 10 second timeout per link
            link_service.create_link(&user, request),
        )
        .await;

        match create_result {
            Ok(Ok(link_response)) => {
                info!(
                    "Successfully created link {} of {}",
                    index + 1,
                    requested_count
                );
                results.push(link_response);
            },
            Ok(Err(e)) => {
                warn!(
                    "Failed to create link {} of {}: {}",
                    index + 1,
                    requested_count,
                    e
                );
                errors.push(serde_json::json!({
                    "index": index,
                    "error": e.to_string()
                }));
            },
            Err(_) => {
                error!("Timeout creating link {} of {}", index + 1, requested_count);
                errors.push(serde_json::json!({
                    "index": index,
                    "error": "Request timeout - link creation took too long"
                }));
            },
        }
    }

    let response = serde_json::json!({
        "success": results.len(),
        "failed": errors.len(),
        "links": results,
        "errors": errors,
    });

    (StatusCode::MULTI_STATUS, Json(response)).into_response()
}
