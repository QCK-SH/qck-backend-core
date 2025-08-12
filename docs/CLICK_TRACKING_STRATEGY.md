# Click Tracking Strategy

## Overview
QCK uses a dual-database approach for click tracking:
- **PostgreSQL**: Stores denormalized click counts for fast dashboard access
- **ClickHouse**: Stores detailed analytics for every single HTTP request

## Click Count Rules

### What Counts as a Click (Increments PostgreSQL click_count)
- ✅ **GET requests** - Real user visits
- ❌ **HEAD requests** - Bot/crawler checks (stored in ClickHouse but doesn't increment counter)
- ❌ **OPTIONS requests** - CORS preflight checks
- ❌ **Other methods** - Not counted as clicks

### PostgreSQL (links table)
```sql
-- click_count field is incremented ONLY for GET requests
UPDATE links 
SET click_count = click_count + 1 
WHERE short_code = ? 
  AND [request_method = 'GET'];  -- Pseudo-code
```

### ClickHouse (link_events table)
```sql
-- ALL requests are stored for analytics
INSERT INTO link_events (
    link_id, 
    http_method,  -- 'GET', 'HEAD', 'OPTIONS', etc.
    ...
) VALUES (...);
```

## Why This Approach?

1. **Performance**: Dashboard can show click counts without querying ClickHouse
2. **Analytics**: ClickHouse has complete request history for deep analysis
3. **Bot Detection**: Can identify bot traffic (HEAD requests) vs real users (GET requests)
4. **Accuracy**: Click count reflects real visits, not bot checks

## Implementation in Rust Backend

```rust
// Pseudo-code for redirect handler
async fn handle_redirect(method: HttpMethod, short_code: String) {
    // Store EVERY request in ClickHouse
    clickhouse.insert_event(link_id, method, ...);
    
    // Only increment PostgreSQL counter for GET requests
    if method == HttpMethod::GET {
        postgres.increment_click_count(link_id);
    }
    
    // Return redirect response
    redirect_to(target_url);
}
```

## Querying Click Data

### Get simple click count (fast)
```sql
-- From PostgreSQL
SELECT click_count FROM links WHERE id = ?;
```

### Get detailed analytics (comprehensive)
```sql
-- From ClickHouse
SELECT 
    countIf(http_method = 'GET') as real_clicks,
    countIf(http_method = 'HEAD') as bot_checks,
    uniqExact(ip_address) as unique_visitors
FROM link_events 
WHERE link_id = ?;
```

## Bot vs Human Traffic

The system can distinguish between:
- **Human traffic**: GET requests from non-bot user agents
- **Bot checks**: HEAD requests (checking if link exists)
- **Bot visits**: GET requests with bot user agents
- **Preflight**: OPTIONS requests for CORS

This allows for accurate analytics and bot detection.