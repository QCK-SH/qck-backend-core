# QCK Backend API Documentation

## Base URL
- Development: `http://localhost:8080/api/v1`
- Production: `https://api.qck.sh/v1`

## Authentication Endpoints

### User Registration

**Endpoint:** `POST /auth/register`

Creates a new user account with email and password authentication.

#### Request

```bash
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "SecureP@ssw0rd123!",
    "password_confirmation": "SecureP@ssw0rd123!",
    "full_name": "John Doe",
    "company_name": "Acme Corp",
    "accept_terms": true
  }'
```

#### Request Body

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `email` | string | Yes | User's email address (max 320 chars) |
| `password` | string | Yes | Password (min 8 chars, must include uppercase, lowercase, number, special char) |
| `password_confirmation` | string | Yes | Must match password field |
| `full_name` | string | Yes | User's full name (1-255 chars) |
| `company_name` | string | No | User's company name (max 255 chars, optional) |
| `accept_terms` | boolean | Yes | Must be `true` to accept terms |

#### Password Requirements
- Minimum 8 characters
- At least one uppercase letter (A-Z)
- At least one lowercase letter (a-z)
- At least one number (0-9)
- At least one special character (!@#$%^&* etc.)

#### Success Response (201 Created)

```json
{
  "success": true,
  "data": {
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com",
    "full_name": "John Doe",
    "company_name": "Acme Corp",
    "email_verification_required": true,
    "verification_sent": true,
    "message": "Registration successful! Please check your email for a 6-digit verification code."
  },
  "message": "User registered successfully"
}
```

#### Error Responses

##### 400 Bad Request - Validation Error
```json
{
  "success": false,
  "data": null,
  "message": "password: Password must be at least 8 characters with uppercase, lowercase, number and special character"
}
```

##### 400 Bad Request - Password Mismatch
```json
{
  "success": false,
  "data": null,
  "message": "Passwords do not match"
}
```

##### 400 Bad Request - Terms Not Accepted
```json
{
  "success": false,
  "data": null,
  "message": "You must accept the terms and conditions"
}
```

##### 409 Conflict - Email Already Exists
```json
{
  "success": false,
  "data": null,
  "message": "An account with this email address already exists"
}
```

##### 429 Too Many Requests - Rate Limit Exceeded
```json
{
  "success": false,
  "data": null,
  "message": "Too many registration attempts. Please try again in 60 seconds"
}
```

#### Rate Limiting
- **Limit:** 5 requests per minute per IP address
- **Window:** 60 seconds sliding window
- **Reset:** Automatically after 60 seconds

#### Email Verification
- New accounts are created with `email_verified: false`
- Users will receive a verification email (when email service is implemented)
- Verification is required before full account access

#### Notes
- Emails are stored in lowercase for consistency
- Email comparison is case-insensitive (user@example.com = USER@EXAMPLE.COM)
- Subscription tier is set to "pending" until user completes email verification and selects a plan
- Passwords are hashed using Argon2id algorithm (OWASP recommended)
- Email verification with 6-digit code will be implemented in DEV-103

---

## Testing the Registration Endpoint

### Using curl

```bash
# Successful registration
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "TestP@ssw0rd123!",
    "password_confirmation": "TestP@ssw0rd123!",
    "full_name": "Test User",
    "company_name": null,
    "accept_terms": true
  }'

# Test weak password
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test2@example.com",
    "password": "weak",
    "password_confirmation": "weak",
    "accept_terms": true
  }'

# Test password mismatch
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test3@example.com",
    "password": "TestP@ssw0rd123!",
    "password_confirmation": "DifferentP@ssw0rd456!",
    "accept_terms": true
  }'
```

### Using HTTPie

```bash
# Successful registration
http POST localhost:8080/v1/auth/register \
  email=test@example.com \
  password=TestP@ssw0rd123! \
  password_confirmation=TestP@ssw0rd123! \
  accept_terms=true
```

### Using JavaScript/Fetch

```javascript
const response = await fetch('http://localhost:8080/v1/auth/register', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    email: 'test@example.com',
    password: 'TestP@ssw0rd123!',
    password_confirmation: 'TestP@ssw0rd123!',
    full_name: 'Test User',
    company_name: 'Test Company', // optional, can be null
    accept_terms: true
  })
});

const data = await response.json();
console.log(data);
```

---

## OpenAPI Specification

Full OpenAPI 3.0 specification available at: `/docs/openapi/auth-register.yaml`

To view in Swagger UI, you can use:
- [Swagger Editor](https://editor.swagger.io/) - Paste the YAML content
- [Redoc](https://redocly.github.io/redoc/) - For better documentation rendering

---

## Integration Tests

Run the registration tests:

```bash
# Run all registration tests
cargo test --test registration_test

# Run with output
cargo test --test registration_test -- --nocapture
```

Test coverage includes:
- ✅ Successful registration
- ✅ Duplicate email detection
- ✅ Weak password validation
- ✅ Password mismatch
- ✅ Invalid email format
- ✅ Terms acceptance requirement
- ✅ Rate limiting (5 req/min)
- ✅ Case-insensitive email checking
