-- Revert ip_address column from TEXT back to INET
ALTER TABLE password_reset_tokens 
ALTER COLUMN ip_address TYPE INET USING ip_address::INET;