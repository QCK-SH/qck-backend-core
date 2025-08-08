# Architecture: Production Scaling for 1M Clicks/Day
Date: 2025-08-08 15:40
Linear Issue: N/A
Status: completed

## Context
- **Request**: Document production scaling requirements for 1M clicks/day target
- **Purpose**: Provide clear guidance for connection pool sizing and resource planning
- **Dependencies**: Redis and PostgreSQL connection pools implemented

## Implementation
### Load Analysis for 1M Clicks/Day
- **Average Load**: ~12 clicks/second (1M / 86400 seconds)
- **Peak Load**: ~100 clicks/second (8x average during peak hours)
- **Cache Strategy**: 95% hit ratio target to minimize database load
- **Concurrent Operations**: 50-200 simultaneous database operations

### Connection Pool Sizing
```bash
# PRODUCTION TARGET: 1M clicks/day
DATABASE_MAX_CONNECTIONS=300   # Handles 100 peak req/s
REDIS_POOL_SIZE=150           # 95% cache hit ratio target

# Scale-up roadmap:
# 2M clicks/day (200 peak): DB=500, Redis=250
# 5M clicks/day (500 peak): DB=800, Redis=400
# 10M+ clicks/day: DB=1000+, Redis=600+ + clustering
```

### Resource Requirements
- **API Instances**: 1GB RAM each, 1 CPU core
- **PostgreSQL**: 2GB RAM, 2 CPU cores, 400 max_connections
- **Redis**: 512MB RAM, 0.5 CPU core, 200 max_clients
- **Total System**: ~4GB RAM, 4 CPU cores minimum

## How It Works
1. **Request Flow**: User clicks → Redis cache lookup (95% hit) → DB query (5% miss)
2. **Connection Pooling**: Pre-warmed connections reduce latency
3. **Load Balancing**: 2 API instances for redundancy
4. **Monitoring**: Health checks track connection pool status

## Performance Targets
- **URL Redirect Latency**: < 50ms (95th percentile)
- **Redis Cache Hits**: < 1ms (95%+ hit rate)
- **Database Queries**: < 20ms (with indexing)
- **API Response Time**: < 100ms (URL shortening)
- **Throughput**: 100+ sustained, 200+ burst req/s
- **Availability**: 99.9% uptime

## Docker Configuration
```yaml
# Production docker-compose.yml template
services:
  qck-api:
    deploy:
      replicas: 2
      resources:
        limits:
          memory: 1GB
          cpus: '1.0'
    environment:
      - DATABASE_MAX_CONNECTIONS=300
      - REDIS_POOL_SIZE=150
```

## Files Updated
- `.env.example:8-33` - Added production scaling comments
- `docker-compose.yml:17-22` - Added production guidance comments

## Monitoring Required
- Connection pool utilization (should be <80%)
- Cache hit ratio (target 95%+)
- Response time percentiles
- Error rates and retry patterns

## Rollback
- Revert connection pool sizes to development values
- Remove production guidance from configuration files

## Notes
- Based on typical URL shortener traffic patterns
- Assumes 95% cache hit ratio for optimal performance
- Resource estimates include 25% safety margin
- Horizontal scaling recommended beyond 5M clicks/day