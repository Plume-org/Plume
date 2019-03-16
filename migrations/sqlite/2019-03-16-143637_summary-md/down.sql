-- This file should undo anything in `up.sql`
CREATE TABLE users2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    username VARCHAR NOT NULL,
    display_name VARCHAR NOT NULL DEFAULT '',
    outbox_url VARCHAR NOT NULL UNIQUE,
    inbox_url VARCHAR NOT NULL UNIQUE,
    is_admin BOOLEAN NOT NULL DEFAULT 'f',
    summary TEXT NOT NULL DEFAULT '',
    email TEXT,
    hashed_password TEXT,
    instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url TEXT NOT NULL default '' UNIQUE,
    private_key TEXT,
    public_key TEXT NOT NULL DEFAULT '',
    shared_inbox_url VARCHAR,
    followers_endpoint VARCHAR NOT NULL DEFAULT '' UNIQUE,
    avatar_id INTEGER REFERENCES medias(id) ON DELETE SET NULL,
    last_fetched_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    fqn TEXT NOT NULL DEFAULT ''
    CONSTRAINT blog_authors_unique UNIQUE (username, instance_id)
);

INSERT INTO users2 SELECT
    id,
    username,
    display_name,
    outbox_url,
    inbox_url,
    is_admin,
    summary,
    email,
    hashed_password,
    instance_id,
    creation_date,
    ap_url,
    private_key,
    public_key,
    shared_inbox_url,
    followers_endpoint,
    avatar_id,
    last_fetched_date,
    fqn
FROM users;
DROP TABLE users;
ALTER TABLE users2 RENAME TO users;
