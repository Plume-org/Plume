-- This file should undo anything in `up.sql`
ALTER TABLE mentions ADD COLUMN ap_url VARCHAR NOT NULL DEFAULT '';
