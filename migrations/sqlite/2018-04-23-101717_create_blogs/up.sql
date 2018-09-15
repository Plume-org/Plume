-- Your SQL goes here
CREATE TABLE blogs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_id VARCHAR NOT NULL,
    title VARCHAR NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    outbox_url VARCHAR NOT NULL,
    inbox_url VARCHAR NOT NULL,
    instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL,
    creation_date INTEGER NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url text not null default '',
    private_key TEXT,
    public_key TEXT NOT NULL DEFAULT ''
)
