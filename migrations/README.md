# QCK Core Migrations

## Migration Philosophy

The `qck-core` backend is the **open-source foundation** of the QCK platform. It should be fully functional for self-hosted deployments without any cloud-specific features.

### Core Principle

> **OSS-First Design**: The core migrations should create a complete, working URL shortener that requires NO cloud infrastructure, payment processing, or commercial features.

---

## Migration Numbering Convention

### Core Migrations: `0` - `899999`

Migrations in this range belong to `qck-core` (OSS) and should include ONLY:

- Essential authentication (users, sessions, password resets)
- Core URL shortening (links, redirects, basic analytics)
- Self-hostable features (no Stripe, no team management, no SaaS features)

**Format**: `YYYY-MM-DD-NNNNNN_description`

Examples:
- `2025-01-08-000001_initial_schema`
- `2025-08-22-133222_create_refresh_tokens`
- `2025-08-24-043551_add_user_profile_fields`

### Cloud Migrations: `900000` - `999999`

Migrations in this range belong to `qck-cloud` (commercial SaaS) and include:

- Payment processing (Stripe integration)
- Advanced onboarding flows (email verification, plan selection)
- Multi-tenancy features (teams, workspaces)
- Commercial analytics and reporting
- Enterprise features

**Format**: `YYYY-MM-DD-9NNNNN_description_cloud`

Examples:
- `2025-09-01-900001_seed_demo_users`
- `2025-10-06-900003_add_payments_table_cloud`
- `2025-10-10-900004_extend_onboarding_statuses_cloud`

---

## Current Core Migrations

### 1. Initial Schema (`2025-01-08-000001`)
**Purpose**: Foundation for users and links

**Creates**:
- `users` table with basic auth fields
  - `email`, `password_hash`, `is_active`
  - `email_verified` (boolean, default false)
  - `subscription_tier` (varchar, default 'free')
  - `created_at`, `updated_at`, `email_verified_at`
- `links` table (initial version, replaced in migration 6)
- `refresh_tokens` table for JWT refresh tokens
- Essential indexes for performance
- `update_updated_at_column()` trigger function

**Cloud-Specific Fields** (inherited, acceptable):
- `email_verified`: Used for OSS email verification
- `subscription_tier`: Defaults to 'free', ignored in OSS

### 2. Create Refresh Tokens (`2025-08-22-133222`)
**Purpose**: JWT refresh token management

**Creates**:
- Refresh token tracking table
- Indexes for token lookup and expiration

### 3. Add User Profile Fields (`2025-08-24-043551`)
**Purpose**: Basic user profile information

**Adds to `users`**:
- `full_name` VARCHAR(255) NOT NULL
- `company_name` VARCHAR(255) NULL

**OSS Justification**: Reasonable for self-hosted deployments to track user names

### 4. Add Onboarding Status (`2025-08-24-060414`)
**Purpose**: Simple 2-state onboarding flow

**Adds to `users`**:
- `onboarding_status` VARCHAR(50) DEFAULT 'registered'
- Check constraint: `IN ('registered', 'completed')`

**OSS Use Case**: 
- `registered`: Just signed up
- `completed`: Profile completed (or auto-completed for OSS)

**Note**: Cloud migration 900004 extends this to 5 states for commercial flow

### 5. Change Refresh Token IP to TEXT (`2025-08-25-122440`)
**Purpose**: Store IPv6 addresses properly

**Changes**:
- `ip_address` column type from VARCHAR to TEXT

### 6. Create Password Reset Tokens (`2025-08-25-161133`)
**Purpose**: Password recovery mechanism

**Creates**:
- `password_reset_tokens` table
- Token validation and expiration logic

### 7. Change IP Address to TEXT (`2025-08-25-162000`)
**Purpose**: Consistency for IPv6 support across tables

### 8. Create Links Table (Consolidated) (`2025-08-29-072311`)
**Purpose**: Complete link management schema

**Creates**:
- Full-featured `links` table with:
  - Basic fields: `short_code`, `original_url`, `user_id`
  - Metadata: `title`, `description`, `tags[]`
  - Rich content: `og_image`, `favicon_url`
  - UTM tracking parameters
  - Soft delete support (`deleted_at`)
  - Processing status for async metadata extraction
  - Password-protected links
  - Custom aliases
  - Expiration support

**Consolidates**: Multiple incremental migrations into single schema

---

## What Belongs in Core vs Cloud?

### Core (qck-core) ✓

**Authentication**:
- User registration and login
- Password resets
- Session management
- Basic email verification (boolean flag)

**URL Shortening**:
- Short link creation
- Custom aliases
- Link metadata (title, description, OG tags)
- Basic click tracking
- UTM parameters
- Soft deletes

