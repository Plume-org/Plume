-- This file should undo anything in `up.sql`
ALTER TABLE users ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT 'f';
UPDATE users SET is_admin = 't' WHERE role = 0;
ALTER TABLE users DROP COLUMN role;
