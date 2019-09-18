-- This file should undo anything in `up.sql`

CREATE TABLE blogs_before_themes (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    actor_id VARCHAR NOT NULL,
    title VARCHAR NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    outbox_url VARCHAR NOT NULL UNIQUE,
    inbox_url VARCHAR NOT NULL UNIQUE,
    instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url text not null default '' UNIQUE,
    private_key TEXT,
    public_key TEXT NOT NULL DEFAULT '',
    fqn TEXT NOT NULL DEFAULT '',
    summary_html TEXT NOT NULL DEFAULT '',
    icon_id INTEGER REFERENCES medias(id) ON DELETE SET NULL DEFAULT NULL,
    banner_id INTEGER REFERENCES medias(id) ON DELETE SET NULL DEFAULT NULL,
    CONSTRAINT blog_unique UNIQUE (actor_id, instance_id)
);
INSERT INTO blogs_before_themes SELECT
	id,
    actor_id,
    title,
    summary,
    outbox_url,
    inbox_url,
    instance_id,
    creation_date,
    ap_url,
    private_key,
    public_key,
    fqn,
    summary_html,
    icon_id,
    banner_id
FROM blogs;
DROP TABLE blogs;
ALTER TABLE blogs_before_themes RENAME TO blogs;

CREATE TABLE users_before_themes (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    username VARCHAR NOT NULL,
    display_name VARCHAR NOT NULL DEFAULT '',
    outbox_url VARCHAR NOT NULL UNIQUE,
    inbox_url VARCHAR NOT NULL UNIQUE,
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
    avatar_id INTEGER REFERENCES medias(id) ON DELETE CASCADE,
    last_fetched_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    fqn TEXT NOT NULL DEFAULT '',
    summary_html TEXT NOT NULL DEFAULT '',
    role INTEGER NOT NULL DEFAULT 2,
    FOREIGN KEY (avatar_id) REFERENCES medias(id) ON DELETE SET NULL,
    CONSTRAINT blog_authors_unique UNIQUE (username, instance_id)
);
INSERT INTO users_before_themes SELECT
	id,
    username,
    display_name,
    outbox_url,
    inbox_url,
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
    fqn,
    summary_html,
    role
FROM users;
DROP TABLE users;
ALTER TABLE users_before_themes RENAME TO users;
