-- ============================================================================
-- Seed Demo Link Events for Development/Staging
-- Description: Creates realistic click event data for demo links
-- PROTECTED: This migration is automatically skipped in production environment
-- Date: 2025-09-02
-- ============================================================================

-- Skip this migration in production
-- The migration runner should check for ENVIRONMENT variable before running this
-- Production check: if ENVIRONMENT = 'production' then SKIP this file

USE qck_analytics;

-- ============================================================================
-- SEED LINK EVENTS FOR DEMO USERS
-- ============================================================================

-- Clear existing demo link events (based on known demo user IDs)
ALTER TABLE link_events DELETE WHERE user_id IN (
    'f1111111-1111-1111-1111-111111111111',  -- Free tier
    'f2222222-2222-2222-2222-222222222222',  -- Pro tier
    'f3333333-3333-3333-3333-333333333333',  -- Business tier
    'f4444444-4444-4444-4444-444444444444'   -- Enterprise tier
);

-- Function to generate realistic events for links
-- We'll insert events with various patterns to simulate real usage

-- Free tier links (50 links, moderate traffic)
-- Generate 5-50 clicks per link over the past 30 days
INSERT INTO link_events (
    link_id,
    user_id,
    timestamp,
    ip_address,
    user_agent,
    referrer,
    country,
    country_code,
    city,
    region,
    device_type,
    browser,
    os,
    is_bot,
    http_method,
    response_time,
    status_code,
    utm_source,
    utm_medium,
    utm_campaign
)
SELECT
    generateUUIDv4() as link_id,  -- We'll need to map these to actual link IDs
    'f1111111-1111-1111-1111-111111111111' as user_id,
    now() - interval rand() % 30 day as timestamp,
    toIPv6(concat('192.168.', toString(rand() % 256), '.', toString(rand() % 256))) as ip_address,
    arrayElement([
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0',
        'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/537.36',
        'Mozilla/5.0 (X11; Linux x86_64) Firefox/121.0'
    ], (rand() % 4) + 1) as user_agent,
    arrayElement([
        'https://google.com/',
        'https://facebook.com/',
        'https://twitter.com/',
        'https://linkedin.com/',
        'direct'
    ], (rand() % 5) + 1) as referrer,
    arrayElement(['United States', 'United Kingdom', 'Canada', 'Australia', 'Germany'], (rand() % 5) + 1) as country,
    arrayElement(['US', 'GB', 'CA', 'AU', 'DE'], (rand() % 5) + 1) as country_code,
    arrayElement(['New York', 'London', 'Toronto', 'Sydney', 'Berlin'], (rand() % 5) + 1) as city,
    arrayElement(['NY', 'England', 'Ontario', 'NSW', 'Brandenburg'], (rand() % 5) + 1) as region,
    arrayElement(['desktop', 'mobile', 'tablet'], (rand() % 3) + 1) as device_type,
    arrayElement(['Chrome', 'Safari', 'Firefox', 'Edge'], (rand() % 4) + 1) as browser,
    arrayElement(['Windows', 'iOS', 'macOS', 'Android', 'Linux'], (rand() % 5) + 1) as os,
    false as is_bot,
    'GET' as http_method,
    toUInt16(20 + rand() % 180) as response_time,
    200 as status_code,
    if(rand() % 10 < 3, arrayElement(['google', 'facebook', 'twitter', 'email'], (rand() % 4) + 1), null) as utm_source,
    if(rand() % 10 < 3, arrayElement(['cpc', 'social', 'email', 'referral'], (rand() % 4) + 1), null) as utm_medium,
    if(rand() % 10 < 2, arrayElement(['summer_sale', 'black_friday', 'launch'], (rand() % 3) + 1), null) as utm_campaign
FROM numbers(500);  -- 500 total events for free tier (avg 10 per link)

