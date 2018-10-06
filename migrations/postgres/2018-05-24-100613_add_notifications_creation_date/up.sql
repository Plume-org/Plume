-- Your SQL goes here
ALTER TABLE notifications ADD COLUMN creation_date TIMESTAMP NOT NULL DEFAULT now();
