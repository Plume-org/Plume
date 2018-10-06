-- This file should undo anything in `up.sql`
ALTER TABLE ONLY posts ALTER COLUMN license SET DEFAULT 'CC-0';
ALTER TABLE ONLY instances ALTER COLUMN default_license SET DEFAULT 'CC-0';
