-- Your SQL goes here
ALTER TABLE users ADD COLUMN is_moderator BOOLEAN NOT NULL DEFAULT 'f';
UPDATE users SET is_moderator = is_admin;
