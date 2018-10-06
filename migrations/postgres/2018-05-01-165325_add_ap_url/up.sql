-- Your SQL goes here
ALTER TABLE blogs ADD COLUMN ap_url TEXT NOT NULL default '';
ALTER TABLE users ADD COLUMN ap_url TEXT NOT NULL default '';
