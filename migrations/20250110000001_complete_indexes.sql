-- Migration: Complete Database Indexes (DEV-89)
-- Description: Add missing performance indexes for expires_at columns
-- Author: QCK Team
-- Date: 2025-01-10

-- ============================================================================
-- LINKS TABLE INDEXES
-- ============================================================================

-- Index for expiring links cleanup
-- This helps with queries that find and clean up expired links, regardless of their active status
-- Note: Using WHERE expires_at IS NOT NULL instead of WHERE expires_at < NOW() because:
--   1. NOW() would create a moving target that changes constantly
--   2. We need to efficiently query both expired AND future expiring links
--   3. The index remains stable and useful for range queries like "expires_at < NOW()"
CREATE INDEX IF NOT EXISTS idx_links_expires_at
    ON links (expires_at)
    WHERE expires_at IS NOT NULL;

COMMENT ON INDEX idx_links_expires_at IS 'Index for efficiently finding expired links (regardless of active status) that need cleanup';

-- ============================================================================
-- USER_SESSIONS TABLE INDEXES
-- ============================================================================

-- Index for session cleanup
-- This helps with queries that find and remove expired sessions
-- Note: Using WHERE expires_at IS NOT NULL for the same reasons as links table:
--   - Stable index that doesn't change with time
--   - Supports both expired and soon-to-expire session queries
--   - Efficient for range scans with "expires_at < NOW()" predicates
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires_at
    ON user_sessions (expires_at)
    WHERE expires_at IS NOT NULL;

COMMENT ON INDEX idx_user_sessions_expires_at IS 'Index for efficiently finding expired sessions for cleanup';

-- ============================================================================
-- PERFORMANCE VERIFICATION
-- ============================================================================

-- Analyze tables to update statistics after index creation
ANALYZE links;
ANALYZE user_sessions;

-- Database indexes completed successfully (DEV-89)
-- Created indexes: idx_links_expires_at, idx_user_sessions_expires_at
-- Tables analyzed for optimal query planning