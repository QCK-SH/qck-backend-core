-- ClickHouse migration for URLhaus threat intelligence data
-- Simplified table for malicious URL security checks

-- Use the existing qck_analytics database
USE qck_analytics;

-- Drop old tables if they exist
DROP TABLE IF EXISTS urlhaus_stats;
DROP TABLE IF EXISTS active_threats;
DROP TABLE IF EXISTS urlhaus_threats;

-- Create simplified URLhaus threat table (only what we actually use)
CREATE TABLE IF NOT EXISTS urlhaus_threats (
    -- Essential fields for security checking
    url String,
    url_host String,  -- Domain for faster lookups
    threat_type String,  -- malware_download, phishing, etc.
    tags Array(String),  -- Malware families, campaigns
    url_status Enum8('online' = 1, 'offline' = 2, 'unknown' = 3),
    last_checked DateTime DEFAULT now(),
    
    -- Minimal metadata we actually need
    urlhaus_id UInt64,  -- Keep for deduplication
    first_seen DateTime,
    last_seen DateTime,
    reporter String,
    urlhaus_link String,
    
    -- Indexes for fast lookups
    INDEX url_idx url TYPE tokenbf_v1(10240, 3, 0) GRANULARITY 4,
    INDEX host_idx url_host TYPE tokenbf_v1(10240, 3, 0) GRANULARITY 4
) ENGINE = ReplacingMergeTree(last_checked)
ORDER BY (url, url_host)
TTL last_checked + INTERVAL 7 DAY  -- Auto-delete after 7 days (we refresh daily)
SETTINGS index_granularity = 8192;

-- Documentation: URLhaus malicious URL threat intelligence for security scanning
-- Updated daily from abuse.ch online threats feed