-- Your SQL goes here
ALTER TABLE instances ADD COLUMN long_description_html VARCHAR NOT NULL DEFAULT '';
ALTER TABLE instances ADD COLUMN short_description_html VARCHAR NOT NULL DEFAULT '';
