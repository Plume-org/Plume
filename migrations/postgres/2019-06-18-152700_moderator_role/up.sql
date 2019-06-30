-- Your SQL goes here
CREATE TYPE user_role AS ENUM ('normal', 'moderator', 'admin');
ALTER TABLE users ADD COLUMN role user_role NOT NULL DEFAULT 'normal';
UPDATE users SET role = 'admin' WHERE is_admin = 't';
ALTER TABLE users DROP COLUMN is_admin;
