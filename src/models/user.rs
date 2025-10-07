// User Database Model
// DEV-94: User lookup for token refresh authentication

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::schema::users;

/// Onboarding status enumeration for tracking user progress (OSS simplified)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OnboardingStatus {
    Registered, // Just registered (auto-verified in OSS)
    Completed,  // Onboarding completed, full access
}

impl OnboardingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OnboardingStatus::Registered => "registered",
            OnboardingStatus::Completed => "completed",
        }
    }

    pub fn from_string(s: &str) -> Result<Self, String> {
        match s {
            "registered" => Ok(OnboardingStatus::Registered),
            "completed" => Ok(OnboardingStatus::Completed),
            // Legacy compatibility for existing data
            "verified" | "plan_selected" | "payment_pending" => Ok(OnboardingStatus::Completed),
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

    /// Check if user has completed onboarding (OSS simplified - no payments)
    pub fn needs_payment(&self) -> bool {
        // OSS version has no payments
        false
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
        // OSS simplified onboarding - only registered and completed
        assert_eq!(OnboardingStatus::Registered.as_str(), "registered");
        assert_eq!(OnboardingStatus::Completed.as_str(), "completed");

        assert_eq!(
            OnboardingStatus::from_string("registered"),
            Ok(OnboardingStatus::Registered)
        );
        assert_eq!(
            OnboardingStatus::from_string("completed"),
            Ok(OnboardingStatus::Completed)
        );

        // Legacy compatibility - old statuses map to completed
        assert_eq!(
            OnboardingStatus::from_string("verified"),
            Ok(OnboardingStatus::Completed)
        );
        assert_eq!(
            OnboardingStatus::from_string("plan_selected"),
            Ok(OnboardingStatus::Completed)
        );
        assert_eq!(
            OnboardingStatus::from_string("payment_pending"),
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
            onboarding_status: OnboardingStatus::Registered.as_str().to_string(),
        };

        assert_eq!(
            plan_selected_user.onboarding_status_enum().unwrap(),
            OnboardingStatus::Registered
        );
        assert!(!plan_selected_user.is_onboarding_complete());
        assert!(!plan_selected_user.needs_payment()); // OSS has no payments

        // Test another registered user (OSS - no payment needed)
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
            onboarding_status: OnboardingStatus::Registered.as_str().to_string(),
        };

        assert!(!plan_selected_free_user.needs_payment()); // OSS has no payments

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
