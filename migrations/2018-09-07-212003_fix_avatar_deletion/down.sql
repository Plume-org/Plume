-- This file should undo anything in `up.sql`
ALTER TABLE users
    DROP CONSTRAINT users_avatar_id_fkey,
    ADD CONSTRAINT users_avatar_id_fkey
    FOREIGN KEY (avatar_id)
    REFERENCES medias(id)
    ON DELETE CASCADE;
