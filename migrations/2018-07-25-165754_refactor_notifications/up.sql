-- Your SQL goes here
ALTER TABLE notifications DROP COLUMN title;
ALTER TABLE notifications DROP COLUMN content;
ALTER TABLE notifications DROP COLUMN link;
ALTER TABLE notifications DROP COLUMN data;

ALTER TABLE notifications ADD COLUMN kind VARCHAR NOT NULL DEFAULT 'unknown';
ALTER TABLE notifications ADD COLUMN object_id INTEGER NOT NULL DEFAULT 0;
