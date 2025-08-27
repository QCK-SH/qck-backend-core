-- Change ip_address column from INET to TEXT to simplify Diesel compatibility
ALTER TABLE password_reset_tokens 
ALTER COLUMN ip_address TYPE TEXT USING ip_address::TEXT;