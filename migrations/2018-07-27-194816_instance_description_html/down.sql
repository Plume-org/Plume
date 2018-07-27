-- This file should undo anything in `up.sql`
ALTER TABLE instances DROP COLUMN long_description_html;
ALTER TABLE instances DROP COLUMN short_description_html;
