# Refresh Token Rotation API Documentation

## Overview

This document describes the implementation of refresh token rotation for the QCK Backend API, as specified in Linear task DEV-107. The system implements automatic token rotation with comprehensive security features including token reuse detection, device fingerprinting, and family-based invalidation.

## Key Features

### 1. Automatic Token Rotation
- New token pair generated on each refresh
- Old refresh token immediately revoked
- Seamless user experience with no interruption

### 2. Token Reuse Detection
- Detects when a revoked token is reused (potential theft)
- Invalidates entire token family on reuse detection
- Forces user to re-authenticate for security

### 3. Device Fingerprinting
- Tracks device information (IP, user agent)
- Monitors for suspicious activity patterns
- Enables per-device token management

### 4. Rate Limiting
- Refresh endpoint: 10 requests per minute per IP
- 5-minute block on rate limit violation
- Prevents brute force attacks

## API Endpoints

### POST /v1/auth/refresh

Refreshes an access token using a valid refresh token.

#### Request

```json
{
  "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

#### Headers
- `User-Agent`: Used for device fingerprinting
- `X-Real-IP` or `X-Forwarded-For`: Used for IP tracking

#### Response

##### Success (200 OK)
```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "expires_in": 3600,
    "token_type": "Bearer"
  },
  "message": "Token refreshed successfully"
}
```

##### Error Responses

###### Token Expired (401 Unauthorized)
```json
{
  "success": false,
  "data": null,
  "message": "Refresh token expired"
}
```

###### Token Revoked (401 Unauthorized)
```json
{
  "success": false,
  "data": null,
  "message": "Refresh token revoked"
}
```

###### Token Reuse Detected (403 Forbidden)
```json
{
  "success": false,
  "data": null,
  "message": "Security breach detected - all tokens revoked"
}
```

###### Suspicious Activity (403 Forbidden)
```json
{
  "success": false,
  "data": null,
  "message": "Suspicious activity detected - please login again"
}
```

###### Rate Limited (429 Too Many Requests)
```json
{
  "success": false,
  "data": null,
  "message": "Rate limit exceeded. Try again in 300 seconds"
}
```

## Database Schema

### refresh_tokens Table

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | Primary key |
| user_id | UUID | Foreign key to users table |
| jti_hash | VARCHAR(64) | SHA-256 hash of JWT ID |
| token_family | VARCHAR(64) | Family identifier for rotation tracking |
| created_at | TIMESTAMP | Token creation time |
| expires_at | TIMESTAMP | Token expiration time |
| issued_at | TIMESTAMP | Token issue time |
| last_used_at | TIMESTAMP | Last usage timestamp |
| revoked_at | TIMESTAMP | Revocation timestamp (null if active) |
| revoked_reason | VARCHAR(255) | Reason for revocation |
| device_fingerprint | VARCHAR(255) | Device identification hash |
| ip_address | INET | IP address of token usage |
| user_agent | TEXT | Browser/client user agent |
| updated_at | TIMESTAMP | Last update timestamp |

### Indexes
- `idx_refresh_tokens_jti_hash` - Fast lookup by JWT ID
- `idx_refresh_tokens_user_id` - User token queries
- `idx_refresh_tokens_token_family` - Family invalidation
- `idx_refresh_tokens_user_active_v2` - Active token queries

## Security Mechanisms

### 1. Token Family Tracking
```
Initial Login:
  Token A (family: F1) → Active

First Refresh:
  Token A → Revoked
  Token B (family: F1) → Active

Token Reuse (Attack):
  Token A used again → Detected
  All tokens in family F1 → Revoked
  User must re-authenticate
```

### 2. Device Fingerprinting Algorithm
```
fingerprint = SHA256(user_agent + ip_address + timezone + screen_resolution + language + encoding)
```

### 3. Suspicious Activity Detection
- Multiple tokens from different IPs in short time
- Rapid device changes
- Geographic impossibilities

### 4. Rate Limiting Configuration
```rust
RateLimitConfig {
    max_requests: 10,
    window_seconds: 60,
    burst_limit: Some(3),
    block_duration: 300,
    distributed: true,
}
```

## Implementation Details

### Token Rotation Flow

1. **Receive refresh request**
   - Extract device information
   - Generate fingerprint
   - Check rate limits

2. **Validate old token**
   - Check expiration
   - Check revocation status
   - Detect reuse attempts

3. **Database transaction**
   - Mark old token as used
   - Check for token reuse
   - Create new token pair
   - Revoke old token

4. **Return new tokens**
   - New access token (1 hour)
   - New refresh token (7 days)

### Error Handling

All rotation operations are atomic using database transactions. If any step fails:
- Transaction rolls back
- No partial state changes
- Clear error returned to client

### Performance Considerations

- Token validation: < 5ms target
- Database queries use indexes
- Redis caching for blacklisted tokens
- Connection pooling for scalability

## Usage Examples

### JavaScript/TypeScript
```typescript
async function refreshToken(refreshToken: string): Promise<TokenPair> {
  const response = await fetch('/v1/auth/refresh', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ refresh_token: refreshToken }),
  });

  if (!response.ok) {
    if (response.status === 403) {
      // Security breach or suspicious activity
      // Force user to login again
      window.location.href = '/login';
    }
    throw new Error('Token refresh failed');
  }

  const data = await response.json();
  return data.data;
}
```

### cURL
```bash
curl -X POST https://api.qck.sh/v1/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."}'
```

## Testing

### Unit Tests
Located in `/tests/refresh_token_rotation_test.rs`:
- Token storage with device info
- Family tracking and invalidation
- Reuse detection
- Suspicious activity detection
- Expiration handling
- Active token counting

### Integration Tests
Located in `/tests/refresh_token_integration_test.rs`:
- Complete rotation flow
- Concurrent refresh attempts
- Device fingerprint tracking
- Logout invalidation
- Rate limiting

## Migration Guide

### From Non-Rotating Tokens

1. Deploy new code with rotation support
2. Existing tokens continue to work
3. On next refresh, tokens enter rotation system
4. Old tokens automatically expire

### Database Migration
```sql
-- Run automatically via Diesel migrations
ALTER TABLE refresh_tokens 
    ADD COLUMN token_family VARCHAR(64) NOT NULL DEFAULT gen_random_uuid()::text,
    ADD COLUMN device_fingerprint VARCHAR(255),
    ADD COLUMN ip_address INET,
    -- ... other columns
```

## Monitoring

### Key Metrics
- Token rotation success rate
- Reuse detection events
- Average rotation latency
- Rate limit violations

### Alerts
- High reuse detection rate (potential attack)
- Rotation failures
- Performance degradation

## Compliance

- **GDPR**: IP addresses and user agents are stored with user consent
- **Security**: Tokens are hashed before storage
- **Privacy**: Device fingerprints are one-way hashes

## Related Documentation

- [JWT Token Validation (DEV-113)](./JWT_SETUP.md)
- [Rate Limiting Middleware (DEV-115)](./RATE_LIMITING.md)
- [Database Migrations](./MIGRATION_SETUP.md)

---

*Last Updated: August 2025*
*Linear Task: DEV-107*
*Implementation Status: Complete*
