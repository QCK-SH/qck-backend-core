-- Create consolidated links table for URL shortening
-- DEV-105: Complete link management schema
-- Consolidates: create_links_table + add_deleted_at + remove_redundant_columns + add_og_image_favicon + add_processing_status

-- Drop the old links table from initial_schema if it exists
DROP TABLE IF EXISTS links CASCADE;

-- Create the new consolidated links table
CREATE TABLE links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    short_code VARCHAR(20) NOT NULL UNIQUE,
    original_url TEXT NOT NULL,
    title VARCHAR(500),
    description TEXT,
    tags TEXT[], -- Array of tags
    custom_alias VARCHAR(100),
    is_active BOOLEAN NOT NULL DEFAULT true,
    expires_at TIMESTAMPTZ,
    password_hash TEXT, -- For password-protected links
    last_accessed_at TIMESTAMPTZ,
    
    -- Rich metadata fields (DEV-95)
    og_image TEXT,
    favicon_url TEXT,
    
    -- Processing status for async metadata extraction
    processing_status VARCHAR(20) DEFAULT 'pending' NOT NULL,
    metadata_extracted_at TIMESTAMPTZ,
    
    -- UTM campaign tracking
    utm_source VARCHAR(255),
    utm_medium VARCHAR(255),
    utm_campaign VARCHAR(255),
    utm_term VARCHAR(255),
    utm_content VARCHAR(255),
    
    -- Soft delete support
    deleted_at TIMESTAMPTZ DEFAULT NULL,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_links_user_id ON links(user_id);
CREATE INDEX IF NOT EXISTS idx_links_short_code ON links(short_code);
CREATE INDEX IF NOT EXISTS idx_links_created_at ON links(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_links_is_active ON links(is_active) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_links_expires_at ON links(expires_at) WHERE expires_at IS NOT NULL;

-- Soft delete indexes
CREATE INDEX IF NOT EXISTS idx_links_deleted_at ON links(deleted_at);
CREATE INDEX IF NOT EXISTS idx_links_user_deleted ON links(user_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_links_short_code_deleted ON links(short_code, deleted_at);

-- Processing status index
CREATE INDEX IF NOT EXISTS idx_links_processing_status ON links(processing_status);

-- Rich metadata index
CREATE INDEX IF NOT EXISTS idx_links_has_og_image ON links(id) WHERE og_image IS NOT NULL;

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_links_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER links_updated_at_trigger
    BEFORE UPDATE ON links
    FOR EACH ROW
    EXECUTE FUNCTION update_links_updated_at();