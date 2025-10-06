-- Add onboarding status to users table
ALTER TABLE users
ADD COLUMN onboarding_status VARCHAR(50) NOT NULL DEFAULT 'registered';

-- Update existing users to completed status
UPDATE users
SET onboarding_status = 'completed'
WHERE email_verified = true;

-- Add check constraint for valid onboarding statuses
ALTER TABLE users
ADD CONSTRAINT valid_onboarding_status CHECK (
    onboarding_status IN ('registered', 'completed')
);
