# JWT HS256 (HMAC) Setup Guide for Production

## Overview
The QCK Backend uses HS256 (HMAC with SHA-256) for JWT token signing as specified in Linear DEV-113. This guide explains how to generate and configure HMAC secrets for production deployment.

## Why HS256 over ES256?
While DEV-92 originally specified ES256, DEV-113 updated the requirement to HS256 for the following reasons:
- **Simpler**: Symmetric keys instead of asymmetric key pairs
- **Faster**: HMAC operations are more efficient than ECDSA
- **Easier key management**: Single secret vs public/private key pairs
- **Perfect for monolithic backends**: Same service issues and validates tokens

## Requirements
- OpenSSL or similar tool (for secret generation)
- HS256 algorithm (HMAC SHA-256)
- Separate secrets for access and refresh tokens (required for security)
- Minimum 256-bit (32 bytes) secret length

## 🔑 Generating HMAC Secrets for Production

### Option 1: Using OpenSSL (Recommended)
```bash
# Generate 32-byte (256-bit) secret for access tokens
openssl rand -hex 32

# Generate different 32-byte secret for refresh tokens
openssl rand -hex 32
```

### Option 2: Using /dev/urandom
```bash
# Generate base64-encoded secrets
head -c 32 /dev/urandom | base64
```

### Option 3: Using Node.js
```javascript
// Generate cryptographically secure random secrets
const crypto = require('crypto');
console.log('Access Secret:', crypto.randomBytes(32).toString('hex'));
console.log('Refresh Secret:', crypto.randomBytes(32).toString('hex'));
```

### Option 4: Using Python
```python
import secrets
print(f"Access Secret: {secrets.token_hex(32)}")
print(f"Refresh Secret: {secrets.token_hex(32)}")
```

## 🚀 Production Deployment

### Required Environment Variables
```bash
# JWT Configuration (HS256)
JWT_ACCESS_SECRET=<your-generated-access-secret>
JWT_REFRESH_SECRET=<your-generated-refresh-secret>
JWT_ACCESS_EXPIRY=3600        # 1 hour for production
JWT_REFRESH_EXPIRY=604800     # 7 days
JWT_KEY_VERSION=1              # For key rotation
JWT_AUDIENCE=qck.sh            # Your production domain
JWT_ISSUER=qck.sh              # Your production domain
```

### Docker Compose Configuration
```yaml
services:
  qck-api:
    environment:
      - JWT_ACCESS_SECRET=${JWT_ACCESS_SECRET}
      - JWT_REFRESH_SECRET=${JWT_REFRESH_SECRET}
      - JWT_ACCESS_EXPIRY=3600
      - JWT_REFRESH_EXPIRY=604800
      - JWT_AUDIENCE=qck.sh
      - JWT_ISSUER=qck.sh
```

### Kubernetes Secrets
```bash
# Create secrets
kubectl create secret generic jwt-secrets \
  --from-literal=JWT_ACCESS_SECRET=$(openssl rand -hex 32) \
  --from-literal=JWT_REFRESH_SECRET=$(openssl rand -hex 32)
```

## 🔐 Security Best Practices

### 1. Secret Storage
- **Never commit secrets to version control**
- Use environment variables or secret management services
- Rotate secrets regularly (implement key versioning)

### 2. Secret Management Services
- **AWS Secrets Manager**: Automatic rotation, encryption at rest
- **HashiCorp Vault**: Dynamic secrets, audit logging
- **Azure Key Vault**: Managed HSM support
- **Google Secret Manager**: Automatic replication, IAM integration

### 3. Secret Requirements
- Minimum 256 bits (32 bytes) of entropy
- Different secrets for access and refresh tokens
- Environment-specific secrets (dev/staging/prod)
- Regular rotation schedule (e.g., quarterly)

### 4. Token Expiry Settings
- **Development**: 86400 seconds (1 day) for convenience
- **Production**: 3600 seconds (1 hour) for security
- **Refresh tokens**: 604800 seconds (7 days) in all environments

## 📋 Security Checklist

- [ ] Generated cryptographically secure secrets (32+ bytes)
- [ ] Using different secrets for access and refresh tokens
- [ ] Secrets stored securely (not in code or configs)
- [ ] Environment-specific secrets for dev/staging/prod
- [ ] Proper token expiry times configured
- [ ] Key rotation plan in place
- [ ] Audit logging enabled for token operations
- [ ] Token blacklisting implemented (Redis)

## 🚨 Common Issues

### "Invalid JWT secret" Error
- Ensure secrets are properly base64 or hex encoded
- Check environment variable is being loaded correctly
- Verify secret meets minimum length requirement (32 bytes)

### "Token signature verification failed"
- Confirm using correct secret for token type (access vs refresh)
- Check JWT_KEY_VERSION matches between signing and verification
- Ensure secrets haven't been accidentally truncated

### Performance Considerations
- HS256 is significantly faster than ES256
- HMAC operations scale better under high load
- Symmetric keys reduce memory footprint

## 📝 Testing Your Configuration

### Development Testing
```bash
# Set test secrets
export JWT_ACCESS_SECRET="dev-access-secret-change-in-production-hs256"
export JWT_REFRESH_SECRET="dev-refresh-secret-change-in-production-hs256"
export JWT_ACCESS_EXPIRY=86400
export JWT_AUDIENCE=dev.qck.sh
export JWT_ISSUER=dev.qck.sh

# Run tests
cargo test jwt
```

### Production Validation
```bash
# Verify environment variables are set
env | grep JWT_

# Test token generation and validation
curl -X POST https://api.qck.sh/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com","password":"test"}'
```

## 🔄 Key Rotation Strategy

1. Generate new secrets
2. Update JWT_KEY_VERSION environment variable
3. Deploy with both old and new secrets available
4. Gradually phase out old tokens
5. Remove old secrets after grace period

## 📚 Additional Resources

- [JWT Best Practices](https://tools.ietf.org/html/rfc8725)
- [OWASP JWT Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/JSON_Web_Token_for_Java_Cheat_Sheet.html)
- [HMAC Security Considerations](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.198-1.pdf)