-- Enhance refresh_tokens table for token rotation support
-- Implements DEV-107: Build Refresh Token Rotation

-- Add new columns for token rotation features
-- Note: token_family will be set per-user for existing tokens to maintain consistency
ALTER TABLE refresh_tokens 
    ADD COLUMN IF NOT EXISTS token_family VARCHAR(64),
    ADD COLUMN IF NOT EXISTS issued_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS last_used_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS revoked_reason VARCHAR(255),
    ADD COLUMN IF NOT EXISTS device_fingerprint VARCHAR(255),
    ADD COLUMN IF NOT EXISTS ip_address INET,
    ADD COLUMN IF NOT EXISTS user_agent TEXT,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW();

-- Set token_family for existing tokens: one unique family per user for better organization
-- Each user gets a single clean UUID as their token family
WITH user_families AS (
    SELECT user_id, gen_random_uuid()::text AS family_id
    FROM refresh_tokens
    WHERE token_family IS NULL
    GROUP BY user_id
)
UPDATE refresh_tokens rt
SET token_family = uf.family_id
FROM user_families uf
WHERE rt.user_id = uf.user_id
  AND rt.token_family IS NULL;

-- Now make token_family NOT NULL after setting values
ALTER TABLE refresh_tokens 
    ALTER COLUMN token_family SET NOT NULL;

-- Create additional indexes for new columns
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_token_family ON refresh_tokens(token_family);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_active_v2 ON refresh_tokens(user_id, revoked_at, expires_at);

-- Add comments for documentation
COMMENT ON TABLE refresh_tokens IS 'Stores refresh tokens for JWT authentication with rotation support';
COMMENT ON COLUMN refresh_tokens.jti_hash IS 'SHA-256 hash of JWT ID for secure token lookup';
COMMENT ON COLUMN refresh_tokens.token_family IS 'Family ID to detect and invalidate reused tokens';
COMMENT ON COLUMN refresh_tokens.device_fingerprint IS 'Device identification for security tracking';
COMMENT ON COLUMN refresh_tokens.last_used_at IS 'Timestamp of last token usage for activity tracking';
COMMENT ON COLUMN refresh_tokens.revoked_reason IS 'Reason for token revocation (reuse_detected, user_logout, admin_action, etc.)';