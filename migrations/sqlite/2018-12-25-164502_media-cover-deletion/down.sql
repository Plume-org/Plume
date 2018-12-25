-- This file should undo anything in `up.sql`

ALTER TABLE posts DROP CONSTRAINT posts_cover_id_fkey;
ALTER TABLE posts ADD CONSTRAINT posts_cover_id_fkey FOREIGN KEY (cover_id) REFERENCES medias(id);
