-- Update the checksums to match the current files
UPDATE _sqlx_migrations 
SET checksum = E'\\x43e8a56b1ea1e843cab2663f88e25510d237f181a1f1ee662b9cf1580bfb92be131eb96ccae6a2eadabc8c7c5a26ba19'
WHERE version = 20250108000002;

UPDATE _sqlx_migrations 
SET checksum = E'\\xa2e019a7d2eb9af58c2aefff76e6d1a643732cdb13bd995075562a3a233a51de493ff9ba5e74a6eeed3f43af9da8bca5'
WHERE version = 20250109000003;
