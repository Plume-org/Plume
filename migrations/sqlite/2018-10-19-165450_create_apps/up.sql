-- Your SQL goes here
CREATE TABLE apps (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '',
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    redirect_uri TEXT,
    website TEXT,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
