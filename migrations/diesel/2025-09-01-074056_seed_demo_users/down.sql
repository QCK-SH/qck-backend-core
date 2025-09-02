-- Remove demo users and their data
DELETE FROM links WHERE user_id IN (
    SELECT id FROM users WHERE email LIKE 'demo.%@qck.sh'
);

DELETE FROM users WHERE email LIKE 'demo.%@qck.sh';