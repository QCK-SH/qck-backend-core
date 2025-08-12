# Viral Burst Scaling Strategy - 10M Clicks/Second

## The Reality
When a celebrity with 20M+ followers shares a link:
- **First 10 seconds**: 10M clicks/second (everyone clicking immediately)
- **Next 30 seconds**: 1M clicks/second (stragglers)  
- **Next 5 minutes**: 100k clicks/second (retweets)
- **After 10 minutes**: Back to normal (~1k/second)

**Total burst**: ~150M clicks in 5 minutes, not sustained traffic

## Current Bottlenecks & Solutions

### 1. PostgreSQL (CRITICAL BOTTLENECK ❌)

**Problem**: PostgreSQL will DIE at 10M updates/second
```sql
-- This will destroy PostgreSQL:
UPDATE links SET click_count = click_count + 1 WHERE id = ?  -- 10M/sec = DEAD
```

**SOLUTION: Remove PostgreSQL from redirect path entirely**

```rust
// BEFORE (will fail):
async fn handle_redirect(short_code: &str) {
    let link = postgres.get_link(short_code).await;  // Read OK
    postgres.increment_click_count(link.id).await;   // DEATH at scale
    clickhouse.insert_event(...).await;              // Async, OK
    return Redirect::to(link.target_url);
}

// AFTER (viral-ready):
async fn handle_redirect(short_code: &str) {
    // 1. Get from Redis cache first
    let link = redis.get_link(short_code).await
        .or_else(|| postgres.get_link(short_code).await);
    
    // 2. Fire and forget to ClickHouse (async)
    tokio::spawn(async move {
        clickhouse.insert_event(...).await;
    });
    
    // 3. Update Redis counter (10M ops/sec capable)
    redis.incr(format!("clicks:{}", link.id)).await;
    
    // 4. Return immediately
    return Redirect::to(link.target_url);
}
```

### 2. ClickHouse (SCALABLE ✅)

**Current Setup**: Buffer tables can handle 100k/sec
**Viral Setup**: Need distributed cluster for 10M/sec

```sql
-- Distributed table across multiple nodes
CREATE TABLE link_events_cluster ON CLUSTER qck_cluster AS link_events
ENGINE = Distributed(qck_cluster, qck_analytics, link_events_local, rand());

-- Each node has local buffer table
CREATE TABLE link_events_buffer_local AS link_events
ENGINE = Buffer(...
    100000,   -- min_rows (flush at 100k)
    1000000,  -- max_rows (force flush at 1M)
    1,        -- min_time (1 second)
    10        -- max_time (10 seconds max)
);
```

**Scaling Architecture**:
```
                    Load Balancer
                         |
        +----------------+----------------+
        |                |                |
    CH Node 1        CH Node 2        CH Node 3
    (3.3M/sec)       (3.3M/sec)       (3.3M/sec)
        |                |                |
        +----------------+----------------+
                         |
                 Distributed Table
                   (10M/sec total)
```

### 3. Redis (HERO OF THE STORY ✅)

Redis becomes the PRIMARY click counter during viral events:

```rust
// Redis cluster can handle 10M+ ops/sec
struct ViralClickHandler {
    redis_cluster: RedisCluster,
}

impl ViralClickHandler {
    // Real-time counter in Redis
    async fn increment_click(&self, link_id: Uuid) {
        // Redis INCR is atomic and fast
        self.redis_cluster.incr(format!("clicks:{}", link_id)).await;
        
        // Also track per-second rate for monitoring
        let second_key = format!("clicks:{}:{}", link_id, Utc::now().timestamp());
        self.redis_cluster.incr(second_key).expire(60).await;
    }
    
    // Batch sync to PostgreSQL (every 10 seconds)
    async fn sync_to_postgres(&self) {
        // Get all counters from Redis
        let counters = self.redis_cluster.get_pattern("clicks:*").await;
        
        // Batch update PostgreSQL
        for (link_id, count) in counters {
            // Single UPDATE instead of millions
            postgres.execute(
                "UPDATE links SET click_count = $1 WHERE id = $2",
                &[&count, &link_id]
            ).await;
        }
    }
}
```

## Complete Viral Architecture

