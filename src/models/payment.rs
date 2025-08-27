use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::payments;

#[derive(
    Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable, AsChangeset,
)]
#[diesel(table_name = payments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Payment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_customer_id: Option<String>,
    pub provider_payment_id: Option<String>,
    pub provider_subscription_id: Option<String>,
    pub amount: i32, // Amount in cents (e.g., 999 for $9.99/month)
    pub currency: String,
    pub status: String,
    pub payment_method: Option<String>,
    pub subscription_tier: String, // 'pro' for paid monthly subscriptions
    pub billing_period: Option<String>, // 'monthly' for recurring subscriptions
    pub metadata: Option<JsonValue>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub refunded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = payments)]
pub struct NewPayment {
    pub user_id: Uuid,
    pub provider: String,
    pub provider_customer_id: Option<String>,
    pub provider_payment_id: Option<String>,
    pub provider_subscription_id: Option<String>,
    pub amount: i32,
    pub currency: String,
    pub status: String,
    pub payment_method: Option<String>,
    pub subscription_tier: String,
    pub billing_period: Option<String>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
    Refunded,
    PartiallyRefunded,
}

impl PaymentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Processing => "processing",
            PaymentStatus::Completed => "completed",
            PaymentStatus::Failed => "failed",
            PaymentStatus::Cancelled => "cancelled",
            PaymentStatus::Refunded => "refunded",
            PaymentStatus::PartiallyRefunded => "partially_refunded",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(PaymentStatus::Pending),
            "processing" => Some(PaymentStatus::Processing),
            "completed" => Some(PaymentStatus::Completed),
            "failed" => Some(PaymentStatus::Failed),
            "cancelled" => Some(PaymentStatus::Cancelled),
            "refunded" => Some(PaymentStatus::Refunded),
            "partially_refunded" => Some(PaymentStatus::PartiallyRefunded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentProvider {
    Stripe,
    PayPal,
    Manual,
    Test,
}

impl PaymentProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentProvider::Stripe => "stripe",
            PaymentProvider::PayPal => "paypal",
            PaymentProvider::Manual => "manual",
            PaymentProvider::Test => "test",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "stripe" => Some(PaymentProvider::Stripe),
            "paypal" => Some(PaymentProvider::PayPal),
            "manual" => Some(PaymentProvider::Manual),
            "test" => Some(PaymentProvider::Test),
            _ => None,
        }
    }
}

impl Payment {
    pub async fn find_by_user_id(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        use crate::schema::payments::dsl;

        dsl::payments
            .filter(dsl::user_id.eq(user_id))
            .order(dsl::created_at.desc())
            .load::<Self>(conn)
            .await
    }

    pub async fn find_latest_by_user_id(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
    ) -> Result<Option<Self>, diesel::result::Error> {
        use crate::schema::payments::dsl;

        dsl::payments
            .filter(dsl::user_id.eq(user_id))
            .order(dsl::created_at.desc())
            .first::<Self>(conn)
            .await
            .optional()
    }

    pub async fn find_completed_by_user_id(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        use crate::schema::payments::dsl;

        dsl::payments
            .filter(dsl::user_id.eq(user_id))
            .filter(dsl::status.eq(PaymentStatus::Completed.as_str()))
            .order(dsl::completed_at.desc())
            .load::<Self>(conn)
            .await
    }

    pub async fn update_status(
        &mut self,
        conn: &mut AsyncPgConnection,
        new_status: PaymentStatus,
    ) -> Result<(), diesel::result::Error> {
        use crate::schema::payments::dsl;
        use chrono::Utc;

        self.status = new_status.as_str().to_string();
        self.updated_at = Utc::now();

        // Update completion timestamps based on status
        match new_status {
            PaymentStatus::Completed => {
                self.completed_at = Some(Utc::now());
            },
            PaymentStatus::Failed => {
                self.failed_at = Some(Utc::now());
            },
            PaymentStatus::Refunded | PaymentStatus::PartiallyRefunded => {
                self.refunded_at = Some(Utc::now());
            },
            _ => {},
        }

        diesel::update(dsl::payments.find(self.id))
            .set(&*self)
            .execute(conn)
            .await?;

        Ok(())
    }
}