**Self-Hosted Features**:
- User profiles (name, company)
- Single-user or small team deployment
- No external payment dependencies

### Cloud (qck-cloud) ✗

**Payment Processing**:
- Stripe integration
- Subscription management
- Payment history
- Billing cycles

**Advanced Onboarding**:
- Multi-step verification flows
- Plan selection UI
- Payment-gated features

**Multi-Tenancy**:
- Teams and workspaces
- Role-based access control (beyond basic admin)
- Invitation systems

**Commercial Analytics**:
- Advanced dashboards
- Export features
- White-label options

---

## Problem: Cloud-Specific Fields in Core

### Issue

The initial schema (`2025-01-08-000001`) includes fields that suggest cloud features:

1. **`email_verified` (boolean)**: Acceptable for OSS
   - Simple flag for email confirmation
   - Can be auto-verified on registration for self-hosted

2. **`subscription_tier` (varchar)**: Cloud-leaning but acceptable
   - Defaults to 'free' (works for OSS)
   - OSS can ignore this field or use it for internal tier management
   - Not a blocker for self-hosted deployments

3. **`onboarding_status` (varchar)**: Simplified in core
   - Core: 2 states (`registered`, `completed`)
   - Cloud: 5 states (extended in migration 900004)

### Solution Strategy

**Do NOT remove these fields** because:
1. They're already in production databases
2. Removing them requires complex migration surgery
3. They don't prevent OSS functionality
4. Cloud builds on top of core schema

**Instead**:
- Document their purpose clearly (this README)
- Set OSS-friendly defaults
- Cloud extends constraints as needed

---

## Migration Workflow

### For Core Features

1. Create migration with number < 900000
2. Ensure no hard dependencies on cloud services
3. Test in standalone environment (no Stripe, no cloud APIs)
4. Document in this README

### For Cloud Features

1. Create migration with number >= 900000
2. Use clear `_cloud` suffix in name
3. Document dependency on core migrations
4. Place in `qck-cloud/qck-backend/migrations/`

---

## Future Migration Guidelines

### Adding to Core

**Ask yourself**:
- Does this work without internet connectivity?
- Does this require payment processing?
- Does this require cloud-only services (email delivery, etc)?
- Can a solo developer use this on their own server?

If all answers are YES (or work with simple configs), it belongs in core.

### Adding to Cloud

**Indicators**:
- Requires Stripe, SendGrid, or similar SaaS
- Multi-tenant features (teams, permissions beyond basic)
- Advanced analytics requiring ClickHouse Cloud
- Features behind paywalls

---

## Database Schema Philosophy

### Core Schema Goals

1. **Completeness**: Core should be a working product
2. **Simplicity**: Minimal dependencies
3. **Extensibility**: Cloud can build on top
4. **Backwards Compatibility**: Don't break existing installations

### Cloud Schema Goals

1. **Commercial Viability**: Support SaaS business model
2. **Scalability**: Multi-tenant architecture
3. **Feature Rich**: Advanced features for paying customers
4. **Integration Ready**: Connect to payment/email/analytics services

---

## Testing Migrations

### Core Testing

```bash
# Test core migrations in isolation
cd qck-core/qck-backend
docker-compose --env-file .env.dev -f docker-compose.dev.yml up -d
docker logs -f qck-backend-oss-dev
```

**Verify**:
- Migrations run successfully
- No cloud service dependencies
- All features work for single user

### Cloud Testing

```bash
# Test cloud migrations building on core
cd qck-cloud/qck-backend
docker-compose --env-file .env.dev -f docker-compose.dev.yml up -d
docker logs -f qck-api-dev
```

**Verify**:
- Core migrations run first
- Cloud migrations extend schema
- Payment features enabled
- Multi-tenant features work

---

## Migration Safety

### Never Do This

- ❌ Remove migrations from core after deployed
- ❌ Renumber existing migrations
- ❌ Add cloud dependencies to core migrations
- ❌ Break backwards compatibility without version bump

### Always Do This

- ✓ Test migrations on clean database
- ✓ Provide `down.sql` for rollback
- ✓ Document breaking changes
- ✓ Keep core self-contained

---

## Questions?

**Is this a core or cloud feature?**
- Core: Can it run on a Raspberry Pi without internet?
- Cloud: Does it need Stripe, teams, or cloud services?

**Should I modify an existing migration?**
- No. Create a new migration that alters the schema.
- Existing migrations are immutable once deployed.

**What if I need to add a cloud field to a core table?**
- Add it in a cloud migration (900000+ range)
- Make it nullable or provide a sensible default
- Document the extension clearly

---

## Revision History

- **2025-10-12**: Initial documentation (Phase 2 of OSS/Cloud separation)
- Migration strategy established
- Core/Cloud boundaries defined
