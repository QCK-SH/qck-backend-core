-- Drop payments table and related indexes
DROP TABLE IF EXISTS payments;

-- Remove onboarding_status from users table
ALTER TABLE users DROP CONSTRAINT IF EXISTS valid_onboarding_status;
ALTER TABLE users DROP COLUMN IF EXISTS onboarding_status;
