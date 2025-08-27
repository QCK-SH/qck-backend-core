-- Add user profile fields to users table
ALTER TABLE users 
ADD COLUMN full_name VARCHAR(255) NOT NULL DEFAULT '',
ADD COLUMN company_name VARCHAR(255);

-- Remove default from full_name after adding column
ALTER TABLE users 
ALTER COLUMN full_name DROP DEFAULT;