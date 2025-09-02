// User Database Model
// DEV-94: User lookup for token refresh authentication

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::schema::users;

/// Onboarding status enumeration for tracking user progress
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OnboardingStatus {
    Registered,     // Just registered, needs email verification
    Verified,       // Email verified, needs to select plan
    PlanSelected,   // Plan selected, needs payment (if pro)
    PaymentPending, // Payment initiated but not completed
    Completed,      // Onboarding completed, full access
}

impl OnboardingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OnboardingStatus::Registered => "registered",
            OnboardingStatus::Verified => "verified",
            OnboardingStatus::PlanSelected => "plan_selected",
            OnboardingStatus::PaymentPending => "payment_pending",
            OnboardingStatus::Completed => "completed",
        }
    }

    pub fn from_string(s: &str) -> Result<Self, String> {
        match s {
            "registered" => Ok(OnboardingStatus::Registered),
            "verified" => Ok(OnboardingStatus::Verified),
            "plan_selected" => Ok(OnboardingStatus::PlanSelected),
            "payment_pending" => Ok(OnboardingStatus::PaymentPending),
            "completed" => Ok(OnboardingStatus::Completed),
            _ => Err(format!("Invalid onboarding status: {}", s)),
        }
    }
}

/// Subscription tier enumeration matching your pricing structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, diesel::expression::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum SubscriptionTier {
    Pending,    // User hasn't selected a tier yet (after registration, before email verification)
    Free,       // $0/month - 10 active links, 1K analytics, 100 API req/hour
    Pro,        // $19/month - 250 active links, 50K analytics, 1K API req/hour
    Business,   // $49/month - 1K active links, 250K analytics, 5K API req/hour
    Enterprise, // Custom - Unlimited everything, 15K+ API req/hour
}

impl SubscriptionTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionTier::Pending => "pending",
            SubscriptionTier::Free => "free",
            SubscriptionTier::Pro => "pro",
            SubscriptionTier::Business => "business",
            SubscriptionTier::Enterprise => "enterprise",
        }
    }

    /// Get link creation rate limit per hour for this tier
    /// For Business tier, pass user_count for scaling (1-50 users)
    pub fn link_creation_rate_limit(&self, user_count: Option<u32>) -> u32 {
        match self {
            SubscriptionTier::Pending => 5, // Very limited for unverified
            SubscriptionTier::Free => 100,  // 100 per hour
            SubscriptionTier::Pro => 1000,  // 1K per hour
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                1000 + (users - 1) * 1000 // Base 1K + 1K per additional user
            },
            SubscriptionTier::Enterprise => 15000, // 15K per hour (custom)
        }
    }

    /// Get API request rate limit per hour for this tier
    /// For Business tier, scales with user count
    pub fn api_rate_limit(&self, user_count: Option<u32>) -> u32 {
        match self {
            SubscriptionTier::Pending => 10, // Very limited for unverified
            SubscriptionTier::Free => 100,   // Basic API access
            SubscriptionTier::Pro => 1000,   // Full API access
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                1000 + (users - 1) * 1000 // Base 1K + 1K per additional user
            },
            SubscriptionTier::Enterprise => 15000, // Custom API limits
        }
    }

    /// Get maximum active links for this tier
    /// Business tier: Base 1K + 200 per additional user (up to 50 users = 10,800 links)
    pub fn max_active_links(&self, user_count: Option<u32>) -> Option<u32> {
        match self {
            SubscriptionTier::Pending => Some(1), // Just for testing
            SubscriptionTier::Free => Some(10),   // 10 active links
            SubscriptionTier::Pro => Some(250),   // 250 active links
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                Some(1000 + (users - 1) * 200) // Base 1K + 200 per additional user
            },
            SubscriptionTier::Enterprise => None, // Unlimited
        }
    }

    /// Get monthly click analytics limit for this tier
    /// Business tier: Base 250K + 50K per additional user
    pub fn monthly_analytics_limit(&self, user_count: Option<u32>) -> Option<u32> {
        match self {
            SubscriptionTier::Pending => Some(100), // Very limited
            SubscriptionTier::Free => Some(1_000),  // 1K clicks/month
            SubscriptionTier::Pro => Some(50_000),  // 50K clicks/month
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                Some(250_000 + (users - 1) * 50_000) // Base 250K + 50K per additional user
            },
            SubscriptionTier::Enterprise => None, // Unlimited
        }
    }

    /// Get custom domains limit for this tier
    /// Business tier: Base 10 + 1 per additional user
    pub fn custom_domains_limit(&self, user_count: Option<u32>) -> u32 {
        match self {
            SubscriptionTier::Pending => 0, // No custom domains
            SubscriptionTier::Free => 1,    // 1 custom domain
            SubscriptionTier::Pro => 3,     // 3 custom domains
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                10 + (users - 1) * 1 // Base 10 + 1 per additional user
            },
            SubscriptionTier::Enterprise => 1000, // Unlimited (practically)
        }
    }

    /// Get bulk operations limit for this tier
    /// Business tier: Base 1K + 100 per additional user
    pub fn bulk_operations_limit(&self, user_count: Option<u32>) -> u32 {
        match self {
            SubscriptionTier::Pending => 1, // Very limited
            SubscriptionTier::Free => 10,   // Basic bulk (10 links)
            SubscriptionTier::Pro => 100,   // 100 links/batch
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                1000 + (users - 1) * 100 // Base 1K + 100 per additional user
            },
            SubscriptionTier::Enterprise => 10000, // Very high limit
        }
    }

    /// Get campaign management limit for this tier
    /// Business tier: Base 10 + 10 per additional user
    pub fn campaign_limit(&self, user_count: Option<u32>) -> u32 {
        match self {
            SubscriptionTier::Pending => 0, // No campaigns
            SubscriptionTier::Free => 1,    // 1 basic campaign
            SubscriptionTier::Pro => 5,     // 5 campaigns
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                10 + (users - 1) * 10 // Base 10 + 10 per additional user
            },
            SubscriptionTier::Enterprise => 1000, // Unlimited (practically)
        }
    }

    /// Calculate monthly price for this tier with user count
    /// Business tier: $49 base + $10 per additional user
    pub fn monthly_price_usd(&self, user_count: Option<u32>) -> u32 {
        match self {
            SubscriptionTier::Pending => 0, // Free during setup
            SubscriptionTier::Free => 0,    // Always free
            SubscriptionTier::Pro => 19,    // $19/month
            SubscriptionTier::Business => {
                let users = user_count.unwrap_or(1).min(50).max(1); // 1-50 users
                49 + (users - 1) * 10 // $49 base + $10 per additional user
            },
            SubscriptionTier::Enterprise => 0, // Custom pricing
        }
    }
}

