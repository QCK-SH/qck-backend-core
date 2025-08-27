-- Remove user profile fields from users table
ALTER TABLE users 
DROP COLUMN full_name,
DROP COLUMN company_name;