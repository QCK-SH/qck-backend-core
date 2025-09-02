-- ============================================================================
-- ClickHouse Link Totals Table - Real-time Analytics with AggregatingMergeTree
-- Description: One row per link with aggregated statistics using state functions
-- Author: QCK Team
-- Date: 2025-09-01
-- Purpose: Fast lookups with real-time incremental aggregation (1 link UUID = 1 row)
-- Architecture: Uses AggregatingMergeTree with state functions for proper aggregation
-- ============================================================================

USE qck_analytics;

-- AggregatingMergeTree table with aggregation states for real-time analytics
CREATE TABLE IF NOT EXISTS link_totals
(
    link_id UUID,
    total_clicks AggregateFunction(sum, UInt64),
    unique_visitors AggregateFunction(uniq, IPv6),
    total_users AggregateFunction(sum, UInt64),
    total_bots AggregateFunction(sum, UInt64),
    first_click AggregateFunction(min, DateTime64(3, 'UTC')),
    last_click AggregateFunction(max, DateTime64(3, 'UTC')),
    avg_response_time AggregateFunction(avg, UInt32)
) 
ENGINE = AggregatingMergeTree()
ORDER BY link_id
SETTINGS index_granularity = 8192
COMMENT 'Real-time analytics using AggregatingMergeTree with state functions for proper incremental aggregation';

-- Drop existing MV if it exists
DROP TABLE IF EXISTS link_totals_mv;

-- Materialized view with state functions for incremental aggregation
CREATE MATERIALIZED VIEW link_totals_mv TO link_totals AS
SELECT
    link_id,
    sumState(toUInt64(1)) AS total_clicks,
    uniqState(ip_address) AS unique_visitors,
    sumState(CASE WHEN user_id IS NOT NULL THEN toUInt64(1) ELSE toUInt64(0) END) AS total_users,
    sumState(CASE WHEN is_bot = 1 THEN toUInt64(1) ELSE toUInt64(0) END) AS total_bots,
    minState(timestamp) AS first_click,
    maxState(timestamp) AS last_click,
    avgState(toUInt32(response_time)) AS avg_response_time
FROM link_events
GROUP BY link_id;

-- Populate table with any existing data from link_events
INSERT INTO link_totals
SELECT
    link_id,
    sumState(toUInt64(1)) AS total_clicks,
    uniqState(ip_address) AS unique_visitors,
    sumState(CASE WHEN user_id IS NOT NULL THEN toUInt64(1) ELSE toUInt64(0) END) AS total_users,
    sumState(CASE WHEN is_bot = 1 THEN toUInt64(1) ELSE toUInt64(0) END) AS total_bots,
    minState(timestamp) AS first_click,
    maxState(timestamp) AS last_click,
    avgState(toUInt32(response_time)) AS avg_response_time
FROM link_events
GROUP BY link_id;

-- ============================================================================
-- QUERY EXAMPLES FOR APPLICATION USE (AggregatingMergeTree)
-- ============================================================================

-- Get totals for a specific link (ultra-fast lookup with -Merge functions)
-- SELECT 
--     link_id,
--     sumMerge(total_clicks) as total_clicks,
--     uniqMerge(unique_visitors) as unique_visitors,
--     sumMerge(total_users) as total_users,
--     sumMerge(total_bots) as bot_clicks,
--     minMerge(first_click) as first_click,
--     maxMerge(last_click) as last_click,
--     avgMerge(avg_response_time) as avg_response_time
-- FROM link_totals
-- WHERE link_id = {link_id:UUID};

-- Get top links by clicks (with proper -Merge functions)
-- SELECT 
--     link_id,
--     sumMerge(total_clicks) as total_clicks,
--     uniqMerge(unique_visitors) as unique_visitors,
--     round(uniqMerge(unique_visitors) * 100.0 / sumMerge(total_clicks), 2) as conversion_rate,
--     round(sumMerge(total_bots) * 100.0 / sumMerge(total_clicks), 2) as bot_percentage
-- FROM link_totals
-- GROUP BY link_id
-- ORDER BY sumMerge(total_clicks) DESC
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
    'AggregatingMergeTree with state functions' as new_engine,
    count() as total_links_migrated
FROM link_totals;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================
-- Architecture: link_events → link_totals_mv → link_totals (AggregatingMergeTree)
-- Performance: Real-time incremental aggregation with state functions
-- Query Pattern: Use -Merge functions (sumMerge, uniqMerge, etc.)
-- Use Case: Real-time dashboard summaries, link management UI
-- ============================================================================