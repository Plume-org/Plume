-- This file should undo anything in `up.sql`
ALTER TABLE instances ADD COLUMN local_domain VARCHAR NOT NULL;
