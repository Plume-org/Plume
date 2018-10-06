-- This file should undo anything in `up.sql`
ALTER TABLE blogs DROP COLUMN private_key;
ALTER TABLE blogs DROP COLUMN public_key;
