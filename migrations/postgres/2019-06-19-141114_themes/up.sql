-- Your SQL goes here
ALTER TABLE blogs ADD COLUMN theme VARCHAR;
ALTER TABLE users ADD COLUMN preferred_theme VARCHAR;
ALTER TABLE users ADD COLUMN hide_custom_css BOOLEAN NOT NULL DEFAULT 'f';
