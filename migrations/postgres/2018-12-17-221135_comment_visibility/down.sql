-- This file should undo anything in `up.sql`
ALTER TABLE comments DROP COLUMN public_visibility;

DROP TABLE comment_seers;
