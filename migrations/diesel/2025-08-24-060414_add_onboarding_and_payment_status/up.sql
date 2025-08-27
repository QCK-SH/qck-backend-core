-- Add onboarding status to users table to track registration flow progress
ALTER TABLE users 
ADD COLUMN onboarding_status VARCHAR(50) NOT NULL DEFAULT 'registered';

-- Update existing users to have correct onboarding status based on their state
UPDATE users 
SET onboarding_status = CASE 
    WHEN email_verified = true AND subscription_tier != 'pending' THEN 'completed'
    WHEN email_verified = true THEN 'verified'
    ELSE 'registered'
END;

-- Add check constraint for valid onboarding statuses
ALTER TABLE users 
ADD CONSTRAINT valid_onboarding_status CHECK (
    onboarding_status IN (
        'registered',           -- Just registered, needs email verification
        'verified',            -- Email verified, needs to select plan
        'plan_selected',       -- Plan selected, needs payment (if pro)
        'payment_pending',     -- Payment initiated but not completed
        'completed'           -- Onboarding completed, full access
    )
);

-- Create separate payments table to track payment information
CREATE TABLE payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Payment provider information
    provider VARCHAR(50) NOT NULL, -- 'stripe', 'paypal', etc.
    provider_customer_id VARCHAR(255), -- e.g., Stripe customer ID
    provider_payment_id VARCHAR(255), -- e.g., Stripe payment intent ID
    provider_subscription_id VARCHAR(255), -- e.g., Stripe subscription ID
    
    -- Payment details
    amount INTEGER NOT NULL, -- Amount in cents
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    payment_method VARCHAR(50), -- 'card', 'bank_transfer', etc.
    
    -- Subscription tier this payment is for
    subscription_tier VARCHAR(50) NOT NULL,
    billing_period VARCHAR(20), -- 'monthly', 'yearly'
    
    -- Additional metadata
    metadata JSONB DEFAULT '{}',
    failure_reason TEXT,
    
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    refunded_at TIMESTAMPTZ
);

-- Add check constraint for valid payment statuses
ALTER TABLE payments 
ADD CONSTRAINT valid_payment_status CHECK (
    status IN (
        'pending',
        'processing',
        'completed',
        'failed',
        'cancelled',
        'refunded',
        'partially_refunded'
    )
);

-- Add check constraint for valid payment providers
ALTER TABLE payments 
ADD CONSTRAINT valid_payment_provider CHECK (
    provider IN (
        'stripe',
        'paypal',
        'manual',
        'test'
    )
);

-- Add indexes for common queries
CREATE INDEX idx_payments_user_id ON payments(user_id);
CREATE INDEX idx_payments_status ON payments(status);
CREATE INDEX idx_payments_provider ON payments(provider);
CREATE INDEX idx_payments_created_at ON payments(created_at DESC);

-- Add unique constraint to prevent duplicate provider payment IDs
CREATE UNIQUE INDEX idx_payments_provider_payment_id 
ON payments(provider_payment_id) 
WHERE provider_payment_id IS NOT NULL;
