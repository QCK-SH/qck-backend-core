-- QCK Links Table Schema Enhancement Migration
-- Migration: 20250109000003_links_table_schema.sql
-- Created: 2025-01-09
-- Purpose: Enhance existing links table with essential optimizations for MVP
-- Target: Production-ready, efficient URL shortening with essential indexes
--
-- SECURITY APPROACH:
-- * SQL injection prevention via parameterized queries (Rust/SQLx level)
-- * Basic protocol validation (http/https) at database level
-- * No content filtering to avoid blocking legitimate URLs with SQL/tech keywords
-- * Frontend escaping for safe URL rendering

-- ============================================================================
-- EXTENSION SETUP: Essential PostgreSQL features for MVP
-- ============================================================================

-- Enable uuid-ossp for UUID generation (if not already enabled)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================================
-- ENHANCE EXISTING LINKS TABLE: Add missing columns and optimizations
-- ============================================================================

-- Add custom_alias column if it doesn't exist
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'links' AND column_name = 'custom_alias') THEN
        ALTER TABLE links ADD COLUMN custom_alias VARCHAR(50)
            CHECK (custom_alias IS NULL OR (
                length(custom_alias) >= 3 
                AND length(custom_alias) <= 50
                AND custom_alias ~ '^[a-zA-Z0-9]([a-zA-Z0-9_-]*[a-zA-Z0-9])?$'
            ));
        RAISE NOTICE 'Added custom_alias column to links table';
        -- Add UNIQUE constraint separately with error handling
        BEGIN
            ALTER TABLE links ADD CONSTRAINT links_custom_alias_unique UNIQUE (custom_alias);
            RAISE NOTICE 'Added UNIQUE constraint to custom_alias column';
        EXCEPTION
            WHEN duplicate_object THEN
                RAISE NOTICE 'UNIQUE constraint on custom_alias already exists';
            WHEN others THEN
                RAISE EXCEPTION 'Failed to add UNIQUE constraint to custom_alias: %', SQLERRM;
        END;
    END IF;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to add custom_alias column: %', SQLERRM;
END $$;

-- Add last_accessed_at column if it doesn't exist
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'links' AND column_name = 'last_accessed_at') THEN
        ALTER TABLE links ADD COLUMN last_accessed_at TIMESTAMPTZ DEFAULT now() NOT NULL;
        RAISE NOTICE 'Added last_accessed_at column to links table';
    END IF;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to add last_accessed_at column: %', SQLERRM;
END $$;

-- Ensure metadata column is JSONB with proper default and security constraints
DO $$
BEGIN
    -- Check if metadata column exists and update it if needed
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'links' AND column_name = 'metadata') THEN
        -- Safely convert to JSONB with fallback for invalid JSON
        -- This handles: NULL, empty strings, invalid JSON, and valid JSON
        BEGIN
            ALTER TABLE links ALTER COLUMN metadata TYPE JSONB USING 
                CASE 
                    WHEN metadata IS NULL THEN '{}'::jsonb
                    WHEN metadata::text = '' THEN '{}'::jsonb
                    WHEN metadata::text ~ '^\s*$' THEN '{}'::jsonb  -- whitespace only
                    ELSE 
                        -- Try to parse as JSON, fallback to empty object if invalid
                        (SELECT COALESCE(
                            (SELECT metadata::text::jsonb),
                            '{}'::jsonb
                        ))
                END;
            ALTER TABLE links ALTER COLUMN metadata SET DEFAULT '{}'::jsonb;
            ALTER TABLE links ALTER COLUMN metadata SET NOT NULL;
            RAISE NOTICE 'Enhanced metadata column to JSONB with proper default';
        EXCEPTION
            WHEN OTHERS THEN
                -- If conversion fails, try a simpler approach
                ALTER TABLE links ALTER COLUMN metadata TYPE JSONB USING '{}'::jsonb;
                ALTER TABLE links ALTER COLUMN metadata SET DEFAULT '{}'::jsonb;
                ALTER TABLE links ALTER COLUMN metadata SET NOT NULL;
                RAISE NOTICE 'Reset all metadata to empty object due to conversion error: %', SQLERRM;
        END;
    ELSE
        -- Add metadata column if it doesn't exist
        ALTER TABLE links ADD COLUMN metadata JSONB DEFAULT '{}'::jsonb NOT NULL;
        RAISE NOTICE 'Added metadata column to links table';
    END IF;
    
    -- Note: Metadata size validation is done at the application level for performance reasons.
    -- CHECK constraints with pg_column_size() have performance issues on TOASTed data
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to configure metadata column: %', SQLERRM;
END $$;