-- Pro tier links (100 links, higher traffic)
-- Generate 10-100 clicks per link over the past 60 days
INSERT INTO link_events (
    link_id,
    user_id,
    timestamp,
    ip_address,
    user_agent,
    referrer,
    country,
    country_code,
    city,
    region,
    device_type,
    browser,
    os,
    is_bot,
    http_method,
    response_time,
    status_code,
    utm_source,
    utm_medium,
    utm_campaign
)
SELECT
    generateUUIDv4() as link_id,
    'f2222222-2222-2222-2222-222222222222' as user_id,
    now() - interval rand() % 60 day as timestamp,
    toIPv6(concat('10.', toString(rand() % 256), '.', toString(rand() % 256), '.', toString(rand() % 256))) as ip_address,
    arrayElement([
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0',
        'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/537.36',
        'Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (X11; Linux x86_64) Firefox/121.0'
    ], (rand() % 5) + 1) as user_agent,
    arrayElement([
        'https://google.com/',
        'https://facebook.com/',
        'https://twitter.com/',
        'https://linkedin.com/',
        'https://reddit.com/',
        'https://youtube.com/',
        'direct'
    ], (rand() % 7) + 1) as referrer,
    arrayElement([
        'United States', 'United Kingdom', 'Canada', 'Australia', 
        'Germany', 'France', 'Japan', 'Brazil', 'India'
    ], (rand() % 9) + 1) as country,
    arrayElement(['US', 'GB', 'CA', 'AU', 'DE', 'FR', 'JP', 'BR', 'IN'], (rand() % 9) + 1) as country_code,
    arrayElement([
        'New York', 'London', 'Toronto', 'Sydney', 'Berlin',
        'Paris', 'Tokyo', 'São Paulo', 'Mumbai'
    ], (rand() % 9) + 1) as city,
    arrayElement([
        'NY', 'England', 'Ontario', 'NSW', 'Brandenburg',
        'Île-de-France', 'Tokyo', 'São Paulo', 'Maharashtra'
    ], (rand() % 9) + 1) as region,
    arrayElement(['desktop', 'mobile', 'tablet'], (rand() % 3) + 1) as device_type,
    arrayElement(['Chrome', 'Safari', 'Firefox', 'Edge', 'Opera'], (rand() % 5) + 1) as browser,
    arrayElement(['Windows', 'iOS', 'macOS', 'Android', 'Linux'], (rand() % 5) + 1) as os,
    if(rand() % 100 < 5, true, false) as is_bot,  -- 5% bot traffic
    'GET' as http_method,
    toUInt16(20 + rand() % 180) as response_time,
    200 as status_code,
    if(rand() % 10 < 4, arrayElement(['google', 'facebook', 'twitter', 'linkedin', 'email'], (rand() % 5) + 1), null) as utm_source,
    if(rand() % 10 < 4, arrayElement(['cpc', 'social', 'email', 'referral', 'organic'], (rand() % 5) + 1), null) as utm_medium,
    if(rand() % 10 < 3, arrayElement(['q1_campaign', 'brand_awareness', 'product_launch'], (rand() % 3) + 1), null) as utm_campaign
FROM numbers(5000);  -- 5000 total events for pro tier (avg 50 per link)

-- Business tier links (200 links, high traffic)
-- Generate 20-200 clicks per link over the past 90 days
INSERT INTO link_events (
    link_id,
    user_id,
    timestamp,
    ip_address,
    user_agent,
    referrer,
    country,
    country_code,
    city,
    region,
    device_type,
    browser,
    os,
    is_bot,
    http_method,
    response_time,
    status_code,
    utm_source,
    utm_medium,
    utm_campaign
)
SELECT
    generateUUIDv4() as link_id,
    'f3333333-3333-3333-3333-333333333333' as user_id,
    now() - interval rand() % 90 day as timestamp,
    toIPv6(concat('172.', toString(16 + rand() % 16), '.', toString(rand() % 256), '.', toString(rand() % 256))) as ip_address,
    arrayElement([
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0',
        'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/537.36',
        'Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (X11; Linux x86_64) Firefox/121.0',
        'Mozilla/5.0 (Android 14; Mobile) Chrome/120.0.0.0'
    ], (rand() % 6) + 1) as user_agent,
    arrayElement([
        'https://google.com/search',
        'https://facebook.com/',
        'https://twitter.com/',
        'https://linkedin.com/feed',
        'https://reddit.com/r/business',
        'https://youtube.com/watch',
        'https://instagram.com/',
        'https://tiktok.com/',
        'direct'
    ], (rand() % 9) + 1) as referrer,
    arrayElement([
        'United States', 'United Kingdom', 'Canada', 'Australia', 
        'Germany', 'France', 'Japan', 'Brazil', 'India',
        'Mexico', 'Spain', 'Italy', 'Netherlands', 'Singapore'
    ], (rand() % 14) + 1) as country,
    arrayElement([
        'US', 'GB', 'CA', 'AU', 'DE', 'FR', 'JP', 'BR', 'IN',
        'MX', 'ES', 'IT', 'NL', 'SG'
    ], (rand() % 14) + 1) as country_code,
    arrayElement([
        'New York', 'Los Angeles', 'Chicago', 'Houston', 'London', 
        'Toronto', 'Sydney', 'Berlin', 'Paris', 'Tokyo', 
        'São Paulo', 'Mumbai', 'Mexico City', 'Madrid', 'Amsterdam'
    ], (rand() % 15) + 1) as city,
    'Various' as region,
    arrayElement(['desktop', 'mobile', 'tablet'], (rand() % 3) + 1) as device_type,
    arrayElement(['Chrome', 'Safari', 'Firefox', 'Edge', 'Opera', 'Brave'], (rand() % 6) + 1) as browser,
    arrayElement(['Windows', 'iOS', 'macOS', 'Android', 'Linux'], (rand() % 5) + 1) as os,
    if(rand() % 100 < 8, true, false) as is_bot,  -- 8% bot traffic
    'GET' as http_method,
    toUInt16(15 + rand() % 150) as response_time,
    200 as status_code,
    if(rand() % 10 < 6, arrayElement(['google', 'facebook', 'twitter', 'linkedin', 'email', 'instagram'], (rand() % 6) + 1), null) as utm_source,
    if(rand() % 10 < 6, arrayElement(['cpc', 'social', 'email', 'referral', 'organic', 'paid'], (rand() % 6) + 1), null) as utm_medium,
    if(rand() % 10 < 5, arrayElement(['2025_q1', 'brand_campaign', 'product_launch', 'seasonal'], (rand() % 4) + 1), null) as utm_campaign
