-- Your SQL goes here
PRAGMA foreign_keys = ON;
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username VARCHAR NOT NULL,
    display_name VARCHAR NOT NULL DEFAULT '',
    outbox_url VARCHAR NOT NULL,
    inbox_url VARCHAR NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT 'f',
    summary TEXT NOT NULL DEFAULT '',
    email TEXT,
    hashed_password TEXT,
    instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL,
    creation_date INTEGER NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url TEXT NOT NULL default '',
    private_key TEXT,
    public_key TEXT NOT NULL DEFAULT '',
    shared_inbox_url VARCHAR,
    followers_endpoint VARCHAR NOT NULL DEFAULT '',
    avatar_id INTEGER REFERENCES medias(id) ON DELETE CASCADE,
    last_fetched_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (avatar_id) REFERENCES medias(id) ON DELETE SET NULL
);
