-- ============================================================================
-- ClickHouse Link Totals Table - Simple Link Summary
-- Description: One row per link with total statistics
-- Author: QCK Team
-- Date: 2025-08-12
-- Purpose: Fast lookups for link summary data (1 link UUID = 1 row)
-- ============================================================================

USE qck_analytics;

-- Single totals table for link summary stats
CREATE TABLE IF NOT EXISTS link_totals
(
    -- Identifiers
    link_id         UUID,
    
    -- Total counters
    total_clicks    UInt64,                    -- Total click count
    unique_visitors UInt64,                    -- Unique IP addresses
    total_users     UInt64,                    -- Logged-in user clicks
    total_bots      UInt64,                    -- Bot/crawler clicks
    
    -- Timestamps
    first_click     DateTime,                  -- First click timestamp
    last_click      DateTime,                  -- Most recent click
    
    -- Performance metrics
    avg_response_time UInt32                   -- Average response time
)
ENGINE = SummingMergeTree()
ORDER BY link_id
TTL last_click + INTERVAL 2 YEAR              -- Auto-cleanup after 2 years
SETTINGS 
    index_granularity = 8192;

-- Drop existing MV if it exists
DROP TABLE IF EXISTS link_totals_mv;

-- Materialized view that maintains link totals
CREATE MATERIALIZED VIEW link_totals_mv
TO link_totals
AS SELECT 
    link_id,
    count() as total_clicks,
    uniqExact(ip_address) as unique_visitors,
    countIf(user_id IS NOT NULL) as total_users,
    countIf(is_bot = true) as total_bots,
    min(timestamp) as first_click,
    max(timestamp) as last_click,
    round(avg(response_time)) as avg_response_time
FROM link_events
GROUP BY link_id;

-- ============================================================================
-- QUERY EXAMPLES FOR APPLICATION USE
-- ============================================================================

-- Get totals for a specific link (ultra-fast lookup)
-- SELECT 
--     link_id,
--     total_clicks,
--     unique_visitors,
--     total_users,
--     total_bots,
--     first_click,
--     last_click,
--     avg_response_time
-- FROM link_totals
-- WHERE link_id = {link_id:UUID};

-- Get top links by clicks
-- SELECT 
--     link_id,
--     total_clicks,
--     unique_visitors,
--     round(unique_visitors * 100.0 / total_clicks, 2) as conversion_rate,
--     round(total_bots * 100.0 / total_clicks, 2) as bot_percentage
-- FROM link_totals
-- ORDER BY total_clicks DESC
-- LIMIT 10;

-- Get links created today with activity
-- SELECT 
--     link_id,
--     total_clicks,
--     unique_visitors,
--     dateDiff('hour', first_click, last_click) as active_hours
-- FROM link_totals
-- WHERE toDate(first_click) = today()
-- ORDER BY total_clicks DESC;

-- ============================================================================
-- VALIDATION
-- ============================================================================

-- Check that link_totals table exists
SELECT 'link_totals table created' as status
WHERE exists(
    SELECT 1 FROM system.tables 
    WHERE database = 'qck_analytics' AND name = 'link_totals'
);

-- Check that MV exists
SELECT 'link_totals_mv created' as status
WHERE exists(
    SELECT 1 FROM system.tables 
    WHERE database = 'qck_analytics' AND name = 'link_totals_mv'
);

-- Summary of migration
SELECT 
    'Migration complete' as status,
    'link_totals and link_totals_mv created' as result,
    '1 link UUID = 1 row for fast lookups' as purpose;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================
-- Architecture: link_events → link_totals_mv → link_totals
-- Performance: 1 link = 1 row for instant lookups
-- Use Case: Dashboard summaries, link management UI
-- ============================================================================