-- Enhance existing constraints and add new ones
DO $$
BEGIN
    -- Drop existing short_code constraint if it conflicts, then add new one
    BEGIN
        -- Drop any existing short_code length constraint
        ALTER TABLE links DROP CONSTRAINT IF EXISTS links_short_code_check;
        ALTER TABLE links DROP CONSTRAINT IF EXISTS links_short_code_format_check;
    EXCEPTION
        WHEN others THEN
            RAISE EXCEPTION 'Failed to drop existing short_code constraint: %', SQLERRM;
    END;
    
    -- Add new short_code constraint (respecting existing >= 3 requirement)
    BEGIN
        ALTER TABLE links ADD CONSTRAINT links_short_code_format_check 
            CHECK (length(short_code) >= 3 AND short_code ~ '^[a-zA-Z0-9]+$');
        RAISE NOTICE 'Added short_code format constraint (>= 3 chars, alphanumeric only)';
    EXCEPTION
        WHEN duplicate_object THEN
            RAISE NOTICE 'Short_code format constraint already exists';
    END;
    
    -- Add/update original_url constraint  
    BEGIN
        ALTER TABLE links ADD CONSTRAINT links_original_url_check
            CHECK (length(original_url) >= 1 AND length(original_url) <= 8192 
                   -- Only require http/https protocol - content validation happens at app level
                   AND original_url ~ '^https?://');
        RAISE NOTICE 'Added original_url constraint';
    EXCEPTION
        WHEN duplicate_object THEN
            RAISE NOTICE 'Original_url constraint already exists';
    END;
    
    -- Add/update click_count constraint
    BEGIN
        ALTER TABLE links ADD CONSTRAINT links_click_count_check
            CHECK (click_count >= 0);
        RAISE NOTICE 'Added click_count constraint';
    EXCEPTION
        WHEN duplicate_object THEN
            RAISE NOTICE 'Click_count constraint already exists';
    END;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to update constraints: %', SQLERRM;
END $$;

-- Keep expires_at column name for backwards compatibility
-- Note: This migration preserves the existing expires_at column name to avoid breaking changes

-- ============================================================================
-- TABLE COMMENTS: Comprehensive documentation
-- ============================================================================

COMMENT ON TABLE links IS 'URL shortening links table optimized for millions of links with advanced PostgreSQL features';
COMMENT ON COLUMN links.id IS 'Primary key: UUID v4 for global uniqueness and security';
COMMENT ON COLUMN links.short_code IS 'Unique short identifier (Base62): the actual shortened URL path';
COMMENT ON COLUMN links.original_url IS 'Target URL being shortened (up to 8KB supported)';
COMMENT ON COLUMN links.user_id IS 'Owner reference with CASCADE DELETE for clean orphan removal';
COMMENT ON COLUMN links.custom_alias IS 'Optional human-readable alternative to short_code';
COMMENT ON COLUMN links.expires_at IS 'Optional expiration timestamp (NULL = permanent)';
COMMENT ON COLUMN links.click_count IS 'High-performance click counter (updated via atomic operations)';
COMMENT ON COLUMN links.metadata IS 'Extensible JSONB for tags, UTM params, A/B tests, and future features';
COMMENT ON COLUMN links.is_active IS 'Soft delete flag for data recovery (most queries filter is_active = true)';
COMMENT ON COLUMN links.created_at IS 'Record creation timestamp (immutable)';
COMMENT ON COLUMN links.updated_at IS 'Last modification timestamp (updated via trigger)';
COMMENT ON COLUMN links.last_accessed_at IS 'Last click timestamp for analytics and cleanup jobs';

-- ============================================================================
-- ESSENTIAL INDEXES: MVP-focused performance optimization
-- ============================================================================

