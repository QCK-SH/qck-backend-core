// Onboarding handlers for QCK Backend
// Handles plan selection and onboarding flow

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{app::AppState, middleware::auth::AuthenticatedUser, schema::users};

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Request payload for selecting a subscription plan
#[derive(Debug, Deserialize, Serialize)]
pub struct SelectPlanRequest {
    pub plan: String, // "free", "pro", "enterprise"
    pub price: i32,   // 0, 19, 49
}

/// Response for plan selection
#[derive(Debug, Serialize)]
pub struct SelectPlanResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<SelectPlanData>,
}

#[derive(Debug, Serialize)]
pub struct SelectPlanData {
    pub onboarding_status: String,
    pub subscription_tier: String,
    pub requires_payment: bool,
    pub next_step: String,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// POST /onboarding/select-plan - Select subscription plan during onboarding
pub async fn select_plan(
    State(app_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<SelectPlanRequest>,
) -> impl IntoResponse {
    // Validate the plan and price combination
    let subscription_tier = match (payload.plan.as_str(), payload.price) {
        ("free", 0) => "free",
        ("pro", 19) => "pro",
        ("enterprise", 49) => "enterprise",
        _ => {
            tracing::warn!(
                "Invalid plan selection attempt: {} with price ${} by user {}",
                payload.plan,
                payload.price,
                auth_user.user_id
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(SelectPlanResponse {
                    success: false,
                    message: format!(
                        "Invalid plan selection: {} with price ${}",
                        payload.plan, payload.price
                    ),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    // Parse user_id as UUID
    let user_id = match Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!("Invalid user ID format: {}", auth_user.user_id);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SelectPlanResponse {
                    success: false,
                    message: "Invalid user session".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    // Get database connection
    let mut conn = match app_state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SelectPlanResponse {
                    success: false,
                    message: "Service temporarily unavailable".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    // Check current user status
    let current_user = match users::dsl::users
        .filter(users::id.eq(user_id))
        .select((users::email_verified, users::onboarding_status))
        .first::<(bool, String)>(&mut conn)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to get user data: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SelectPlanResponse {
                    success: false,
                    message: "Failed to retrieve user data".to_string(),
                    data: None,
                }),
            )
                .into_response();
        },
    };

    // Ensure email is verified before plan selection
    if !current_user.0 {
        return (
            StatusCode::FORBIDDEN,
            Json(SelectPlanResponse {
                success: false,
                message: "Please verify your email before selecting a plan".to_string(),
                data: None,
            }),
        )
            .into_response();
    }

    // Determine the final onboarding status and next step
    let (final_status, next_step) = if subscription_tier == "free" {
        // Free plan: complete onboarding immediately
        ("completed", "dashboard")
    } else {
        // Paid plans: need payment (when payment gateway is added)
        // For now, we'll complete onboarding but note that payment is pending
        ("plan_selected", "payment") // Will change to "payment_pending" with payment gateway
    };

    let requires_payment = subscription_tier != "free";

    // Update user's subscription tier and onboarding status
    let result = diesel::update(users::dsl::users.filter(users::id.eq(user_id)))
        .set((
            users::onboarding_status.eq(final_status),
            users::subscription_tier.eq(subscription_tier),
        ))
        .execute(&mut conn)
        .await;

    match result {
        Ok(_) => {
            tracing::info!(
                "User {} selected {} plan (${}/mo) - Status: {}, Next: {}",
                user_id,
                subscription_tier,
                payload.price,
                final_status,
                next_step
            );

            (
                StatusCode::OK,
                Json(SelectPlanResponse {
                    success: true,
                    message: if requires_payment {
                        "Plan selected successfully. Payment setup required.".to_string()
                    } else {
                        "Free plan activated successfully!".to_string()
                    },
                    data: Some(SelectPlanData {
                        onboarding_status: final_status.to_string(),
                        subscription_tier: subscription_tier.to_string(),
                        requires_payment,
                        next_step: next_step.to_string(),
                    }),
                }),
            )
                .into_response()
        },
        Err(e) => {
            tracing::error!("Failed to update user plan: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SelectPlanResponse {
                    success: false,
                    message: "Failed to update subscription plan".to_string(),
                    data: None,
                }),
            )
                .into_response()
        },
    }
}

/// GET /onboarding/status - Get current onboarding status
pub async fn get_status(
    State(app_state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> impl IntoResponse {
    #[derive(Debug, Serialize)]
    struct OnboardingStatusResponse {
        success: bool,
        data: OnboardingStatusData,
    }

    #[derive(Debug, Serialize)]
    struct OnboardingStatusData {
        onboarding_status: String,
        email_verified: bool,
        subscription_tier: String,
        completed_steps: Vec<String>,
        next_step: String,
    }

    let user_id = match Uuid::parse_str(&auth_user.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "message": "Invalid user session"
                })),
            )
                .into_response();
        },
    };

    let mut conn = match app_state.diesel_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "message": "Service temporarily unavailable"
                })),
            )
                .into_response();
        },
    };

    match users::dsl::users
        .filter(users::id.eq(user_id))
        .select((
            users::onboarding_status,
            users::email_verified,
            users::subscription_tier,
        ))
        .first::<(String, bool, String)>(&mut conn)
        .await
    {
        Ok((status, verified, tier)) => {
            let mut completed_steps = vec!["registered".to_string()];

            if verified {
                completed_steps.push("verified".to_string());
            }

            if status == "plan_selected" || status == "completed" {
                completed_steps.push("plan_selected".to_string());
            }

            if status == "completed" {
                completed_steps.push("completed".to_string());
            }

            let next_step = match (status.as_str(), verified) {
                (_, false) => "verify_email",
                ("verified", true) => "select_plan",
                ("plan_selected", true) if tier != "free" => "payment",
                ("completed", true) => "dashboard",
                _ => "dashboard",
            };

            (
                StatusCode::OK,
                Json(OnboardingStatusResponse {
                    success: true,
                    data: OnboardingStatusData {
                        onboarding_status: status,
                        email_verified: verified,
                        subscription_tier: tier,
                        completed_steps,
                        next_step: next_step.to_string(),
                    },
                }),
            )
                .into_response()
        },
        Err(e) => {
            tracing::error!("Failed to get onboarding status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "message": "Failed to retrieve onboarding status"
                })),
            )
                .into_response()
        },
    }
}
