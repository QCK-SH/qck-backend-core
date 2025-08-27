-- Drop triggers
DROP TRIGGER IF EXISTS trigger_users_updated_at ON users;
DROP TRIGGER IF EXISTS trigger_links_updated_at ON links;

-- Drop function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop indexes
DROP INDEX IF EXISTS idx_refresh_tokens_active;
DROP INDEX IF EXISTS idx_refresh_tokens_expires_at;
DROP INDEX IF EXISTS idx_refresh_tokens_jti_hash;
DROP INDEX IF EXISTS idx_refresh_tokens_user_id;

DROP INDEX IF EXISTS idx_links_created_at;
DROP INDEX IF EXISTS idx_links_user_recent;
DROP INDEX IF EXISTS idx_links_custom_alias_active;
DROP INDEX IF EXISTS idx_links_short_code_active;

DROP INDEX IF EXISTS idx_users_subscription_created;
DROP INDEX IF EXISTS idx_users_unverified_emails;
DROP INDEX IF EXISTS idx_users_email_ci;

-- Drop tables
DROP TABLE IF EXISTS refresh_tokens;
DROP TABLE IF EXISTS links;
DROP TABLE IF EXISTS users;

-- Drop enum type
DROP TYPE IF EXISTS subscription_tier;

-- Drop extension
DROP EXTENSION IF EXISTS "uuid-ossp";