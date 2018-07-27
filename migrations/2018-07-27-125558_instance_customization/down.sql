-- This file should undo anything in `up.sql`
ALTER TABLE instances DROP COLUMN open_registrations;
ALTER TABLE instances DROP COLUMN short_description;
ALTER TABLE instances DROP COLUMN long_description;
ALTER TABLE instances DROP COLUMN default_license;
