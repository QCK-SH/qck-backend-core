// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use diesel::pg::sql_types::*;

    links (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 20]
        short_code -> Varchar,
        original_url -> Text,
        #[max_length = 500]
        title -> Nullable<Varchar>,
        description -> Nullable<Text>,
        tags -> Nullable<Array<Nullable<Text>>>,
        #[max_length = 100]
        custom_alias -> Nullable<Varchar>,
        is_active -> Bool,
        expires_at -> Nullable<Timestamptz>,
        password_hash -> Nullable<Text>,
        last_accessed_at -> Nullable<Timestamptz>,
        og_image -> Nullable<Text>,
        favicon_url -> Nullable<Text>,
        #[max_length = 20]
        processing_status -> Varchar,
        metadata_extracted_at -> Nullable<Timestamptz>,
        #[max_length = 255]
        utm_source -> Nullable<Varchar>,
        #[max_length = 255]
        utm_medium -> Nullable<Varchar>,
        #[max_length = 255]
        utm_campaign -> Nullable<Varchar>,
        #[max_length = 255]
        utm_term -> Nullable<Varchar>,
        #[max_length = 255]
        utm_content -> Nullable<Varchar>,
        deleted_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel::pg::sql_types::*;

    password_reset_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        token_hash -> Varchar,
        expires_at -> Timestamptz,
        used_at -> Nullable<Timestamptz>,
        created_at -> Nullable<Timestamptz>,
        ip_address -> Nullable<Text>,
        user_agent -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel::pg::sql_types::*;

    payments (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 50]
        provider -> Varchar,
        #[max_length = 255]
        provider_customer_id -> Nullable<Varchar>,
        #[max_length = 255]
        provider_payment_id -> Nullable<Varchar>,
        #[max_length = 255]
        provider_subscription_id -> Nullable<Varchar>,
        amount -> Int4,
        #[max_length = 3]
        currency -> Varchar,
        #[max_length = 50]
        status -> Varchar,
        #[max_length = 50]
        payment_method -> Nullable<Varchar>,
        #[max_length = 50]
        subscription_tier -> Varchar,
        #[max_length = 20]
        billing_period -> Nullable<Varchar>,
        metadata -> Nullable<Jsonb>,
        failure_reason -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
        failed_at -> Nullable<Timestamptz>,
        refunded_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel::pg::sql_types::*;

    refresh_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        jti_hash -> Varchar,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
        revoked_at -> Nullable<Timestamptz>,
        #[max_length = 64]
        token_family -> Varchar,
        issued_at -> Timestamptz,
        last_used_at -> Nullable<Timestamptz>,
        #[max_length = 255]
        revoked_reason -> Nullable<Varchar>,
        #[max_length = 255]
        device_fingerprint -> Nullable<Varchar>,
        ip_address -> Nullable<Text>,
        user_agent -> Nullable<Text>,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel::pg::sql_types::*;

    users (id) {
        id -> Uuid,
        #[max_length = 320]
        email -> Varchar,
        password_hash -> Text,
        is_active -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        email_verified -> Bool,
        #[max_length = 50]
        subscription_tier -> Varchar,
        email_verified_at -> Nullable<Timestamptz>,
        #[max_length = 255]
        full_name -> Varchar,
        #[max_length = 255]
        company_name -> Nullable<Varchar>,
        #[max_length = 50]
        onboarding_status -> Varchar,
    }
}

diesel::joinable!(links -> users (user_id));
diesel::joinable!(password_reset_tokens -> users (user_id));
diesel::joinable!(payments -> users (user_id));
diesel::joinable!(refresh_tokens -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    links,
    password_reset_tokens,
    payments,
    refresh_tokens,
    users,
);
