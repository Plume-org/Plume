-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN last_fetched_date;
