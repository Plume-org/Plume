-- Your SQL goes here
ALTER TABLE users ADD COLUMN avatar_id INTEGER REFERENCES medias(id) ON DELETE CASCADE;
