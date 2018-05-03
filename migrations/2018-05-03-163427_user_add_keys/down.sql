-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN private_key;
ALTER TABLE users DROP COLUMN public_key;