impl FromStr for SubscriptionTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(SubscriptionTier::Pending),
            "free" => Ok(SubscriptionTier::Free),
            "pro" => Ok(SubscriptionTier::Pro),
            "business" => Ok(SubscriptionTier::Business),
            "enterprise" => Ok(SubscriptionTier::Enterprise),
            _ => Err(format!("Invalid subscription tier: {}", s)),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for SubscriptionTier
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let value = String::from_sql(bytes)?;
        Self::from_str(&value).map_err(|e| e.into())
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for SubscriptionTier
where
    DB: diesel::backend::Backend,
    str: diesel::serialize::ToSql<diesel::sql_types::Text, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.as_str().to_sql(out)
    }
}

/// User database model - queryable from database
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub email_verified: bool,
    pub subscription_tier: String, // Will convert to enum
    pub email_verified_at: Option<DateTime<Utc>>,
    pub full_name: String,
    pub company_name: Option<String>,
    pub onboarding_status: String,
}

/// New user for insertion
#[derive(Debug, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub subscription_tier: String,
    pub full_name: String,
    pub company_name: Option<String>,
    pub onboarding_status: String,
}

/// User update struct
#[derive(Debug, AsChangeset)]
#[diesel(table_name = users)]
pub struct UserUpdate {
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified: Option<bool>,
    pub email_verified_at: Option<Option<DateTime<Utc>>>,
    pub subscription_tier: Option<String>,
    pub is_active: Option<bool>,
    pub full_name: Option<String>,
    pub company_name: Option<Option<String>>,
    pub onboarding_status: Option<String>,
}

/// Errors for user operations
#[derive(thiserror::Error, Debug)]
pub enum UserError {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("User not found")]
    NotFound,

    #[error("Invalid user ID format")]
    InvalidId,

    #[error("Connection pool error")]
    Pool(String),
}

