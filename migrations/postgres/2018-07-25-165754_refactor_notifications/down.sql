-- This file should undo anything in `up.sql`
ALTER TABLE notifications ADD COLUMN title VARCHAR NOT NULL;
ALTER TABLE notifications ADD COLUMN content TEXT;
ALTER TABLE notifications ADD COLUMN link VARCHAR;
ALTER TABLE notifications ADD COLUMN data VARCHAR;

ALTER TABLE notifications DROP COLUMN kind;
ALTER TABLE notifications DROP COLUMN object_id;
