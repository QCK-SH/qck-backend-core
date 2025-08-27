-- Change ip_address column from INET to TEXT in refresh_tokens table
ALTER TABLE refresh_tokens 
ALTER COLUMN ip_address TYPE TEXT USING ip_address::TEXT;