```
   10M requests/sec
         |
    CloudFlare/CDN
         |
    Load Balancer
         |
   +-----+-----+
   |     |     |
App1   App2   App3  (Rust backends)
   |     |     |
   +-----+-----+
         |
    Redis Cluster ← Primary counter (10M ops/sec)
         |
         ├── Async batch to ClickHouse Cluster (analytics)
         |
         └── Sync every 10s to PostgreSQL (persistence)
```

## Implementation Checklist

### Phase 1: Preparation (Do Now)
- [ ] Implement Redis caching for all active links
- [ ] Move click counting to Redis INCR
- [ ] Create batch sync job (Redis → PostgreSQL)
- [ ] Set up ClickHouse buffer tables
- [ ] Implement async event insertion

### Phase 2: Viral Mode (Auto-triggers)
```rust
// Auto-detect viral traffic
if clicks_per_second > 1000 {
    enable_viral_mode(link_id).await;
}

async fn enable_viral_mode(link_id: Uuid) {
    // 1. Cache link in Redis indefinitely
    redis.cache_link_forever(link_id).await;
    
    // 2. Disable PostgreSQL updates
    postgres_updates_enabled = false;
    
    // 3. Switch to Redis-only counting
    use_redis_counter = true;
    
    // 4. Alert ops team
    alert_ops("Viral link detected", link_id).await;
}
```

### Phase 3: Scaling Infrastructure

**Redis Cluster** (for 10M ops/sec):
- 6 Redis nodes (3 masters, 3 slaves)
- Each handling ~1.6M ops/sec
- 256GB RAM total
- Redis Cluster mode enabled

**ClickHouse Cluster** (for 10M events/sec):
- 5 ClickHouse nodes
- Each handling 2M events/sec
- 128GB RAM per node
- NVMe SSDs for storage

**Application Servers**:
- 20 Rust backend instances
- Each handling 500k requests/sec
- Auto-scaling enabled
- Connection pooling optimized

## Burst-Optimized Architecture

### The "Lightning Rod" Pattern
Instead of scaling everything, we create a lightning rod to absorb the burst:

```
   Celebrity Tweet (10M/sec for 30 seconds)
                |
           CloudFlare 
                |
         [LIGHTNING ROD]
                |
    Redis Write-Behind Buffer
         (No DB writes)
                |
    ClickHouse Async Queue
         (Process later)
```

### Smart Burst Handling

```rust
// Detect and handle burst in real-time
struct BurstDetector {
    recent_clicks: Arc<RwLock<VecDeque<(Instant, u64)>>>,
    burst_mode: Arc<AtomicBool>,
}

impl BurstDetector {
    async fn handle_click(&self, link_id: Uuid) -> Result<()> {
        let clicks_per_second = self.calculate_rate().await;
        
        if clicks_per_second > 10_000 {
            // BURST MODE: Redis only, no database writes
            self.burst_mode.store(true, Ordering::Relaxed);
            redis.incr(format!("burst:{}", link_id)).await?;
            
            // Don't wait for anything else
            return Ok(());
        } else if clicks_per_second > 1_000 {
            // ELEVATED: Redis + async ClickHouse
            redis.incr(format!("clicks:{}", link_id)).await?;
            tokio::spawn(async move {
                clickhouse.buffer_insert(event).await;
            });
        } else {
            // NORMAL: Full processing
            redis.incr(format!("clicks:{}", link_id)).await?;
            postgres.increment_lazy(link_id).await?;
            clickhouse.insert(event).await?;
        }
        
        Ok(())
    }
}
```

### Cost for 30-Second Burst

**Actual costs for celebrity tweet burst:**
- CloudFlare: ~$50 (150M requests)
- Redis operations: ~$5 (150M INCR operations)
- ClickHouse ingestion: ~$20 (150M events processed async)
- Bandwidth: ~$100 (assuming 302 redirects)
- **Total: ~$175 per viral burst**

Much more reasonable than $21,000!

## Database Specific Limits

### PostgreSQL Limits:
- **Max UPDATE rate**: ~10,000/sec (single row)
- **Connection limit**: 5,000 connections
- **Solution**: Remove from hot path

### ClickHouse Limits:
- **Single node**: 1-2M events/sec
- **Cluster**: 50M+ events/sec possible
- **Solution**: Horizontal scaling

