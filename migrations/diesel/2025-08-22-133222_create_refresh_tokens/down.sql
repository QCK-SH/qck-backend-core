-- Revert refresh_tokens table enhancements
ALTER TABLE refresh_tokens 
    DROP COLUMN IF EXISTS token_family,
    DROP COLUMN IF EXISTS issued_at,
    DROP COLUMN IF EXISTS last_used_at,
    DROP COLUMN IF EXISTS revoked_reason,
    DROP COLUMN IF EXISTS device_fingerprint,
    DROP COLUMN IF EXISTS ip_address,
    DROP COLUMN IF EXISTS user_agent,
    DROP COLUMN IF EXISTS updated_at;

DROP INDEX IF EXISTS idx_refresh_tokens_token_family;
DROP INDEX IF EXISTS idx_refresh_tokens_user_active_v2;