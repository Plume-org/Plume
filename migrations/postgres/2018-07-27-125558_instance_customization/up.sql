-- Your SQL goes here
ALTER TABLE instances ADD COLUMN open_registrations BOOLEAN NOT NULL DEFAULT 't';
ALTER TABLE instances ADD COLUMN short_description TEXT NOT NULL DEFAULT '';
ALTER TABLE instances ADD COLUMN long_description TEXT NOT NULL DEFAULT '';
ALTER TABLE instances ADD COLUMN default_license TEXT NOT NULL DEFAULT 'CC-0';
