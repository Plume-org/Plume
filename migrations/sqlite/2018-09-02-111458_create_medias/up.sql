-- Your SQL goes here
CREATE TABLE medias (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL DEFAULT '',
    alt_text TEXT NOT NULL DEFAULT '',
    is_remote BOOLEAN NOT NULL DEFAULT 'f',
    remote_url TEXT,
    sensitive BOOLEAN NOT NULL DEFAULT 'f',
    content_warning TEXT,
    owner_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL
)
