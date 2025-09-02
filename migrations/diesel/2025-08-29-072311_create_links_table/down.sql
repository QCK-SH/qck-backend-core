-- Reverse the consolidated links table migration
-- This should restore the table to the state before this migration (no table at all)
-- Since this is the CREATE table migration, down should drop everything

DROP TRIGGER IF EXISTS links_updated_at_trigger ON links;
DROP FUNCTION IF EXISTS update_links_updated_at();

-- Drop all indexes (order matters for dependencies)
DROP INDEX IF EXISTS idx_links_has_og_image;
DROP INDEX IF EXISTS idx_links_processing_status;
DROP INDEX IF EXISTS idx_links_short_code_deleted;
DROP INDEX IF EXISTS idx_links_user_deleted;
DROP INDEX IF EXISTS idx_links_deleted_at;
DROP INDEX IF EXISTS idx_links_expires_at;
DROP INDEX IF EXISTS idx_links_is_active;
DROP INDEX IF EXISTS idx_links_created_at;
DROP INDEX IF EXISTS idx_links_short_code;
DROP INDEX IF EXISTS idx_links_user_id;

-- Drop the entire table since this migration creates it
DROP TABLE IF EXISTS links;