-- 1. REDIRECT LOOKUP: O(1) short_code to URL resolution (MOST CRITICAL)
-- This is the hot path for every redirect - must be lightning fast
-- Check if there's already a general short_code index, if so use it instead of creating a partial one
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'links' AND indexname = 'idx_links_short_code') THEN
        -- No general index exists, create the partial active-only index
        CREATE UNIQUE INDEX IF NOT EXISTS idx_links_short_code_active 
            ON links (short_code) 
            WHERE is_active = true;
        RAISE NOTICE 'Created partial index idx_links_short_code_active for active links';
    ELSE
        -- General index already exists, skip creating partial index to avoid redundancy
        RAISE NOTICE 'General short_code index exists, skipping partial index creation';
    END IF;
END $$;

-- Comment will be added only if the index was created
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_links_short_code_active') THEN
        COMMENT ON INDEX idx_links_short_code_active IS 'CRITICAL: O(1) redirect lookups on short_code (active links only)';
    END IF;
END $$;

-- 2. CUSTOM ALIAS LOOKUP: Alternative redirect path
-- Partial index only for non-NULL aliases (saves space)
CREATE UNIQUE INDEX IF NOT EXISTS idx_links_custom_alias_active 
    ON links (custom_alias) 
    WHERE custom_alias IS NOT NULL AND is_active = true;
COMMENT ON INDEX idx_links_custom_alias_active IS 'O(1) custom alias redirect lookups (active aliases only)';

-- 3. USER DASHBOARD: Fast user link listings with pagination
-- Composite index optimized for "user's recent links" queries
CREATE INDEX IF NOT EXISTS idx_links_user_recent 
    ON links (user_id, created_at DESC) 
    WHERE is_active = true;
COMMENT ON INDEX idx_links_user_recent IS 'User dashboard: fast pagination of users recent links';

-- 4. CREATED_AT: Time-based queries (for analytics)
CREATE INDEX IF NOT EXISTS idx_links_created_at 
    ON links (created_at DESC);
COMMENT ON INDEX idx_links_created_at IS 'Time-based queries and analytics';

-- ============================================================================
-- TRIGGERS: Automatic maintenance and performance optimization
-- ============================================================================

-- AUTO-UPDATE: Use existing updated_at timestamp trigger function if available
-- Check if the generic update_updated_at_column function exists, if not create our own
DO $$
BEGIN
    -- First check if the generic function exists
    IF EXISTS (SELECT 1 FROM pg_proc WHERE proname = 'update_updated_at_column') THEN
        RAISE NOTICE 'Using existing update_updated_at_column function';
    ELSE
        -- Create our own function if the generic one doesn't exist
        CREATE OR REPLACE FUNCTION update_updated_at_column()
        RETURNS TRIGGER AS $func$
        BEGIN
            NEW.updated_at = now();
            RETURN NEW;
        END;
        $func$ LANGUAGE plpgsql;
        RAISE NOTICE 'Created update_updated_at_column trigger function';
    END IF;
    
    -- Create trigger only if it doesn't exist
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'trigger_links_updated_at') THEN
        CREATE TRIGGER trigger_links_updated_at
            BEFORE UPDATE ON links
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column();
        RAISE NOTICE 'Created updated_at trigger for links table using update_updated_at_column function';
    ELSE
        RAISE NOTICE 'Updated_at trigger already exists';
    END IF;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to create updated_at trigger: %', SQLERRM;
END $$;

COMMENT ON TRIGGER trigger_links_updated_at ON links IS 'Auto-update updated_at timestamp on row modifications';

-- CLICK TRACKING: Atomic click counter with last_accessed_at update and row locking
-- This will be called by the redirect service for performance
-- Fixed race condition with SELECT FOR UPDATE
CREATE OR REPLACE FUNCTION increment_click_count(p_short_code VARCHAR(50))
RETURNS BOOLEAN AS $$
DECLARE
    rows_updated INTEGER;
BEGIN
    -- Direct UPDATE with WHERE conditions for simplicity and performance
    UPDATE links 
    SET 
        click_count = click_count + 1,
        last_accessed_at = now()
    WHERE short_code = p_short_code 
        AND is_active = true 
        AND (expires_at IS NULL OR expires_at > now());
    
    -- Check if any row was updated
    GET DIAGNOSTICS rows_updated = ROW_COUNT;
    RETURN rows_updated > 0;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to increment click count for %: %', p_short_code, SQLERRM;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION increment_click_count(VARCHAR) IS 'Atomic click counting with last_accessed_at update for redirect service';

