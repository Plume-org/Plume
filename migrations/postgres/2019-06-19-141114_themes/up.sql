-- Your SQL goes here
ALTER TABLE blogs ADD COLUMN custom_css TEXT;
ALTER TABLE users ADD COLUMN preferred_theme VARCHAR;
ALTER TABLE users ADD COLUMN hide_custom_css BOOLEAN NOT NULL DEFAULT 'f';
