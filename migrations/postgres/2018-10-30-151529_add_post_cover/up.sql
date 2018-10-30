-- Your SQL goes here
ALTER TABLE posts ADD COLUMN cover_id INTEGER REFERENCES medias(id) DEFAULT NULL;
