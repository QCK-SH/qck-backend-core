-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create subscription_tier enum
CREATE TYPE subscription_tier AS ENUM ('free', 'pro');

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(320) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    is_active BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    email_verified BOOLEAN DEFAULT FALSE NOT NULL,
    subscription_tier VARCHAR(50) DEFAULT 'free' NOT NULL,
    email_verified_at TIMESTAMPTZ
);

-- Links table
CREATE TABLE links (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    short_code VARCHAR(50) UNIQUE NOT NULL,
    original_url TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ,
    click_count INTEGER DEFAULT 0 NOT NULL,
    is_active BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    custom_alias VARCHAR(50) UNIQUE,
    last_accessed_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    metadata JSONB DEFAULT '{}' NOT NULL,
    
    CONSTRAINT check_short_code_length CHECK (LENGTH(short_code) >= 3),
    CONSTRAINT check_click_count CHECK (click_count >= 0)
);

-- Refresh tokens table
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    jti_hash VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    
    CONSTRAINT check_expires_after_created CHECK (expires_at > created_at),
    CONSTRAINT check_revoked_after_created CHECK (revoked_at IS NULL OR revoked_at >= created_at)
);

-- Create indexes
CREATE INDEX idx_users_email_ci ON users(LOWER(email));
CREATE INDEX idx_users_unverified_emails ON users(email) WHERE email_verified IS NOT TRUE;
CREATE INDEX idx_users_subscription_created ON users(subscription_tier, created_at DESC);

CREATE UNIQUE INDEX idx_links_short_code_active ON links(short_code) WHERE is_active = true;
CREATE UNIQUE INDEX idx_links_custom_alias_active ON links(custom_alias) WHERE custom_alias IS NOT NULL AND is_active = true;
CREATE INDEX idx_links_user_recent ON links(user_id, created_at DESC) WHERE is_active = true;
CREATE INDEX idx_links_created_at ON links(created_at DESC);

CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_jti_hash ON refresh_tokens(jti_hash);
CREATE INDEX idx_refresh_tokens_expires_at ON refresh_tokens(expires_at);
CREATE INDEX idx_refresh_tokens_active ON refresh_tokens(user_id, expires_at) WHERE revoked_at IS NULL;

-- Create update trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Add update triggers
CREATE TRIGGER trigger_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_links_updated_at
    BEFORE UPDATE ON links
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();