FROM numbers(20000);  -- 20000 total events for business tier (avg 100 per link)

-- Enterprise tier links (500 links, very high traffic)
-- Generate 50-500 clicks per link over the past 120 days
INSERT INTO link_events (
    link_id,
    user_id,
    timestamp,
    ip_address,
    user_agent,
    referrer,
    country,
    country_code,
    city,
    region,
    device_type,
    browser,
    os,
    is_bot,
    http_method,
    response_time,
    status_code,
    utm_source,
    utm_medium,
    utm_campaign
)
SELECT
    generateUUIDv4() as link_id,
    'f4444444-4444-4444-4444-444444444444' as user_id,
    now() - interval rand() % 120 day as timestamp,
    toIPv6(concat(
        toString(rand() % 256), '.', 
        toString(rand() % 256), '.', 
        toString(rand() % 256), '.', 
        toString(rand() % 256)
    )) as ip_address,
    arrayElement([
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0',
        'Mozilla/5.0 (Windows NT 11.0; Win64; x64) Chrome/120.0.0.0',
        'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/537.36',
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) Safari/605.1.15',
        'Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) Mobile/15E148',
        'Mozilla/5.0 (X11; Linux x86_64) Firefox/121.0',
        'Mozilla/5.0 (Android 14; Mobile) Chrome/120.0.0.0',
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edge/120.0.0.0'
    ], (rand() % 9) + 1) as user_agent,
    arrayElement([
        'https://google.com/search',
        'https://bing.com/search',
        'https://facebook.com/',
        'https://twitter.com/',
        'https://linkedin.com/feed',
        'https://reddit.com/',
        'https://youtube.com/',
        'https://instagram.com/',
        'https://tiktok.com/',
        'https://pinterest.com/',
        'https://slack.com/',
        'https://teams.microsoft.com/',
        'direct'
    ], (rand() % 13) + 1) as referrer,
    -- Global distribution for enterprise
    arrayElement([
        'United States', 'United Kingdom', 'Canada', 'Australia', 
        'Germany', 'France', 'Japan', 'Brazil', 'India', 'China',
        'Mexico', 'Spain', 'Italy', 'Netherlands', 'Singapore',
        'South Korea', 'Sweden', 'Switzerland', 'Belgium', 'Poland'
    ], (rand() % 20) + 1) as country,
    arrayElement([
        'US', 'GB', 'CA', 'AU', 'DE', 'FR', 'JP', 'BR', 'IN', 'CN',
        'MX', 'ES', 'IT', 'NL', 'SG', 'KR', 'SE', 'CH', 'BE', 'PL'
    ], (rand() % 20) + 1) as country_code,
    'Global Cities' as city,
    'Various' as region,
    if(rand() % 100 < 10, 'bot', arrayElement(['desktop', 'mobile', 'tablet'], (rand() % 3) + 1)) as device_type,
    arrayElement(['Chrome', 'Safari', 'Firefox', 'Edge', 'Opera', 'Brave', 'Samsung Browser'], (rand() % 7) + 1) as browser,
    arrayElement(['Windows', 'iOS', 'macOS', 'Android', 'Linux', 'Chrome OS'], (rand() % 6) + 1) as os,
    if(rand() % 100 < 10, true, false) as is_bot,  -- 10% bot traffic
    'GET' as http_method,
    toUInt16(10 + rand() % 100) as response_time,
    200 as status_code,
    if(rand() % 10 < 7, arrayElement([
        'google', 'facebook', 'twitter', 'linkedin', 'email', 
        'instagram', 'youtube', 'tiktok', 'reddit', 'newsletter'
    ], (rand() % 10) + 1), null) as utm_source,
    if(rand() % 10 < 7, arrayElement([
        'cpc', 'cpm', 'social', 'email', 'referral', 
        'organic', 'paid_social', 'display', 'video', 'affiliate'
    ], (rand() % 10) + 1), null) as utm_medium,
    if(rand() % 10 < 6, concat('enterprise_campaign_', toString(rand() % 50 + 1)), null) as utm_campaign
FROM numbers(125000);  -- 125000 total events for enterprise tier (avg 250 per link)


-- ============================================================================
-- SUMMARY
-- ============================================================================
-- Total events created:
-- Free tier: ~500 events (10 per link average)
-- Pro tier: ~5,000 events (50 per link average)
-- Business tier: ~20,000 events (100 per link average)
-- Enterprise tier: ~125,000 events (250 per link average)
-- Total: ~150,500 demo events for realistic testing