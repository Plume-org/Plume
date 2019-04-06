-- Your SQL goes here
ALTER TABLE users ADD COLUMN summary_html TEXT NOT NULL DEFAULT '';
UPDATE users SET summary_html = summary;
