-- This file should undo anything in `up.sql

-- TODO: do it correctly once other PR are merged and we know what the schema is

ALTER TABLE blogs DROP COLUMN summary_html;
ALTER TABLE blogs DROP COLUMN icon_id;
ALTER TABLE blogs DROP COLUMN banner_id;