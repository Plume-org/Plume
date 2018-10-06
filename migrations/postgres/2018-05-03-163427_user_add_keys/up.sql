-- Your SQL goes here
ALTER TABLE users ADD COLUMN private_key TEXT;
ALTER TABLE users ADD COLUMN public_key TEXT NOT NULL DEFAULT '';
