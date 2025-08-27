-- Revert ip_address column from TEXT to INET in refresh_tokens table
ALTER TABLE refresh_tokens 
ALTER COLUMN ip_address TYPE INET USING ip_address::INET;