# Demo Users Documentation

This document provides information about the demo user accounts created for development and testing purposes.

## Overview

Four demo user accounts are automatically created when running the database migrations. These accounts represent different subscription tiers and can be used for testing various features and limitations of the QCK platform.

## Demo User Accounts

All demo users share the same password: **Demo123!**

### 1. Free Tier User
- **Email**: `demo.free@qck.sh`
- **Password**: `Demo123!`
- **Subscription Tier**: Free
- **Full Name**: Demo Free User
- **Company**: Startup Inc
- **Features**: Basic URL shortening with link limits

```json
  {
   "email": "demo.free@qck.sh",
   "password": "Demo123!",
    "remember_me": true
  }

```
### 2. Pro Tier User
- **Email**: `demo.pro@qck.sh`
- **Password**: `Demo123!`
- **Subscription Tier**: Pro
- **Full Name**: Demo Pro User
- **Company**: Growth Corp
- **Features**: Enhanced features with higher link limits

### 3. Business Tier User
- **Email**: `demo.business@qck.sh`
- **Password**: `Demo123!`
- **Subscription Tier**: Business
- **Full Name**: Demo Business User
- **Company**: Scale LLC
- **Features**: Business-level features and analytics
```json
  {
   "email": "demo.business@qck.sh",
   "password": "Demo123!",
    "remember_me": true
  }

```
### 4. Enterprise Tier User
- **Email**: `demo.enterprise@qck.sh`
- **Password**: `Demo123!`
- **Subscription Tier**: Enterprise
- **Full Name**: Demo Enterprise User
- **Company**: MegaCorp Global
- **Features**: Unlimited links and premium enterprise features

## Sample Data

Each demo user account comes pre-populated with sample links:

- **Free**: 5 sample links
- **Pro**: 10 sample links  
- **Business**: 15 sample links
- **Enterprise**: 20 sample links

All sample links point to example.com domains with realistic click counts and creation dates spread over the past few weeks.

## Database Implementation

The demo users are created through Diesel migrations:
- Migration file: `migrations/diesel/2025-09-01-074056_seed_demo_users/`
- All users use the same Argon2id password hash for the password "Demo123!"
- Hash: `$argon2id$v=19$m=19456,t=2,p=1$KPAtRDVwr4dE+YODBkz9tQ$m/fW7O4oLWAXedan3NWL2G7J0v8ofqwEsicX+YA9wV8`

## Usage

These demo accounts can be used for:
- Testing subscription tier limitations
- Verifying bulk creation features (especially enterprise unlimited links)
- UI/UX testing across different user types
- API endpoint testing with different permission levels
- Analytics and reporting feature testing

## Environment

These demo users are created in:
- Development environment (docker-compose.dev.yml)
- Staging environment
- **NOT in production** (migration includes safety checks)

## Security Notes

- All demo users share the same password for simplicity
- Demo users are identified by the `demo.%@qck.sh` email pattern
- The migration includes cleanup of existing demo users before creation
- Demo users should not be used in production environments

## Testing Enterprise Features

The enterprise tier demo user (`demo.enterprise@qck.sh`) is particularly useful for testing:
- Unlimited link creation
- Bulk link creation endpoints (up to 100 URLs at once)
- Enterprise-level analytics
- Advanced user management features

## Cleanup

To remove demo users and their data, run the down migration:

```bash
diesel migration revert
```

This will execute the down migration that removes all demo users and their associated links.
