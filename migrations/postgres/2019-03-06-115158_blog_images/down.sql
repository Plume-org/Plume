-- This file should undo anything in `up.sql`
ALTER TABLE blogs DROP COLUMN summary_html;
ALTER TABLE blogs DROP COLUMN icon_id;
ALTER TABLE blogs DROP COLUMN banner_id;