### Redis Limits:
- **Single node**: 1M ops/sec
- **Cluster**: 100M+ ops/sec possible
- **Solution**: Redis Cluster mode

## The Ultimate Solution: Edge Caching

### CloudFlare Workers for Viral Links
The BEST solution is to handle viral traffic at the edge:

```javascript
// CloudFlare Worker (runs at edge, 200+ locations worldwide)
addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request))
})

async function handleRequest(request) {
  const url = new URL(request.url)
  const shortCode = url.pathname.slice(1)
  
  // Check if this is a viral/popular link (cached for 1 hour)
  const cachedTarget = await VIRAL_LINKS.get(shortCode)
  
  if (cachedTarget) {
    // Handle redirect at edge - origin server never sees this request!
    
    // Fire and forget analytics (don't block redirect)
    event.waitUntil(
      fetch('https://analytics.qck.sh/event', {
        method: 'POST',
        body: JSON.stringify({
          link: shortCode,
          timestamp: Date.now(),
          country: request.cf.country,
          city: request.cf.city
        })
      })
    )
    
    // Immediate redirect (< 5ms latency)
    return Response.redirect(cachedTarget, 302)
  }
  
  // Not viral, pass to origin
  return fetch(request)
}
```

### Auto-Viral Detection
```rust
// Backend detects viral links and pushes to CDN
async fn detect_viral_link(&self, link_id: Uuid, rate: u64) {
    if rate > 1000 {  // 1000 clicks/second
        // Push to CloudFlare KV store
        cloudflare_api.put_kv(
            &link.short_code,
            &link.target_url,
            Duration::from_secs(3600)  // Cache for 1 hour
        ).await?;
        
        // Now CloudFlare handles everything!
    }
}
```

With edge caching:
- **Your servers see**: 0 requests (all handled at edge)
- **PostgreSQL load**: 0 updates
- **Redis load**: 0 operations  
- **ClickHouse**: Receives batched analytics later
- **Cost**: Just CloudFlare Workers ($0.50 per million requests)

## Emergency Procedures

### If viral traffic detected:
1. **Immediate**: Switch to Redis-only mode
2. **Within 1 min**: Scale application servers
3. **Within 5 min**: Enable ClickHouse cluster mode
4. **Within 10 min**: Add more Redis nodes if needed
5. **Monitor**: Watch error rates and latencies

### Graceful Degradation:
```rust
// If everything fails, just redirect
async fn emergency_redirect(short_code: &str) {
    // Get from CDN cache or hardcoded popular links
    match short_code {
        "viral1" => Redirect::to("https://target1.com"),
        "viral2" => Redirect::to("https://target2.com"),
        _ => {
            // Try Redis, if fails, return 503
            redis.get_link(short_code)
                .await
                .unwrap_or(Status::ServiceUnavailable)
        }
    }
}
```

## Testing Viral Scenarios

```bash
# Simulate 10M requests/second
artillery quick --count 10000 --num 1000 https://qck.sh/viral-link

# Monitor Redis
redis-cli --cluster call cluster info

# Monitor ClickHouse
clickhouse-client -q "SELECT count() FROM link_events WHERE timestamp > now() - 1"

# Watch PostgreSQL (should be quiet during viral)
psql -c "SELECT click_count FROM links WHERE id = 'viral-link-id'"
```

## Key Takeaways

1. **PostgreSQL cannot handle viral traffic directly** - Use Redis as buffer
2. **ClickHouse can scale horizontally** - Add nodes as needed
3. **Redis is the hero** - Can handle 100M+ ops/sec in cluster mode
4. **CDN is critical** - Cache redirects at edge
5. **Batch updates save the day** - Sync counters periodically, not per-click

## Recovery After Viral Event

```sql
-- Reconcile counts after viral event
UPDATE links 
SET click_count = (
    SELECT COUNT(*) 
    FROM clickhouse.link_events 
    WHERE link_id = links.id 
      AND http_method = 'GET'
)
WHERE id = 'viral-link-id';

-- Clean up Redis
DEL clicks:viral-link-id
```

---

**Remember**: It's better to lose accurate counts than to go down completely. Users care more about uptime than perfect analytics.