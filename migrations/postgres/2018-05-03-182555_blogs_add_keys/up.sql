-- Your SQL goes here
ALTER TABLE blogs ADD COLUMN private_key TEXT;
ALTER TABLE blogs ADD COLUMN public_key TEXT NOT NULL DEFAULT '';
