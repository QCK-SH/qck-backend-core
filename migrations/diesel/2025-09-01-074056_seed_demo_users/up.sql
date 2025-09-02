-- Demo Users Seed Data for Development/Staging
-- All users use the same password that test@example.com uses
-- PROTECTED: This migration is automatically skipped in production environment

-- Clean up existing demo users
DELETE FROM links WHERE user_id IN (
    SELECT id FROM users WHERE email LIKE 'demo.%@qck.sh'
);
DELETE FROM users WHERE email LIKE 'demo.%@qck.sh';

-- Use the same working Argon2id hash from test@example.com
-- This hash is already proven to work in the system

-- Create FREE tier user
INSERT INTO users (
    id, email, password_hash, full_name, company_name,
    subscription_tier, onboarding_status, email_verified,
    created_at, updated_at
) VALUES (
    'f1111111-1111-1111-1111-111111111111',
    'demo.free@qck.sh',
    '$argon2id$v=19$m=19456,t=2,p=1$KPAtRDVwr4dE+YODBkz9tQ$m/fW7O4oLWAXedan3NWL2G7J0v8ofqwEsicX+YA9wV8',
    'Demo Free User',
    'Startup Inc',
    'free',
    'completed',
    true,
    NOW(),
    NOW()
);

-- Create PRO tier user  
INSERT INTO users (
    id, email, password_hash, full_name, company_name,
    subscription_tier, onboarding_status, email_verified,
    created_at, updated_at
) VALUES (
    'f2222222-2222-2222-2222-222222222222',
    'demo.pro@qck.sh',
    '$argon2id$v=19$m=19456,t=2,p=1$KPAtRDVwr4dE+YODBkz9tQ$m/fW7O4oLWAXedan3NWL2G7J0v8ofqwEsicX+YA9wV8',
    'Demo Pro User',
    'Growth Corp',
    'pro',
    'completed',
    true,
    NOW(),
    NOW()
);

-- Create BUSINESS tier user
INSERT INTO users (
    id, email, password_hash, full_name, company_name,
    subscription_tier, onboarding_status, email_verified,
    created_at, updated_at
) VALUES (
    'f3333333-3333-3333-3333-333333333333',
    'demo.business@qck.sh',
    '$argon2id$v=19$m=19456,t=2,p=1$KPAtRDVwr4dE+YODBkz9tQ$m/fW7O4oLWAXedan3NWL2G7J0v8ofqwEsicX+YA9wV8',
    'Demo Business User',
    'Scale LLC',
    'business',
    'completed',
    true,
    NOW(),
    NOW()
);

-- Create ENTERPRISE tier user
INSERT INTO users (
    id, email, password_hash, full_name, company_name,
    subscription_tier, onboarding_status, email_verified,
    created_at, updated_at
) VALUES (
    'f4444444-4444-4444-4444-444444444444',
    'demo.enterprise@qck.sh',
    '$argon2id$v=19$m=19456,t=2,p=1$KPAtRDVwr4dE+YODBkz9tQ$m/fW7O4oLWAXedan3NWL2G7J0v8ofqwEsicX+YA9wV8',
    'Demo Enterprise User',
    'MegaCorp Global',
    'enterprise',
    'completed',
    true,
    NOW(),
    NOW()
);

