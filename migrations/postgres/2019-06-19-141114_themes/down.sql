-- This file should undo anything in `up.sql`
ALTER TABLE blogs DROP COLUMN custom_css;
ALTER TABLE users DROP COLUMN preferred_theme;
ALTER TABLE users DROP COLUMN hide_custom_css;
