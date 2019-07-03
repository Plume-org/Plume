-- Your SQL goes here
ALTER TABLE users ADD COLUMN role INTEGER NOT NULL DEFAULT 2;
UPDATE users SET role = 0 WHERE is_admin = 't';
ALTER TABLE users DROP COLUMN is_admin;