impl User {
    /// Find user by ID
    pub async fn find_by_id(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
    ) -> Result<Self, UserError> {
        use crate::schema::users::dsl::*;

        users
            .filter(id.eq(user_id))
            .first::<User>(conn)
            .await
            .map_err(|e| match e {
                diesel::result::Error::NotFound => UserError::NotFound,
                _ => UserError::Database(e),
            })
    }

    /// Find user by email (case-insensitive)
    pub async fn find_by_email(
        conn: &mut AsyncPgConnection,
        email_str: &str,
    ) -> Result<Self, UserError> {
        use crate::schema::users::dsl::*;
        use diesel::PgTextExpressionMethods;

        // Use Diesel's ilike for PostgreSQL case-insensitive comparison
        users
            .filter(email.ilike(email_str))
            .first::<User>(conn)
            .await
            .map_err(|e| match e {
                diesel::result::Error::NotFound => UserError::NotFound,
                _ => UserError::Database(e),
            })
    }

    /// Create a new user
    pub async fn create(
        conn: &mut AsyncPgConnection,
        new_user: NewUser,
    ) -> Result<Self, UserError> {
        use crate::schema::users::dsl::*;

        diesel::insert_into(users)
            .values(&new_user)
            .get_result::<User>(conn)
            .await
            .map_err(UserError::Database)
    }

    /// Update user
    pub async fn update(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        update: UserUpdate,
    ) -> Result<Self, UserError> {
        use crate::schema::users::dsl::*;

        diesel::update(users.filter(id.eq(user_id)))
            .set(&update)
            .get_result::<User>(conn)
            .await
            .map_err(|e| match e {
                diesel::result::Error::NotFound => UserError::NotFound,
                _ => UserError::Database(e),
            })
    }

    /// Get user's subscription tier as enum
    pub fn subscription_tier_enum(&self) -> SubscriptionTier {
        SubscriptionTier::from_str(&self.subscription_tier).unwrap_or_else(|e| {
            tracing::warn!(
                "Invalid subscription tier '{}' for user {}, defaulting to Free: {}",
                self.subscription_tier,
                self.id,
                e
            );
            SubscriptionTier::Free
        })
    }

    /// Get user's subscription tier as string for compatibility
    pub fn subscription_tier_str(&self) -> &str {
        &self.subscription_tier
    }

    /// Get user's onboarding status as enum
    /// Returns Result to handle invalid statuses properly
    pub fn onboarding_status_enum(&self) -> Result<OnboardingStatus, String> {
        OnboardingStatus::from_string(&self.onboarding_status)
    }

    /// Get user's onboarding status as enum with fallback
    /// Returns Registered as safe default for invalid statuses to prevent auth flow disruption; logs warning for monitoring
    pub fn onboarding_status_enum_with_fallback(&self) -> OnboardingStatus {
        self.onboarding_status_enum().unwrap_or_else(|e| {
            tracing::warn!(
                "Invalid onboarding status '{}' for user {}, defaulting to Registered: {}",
                self.onboarding_status,
                self.id,
                e
            );
            OnboardingStatus::Registered
        })
    }

    /// Check if user has completed onboarding
    pub fn is_onboarding_complete(&self) -> bool {
        match self.onboarding_status_enum() {
            Ok(OnboardingStatus::Completed) => true,
            Ok(_) => false,
            Err(_) => {
                // Log warning and default to false for safety
                tracing::warn!(
                    "Invalid onboarding status '{}' for user {}, treating as incomplete",
                    self.onboarding_status,
                    self.id
                );
                false
            },
        }
    }

