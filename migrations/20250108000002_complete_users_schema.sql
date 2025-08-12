-- Complete users table schema to meet DEV-86 requirements
-- Migration: 20250108_000002_complete_users_schema.sql
-- 
-- IMPORTANT: This migration depends on 20250108_000001_initial_schema.sql which creates:
--   - users table with email_verified BOOLEAN column (line 15)
--   - idx_users_email index (line 113)
-- This migration enhances the existing schema by adding timestamp tracking and optimizations
--
-- NOTE: The email_verified BOOLEAN column already exists from the initial migration.
-- This migration adds email_verified_at TIMESTAMPTZ to track WHEN verification occurred.

-- 1. Create subscription tier enum type (only if it doesn't exist)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'subscription_tier') THEN
        CREATE TYPE subscription_tier AS ENUM ('free', 'pro');
        RAISE NOTICE 'Created subscription_tier enum type';
    ELSE
        RAISE NOTICE 'subscription_tier enum type already exists';
    END IF;
END $$;

-- 2. Alter existing columns and add new ones (check what exists first)
DO $$
BEGIN
    -- Update email column if needed
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email' AND character_maximum_length != 320) THEN
        ALTER TABLE users ALTER COLUMN email TYPE VARCHAR(320);
        RAISE NOTICE 'Updated users.email column to VARCHAR(320)';
    END IF;
    
    -- Update password_hash column if needed
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'password_hash' AND data_type != 'text') THEN
        ALTER TABLE users ALTER COLUMN password_hash TYPE TEXT;
        RAISE NOTICE 'Updated users.password_hash column to TEXT';
    END IF;
    
    -- Add subscription_tier column if it doesn't exist
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'subscription_tier') THEN
        ALTER TABLE users ADD COLUMN subscription_tier subscription_tier DEFAULT 'free' NOT NULL;
        RAISE NOTICE 'Added users.subscription_tier column';
    END IF;
    
    -- Add email_verified_at column if it doesn't exist
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email_verified_at') THEN
        ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;
        RAISE NOTICE 'Added users.email_verified_at column';
    END IF;
END $$;

-- 3. Drop existing email index to recreate as case-insensitive
-- Note: idx_users_email was created in migration 20250108_000001_initial_schema.sql
DROP INDEX IF EXISTS idx_users_email;

-- 4. Create case-insensitive index on email
-- Using lower() function index for case-insensitive searches
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_ci 
    ON users(LOWER(email));

-- 5. Create partial index on unverified emails for performance
-- This helps quickly find users who haven't verified their email
-- The condition 'IS NOT TRUE' includes both FALSE and NULL values
CREATE INDEX IF NOT EXISTS idx_users_unverified_emails 
    ON users(email) 
    WHERE email_verified IS NOT TRUE;

-- 6. Create composite index for subscription tier queries
-- Useful for finding users by tier and when they joined
CREATE INDEX IF NOT EXISTS idx_users_subscription_created 
    ON users(subscription_tier, created_at DESC);

-- 7. Update email verification logic
-- When email is verified, set the timestamp
CREATE OR REPLACE FUNCTION update_email_verified_at()
RETURNS TRIGGER AS $$
BEGIN
    -- Handle email verification timestamp tracking
    -- Support both initial verification and re-verification scenarios
    IF TG_OP = 'UPDATE' THEN
        -- Track verification timestamp for any transition to verified state
        -- This allows re-verification tracking while preserving existing timestamps
        IF NEW.email_verified = TRUE AND OLD.email_verified IS NOT TRUE THEN
            -- Always update timestamp when transitioning to verified
            -- This tracks both initial and re-verifications
            NEW.email_verified_at = NOW();
        ELSIF NEW.email_verified = TRUE AND OLD.email_verified = TRUE AND NEW.email_verified_at IS NULL THEN
            -- Edge case: Already verified but timestamp is missing, set it now
            NEW.email_verified_at = NOW();
        END IF;
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.email_verified = TRUE AND NEW.email_verified_at IS NULL THEN
            NEW.email_verified_at = NOW();
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger only if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'trigger_update_email_verified_at') THEN
        CREATE TRIGGER trigger_update_email_verified_at
            BEFORE INSERT OR UPDATE ON users
            FOR EACH ROW
            EXECUTE FUNCTION update_email_verified_at();
        RAISE NOTICE 'Created email verification trigger';
    ELSE
        RAISE NOTICE 'Email verification trigger already exists';
    END IF;
END $$;

-- 8. Add comments for documentation
COMMENT ON COLUMN users.subscription_tier IS 'User subscription level: free or pro';
COMMENT ON COLUMN users.email_verified_at IS 'Timestamp when email was verified';
COMMENT ON INDEX idx_users_email_ci IS 'Case-insensitive unique index for email lookups';
COMMENT ON INDEX idx_users_unverified_emails IS 'Partial index for finding unverified users efficiently';
COMMENT ON INDEX idx_users_subscription_created IS 'Composite index for subscription tier analytics';