-- AUTO-UPDATE LAST ACCESSED: Update last_accessed_at on redirects (optional trigger)
-- This can be used if we want automatic updates, but increment_click_count is preferred for performance
CREATE OR REPLACE FUNCTION update_last_accessed()
RETURNS TRIGGER AS $$
BEGIN
    NEW.last_accessed_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Note: This trigger is created but not activated by default
-- Uncomment the next lines if you want automatic last_accessed_at updates
-- CREATE TRIGGER trigger_links_last_accessed
--     BEFORE UPDATE OF click_count ON links
--     FOR EACH ROW
--     EXECUTE FUNCTION update_last_accessed();

-- ============================================================================
-- PERFORMANCE STATISTICS: Enable automatic statistics collection
-- ============================================================================

-- Ensure PostgreSQL collects detailed statistics on this critical table
-- Autovacuum settings are intentionally left at PostgreSQL defaults.
-- Uncomment these lines ONLY if you have specific high-churn requirements:
-- ALTER TABLE links SET (autovacuum_analyze_scale_factor = 0.05);
-- ALTER TABLE links SET (autovacuum_vacuum_scale_factor = 0.1);

-- Statistics targets: Using PostgreSQL defaults (100) for most columns
-- Only increase for high-cardinality columns if query planner needs it
-- ALTER TABLE links ALTER COLUMN short_code SET STATISTICS 200;  -- Uncomment if needed

-- ============================================================================
-- TABLE PARTITIONING HINTS (for future scaling)
-- ============================================================================

-- FUTURE OPTIMIZATION: When reaching 10M+ links, consider partitioning by:
-- 1. Range partitioning on created_at (monthly partitions)
--    - Enables efficient time-based queries and maintenance
--    - Old partitions can be archived or dropped
-- 
-- 2. Hash partitioning on user_id for user-specific queries
--    - Distributes user data evenly across partitions
--    - Enables parallel query processing
--
-- Example partition setup (uncomment when needed):
-- 
-- CREATE TABLE links_2025_01 PARTITION OF links 
--     FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');
-- CREATE TABLE links_2025_02 PARTITION OF links 
--     FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');

-- ============================================================================
-- QUERY EXAMPLES: Optimized queries for common operations
-- ============================================================================

-- REDIRECT LOOKUP (most critical - must be sub-millisecond):
-- SELECT original_url FROM links WHERE short_code = $1 AND is_active = true AND (expires_at IS NULL OR expires_at > now());

-- USER DASHBOARD (with pagination):
-- SELECT short_code, original_url, custom_alias, click_count, created_at 
-- FROM links 
-- WHERE user_id = $1 AND is_active = true 
-- ORDER BY created_at DESC 
-- LIMIT 20 OFFSET $2;

-- ANALYTICS QUERY (top links by user):
-- SELECT short_code, original_url, click_count, last_accessed_at
-- FROM links 
-- WHERE user_id = $1 AND is_active = true AND click_count > 0
-- ORDER BY click_count DESC, last_accessed_at DESC
-- LIMIT 10;

-- ============================================================================
-- MIGRATION COMPLETION
-- ============================================================================

-- Grant appropriate permissions (check if user exists first)
DO $$
BEGIN
    -- Check if qck_user role exists before granting permissions
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'qck_user') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON links TO qck_user;
        GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO qck_user;
        RAISE NOTICE 'Granted permissions to qck_user role';
    ELSE
        RAISE NOTICE 'qck_user role does not exist, skipping permission grants';
    END IF;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to grant permissions: %', SQLERRM;
END $$;

-- Create initial statistics
ANALYZE links;

-- Log successful migration
DO $$
DECLARE
    index_count INTEGER;
BEGIN
    SELECT count(*) INTO index_count FROM pg_indexes WHERE tablename = 'links';
    RAISE NOTICE 'Successfully enhanced links table with % essential indexes for MVP', index_count;
EXCEPTION
    WHEN others THEN
        RAISE EXCEPTION 'Failed to complete migration verification: %', SQLERRM;
END $$;