    /// Check if user needs to complete payment
    pub fn needs_payment(&self) -> bool {
        match (self.onboarding_status_enum(), self.subscription_tier_enum()) {
            (Ok(OnboardingStatus::PlanSelected), SubscriptionTier::Pro) => true,
            (Ok(_), _) => false,
            (Err(_), _) => {
                // Log warning and default to false for safety
                tracing::warn!(
                    "Invalid onboarding status '{}' for user {}, treating as no payment needed",
                    self.onboarding_status,
                    self.id
                );
                false
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_subscription_tier_conversion() {
        assert_eq!(SubscriptionTier::Free.as_str(), "free");
        assert_eq!(SubscriptionTier::Pro.as_str(), "pro");

        assert_eq!(
            SubscriptionTier::from_str("free"),
            Ok(SubscriptionTier::Free)
        );
        assert_eq!(SubscriptionTier::from_str("pro"), Ok(SubscriptionTier::Pro));
        assert!(SubscriptionTier::from_str("invalid").is_err());
    }

    #[test]
    fn test_onboarding_status_conversion() {
        assert_eq!(OnboardingStatus::Registered.as_str(), "registered");
        assert_eq!(OnboardingStatus::Verified.as_str(), "verified");
        assert_eq!(OnboardingStatus::PlanSelected.as_str(), "plan_selected");
        assert_eq!(OnboardingStatus::PaymentPending.as_str(), "payment_pending");
        assert_eq!(OnboardingStatus::Completed.as_str(), "completed");

        assert_eq!(
            OnboardingStatus::from_string("registered"),
            Ok(OnboardingStatus::Registered)
        );
        assert_eq!(
            OnboardingStatus::from_string("verified"),
            Ok(OnboardingStatus::Verified)
        );
        assert_eq!(
            OnboardingStatus::from_string("plan_selected"),
            Ok(OnboardingStatus::PlanSelected)
        );
        assert_eq!(
            OnboardingStatus::from_string("payment_pending"),
            Ok(OnboardingStatus::PaymentPending)
        );
        assert_eq!(
            OnboardingStatus::from_string("completed"),
            Ok(OnboardingStatus::Completed)
        );
        assert!(OnboardingStatus::from_string("invalid").is_err());
    }

    #[test]
    fn test_onboarding_status_enum_methods() {
        let now = Utc::now();

        // Test completed user
        let completed_user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            email_verified: true,
            subscription_tier: "pro".to_string(),
            email_verified_at: Some(now),
            full_name: "Test User".to_string(),
            company_name: None,
            onboarding_status: OnboardingStatus::Completed.as_str().to_string(),
        };

        assert_eq!(
            completed_user.onboarding_status_enum().unwrap(),
            OnboardingStatus::Completed
        );
        assert!(completed_user.is_onboarding_complete());
        assert!(!completed_user.needs_payment());

        // Test plan selected pro user (needs payment)
        let plan_selected_user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            email_verified: true,
            subscription_tier: "pro".to_string(),
            email_verified_at: Some(now),
            full_name: "Test User".to_string(),
            company_name: None,
            onboarding_status: OnboardingStatus::PlanSelected.as_str().to_string(),
        };

        assert_eq!(
            plan_selected_user.onboarding_status_enum().unwrap(),
            OnboardingStatus::PlanSelected
        );
        assert!(!plan_selected_user.is_onboarding_complete());
        assert!(plan_selected_user.needs_payment());

        // Test plan selected free user (no payment needed)
        let plan_selected_free_user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            email_verified: true,
            subscription_tier: "free".to_string(),
            email_verified_at: Some(now),
            full_name: "Test User".to_string(),
            company_name: None,
            onboarding_status: OnboardingStatus::PlanSelected.as_str().to_string(),
        };

        assert!(!plan_selected_free_user.needs_payment());

        // Test registered user
        let registered_user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            email_verified: false,
            subscription_tier: "pending".to_string(),
            email_verified_at: None,
            full_name: "Test User".to_string(),
            company_name: None,
            onboarding_status: OnboardingStatus::Registered.as_str().to_string(),
        };

        assert_eq!(
            registered_user.onboarding_status_enum().unwrap(),
            OnboardingStatus::Registered
        );
        assert!(!registered_user.is_onboarding_complete());
        assert!(!registered_user.needs_payment());
    }

    #[test]
    fn test_invalid_onboarding_status_handling() {
        let now = Utc::now();

        // Test user with invalid onboarding status
        let invalid_status_user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            email_verified: true,
            subscription_tier: "pro".to_string(),
            email_verified_at: Some(now),
            full_name: "Test User".to_string(),
            company_name: None,
            onboarding_status: "invalid_status".to_string(),
        };

        // onboarding_status_enum() should return an error
        assert!(invalid_status_user.onboarding_status_enum().is_err());

        // fallback method should return Registered and log warning
        assert_eq!(
            invalid_status_user.onboarding_status_enum_with_fallback(),
            OnboardingStatus::Registered
        );

        // Methods should handle invalid status gracefully
        assert!(!invalid_status_user.is_onboarding_complete());
        assert!(!invalid_status_user.needs_payment());
    }
}
