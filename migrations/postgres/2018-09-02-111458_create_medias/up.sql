-- Your SQL goes here
CREATE TABLE medias (
    id SERIAL PRIMARY KEY,
    file_path TEXT NOT NULL DEFAULT '',
    alt_text TEXT NOT NULL DEFAULT '',
    is_remote BOOLEAN NOT NULL DEFAULT 'f',
    remote_url TEXT,
    sensitive BOOLEAN NOT NULL DEFAULT 'f',
    content_warning TEXT
)
