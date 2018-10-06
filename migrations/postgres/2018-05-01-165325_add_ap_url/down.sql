-- This file should undo anything in `up.sql`
ALTER TABLE blogs DROP COLUMN ap_url;
ALTER TABLE users DROP COLUMN ap_url;
