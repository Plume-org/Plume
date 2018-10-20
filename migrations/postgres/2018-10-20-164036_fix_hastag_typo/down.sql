-- This file should undo anything in `up.sql`
ALTER TABLE tags RENAME COLUMN is_hashtag TO is_hastag;
