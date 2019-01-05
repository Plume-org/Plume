-- Your SQL goes here

ALTER TABLE posts DROP CONSTRAINT posts_cover_id_fkey;
ALTER TABLE posts ADD CONSTRAINT posts_cover_id_fkey FOREIGN KEY (cover_id) REFERENCES medias(id) ON DELETE SET NULL;
