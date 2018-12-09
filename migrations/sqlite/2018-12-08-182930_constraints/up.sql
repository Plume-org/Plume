-- Your SQL goes here
CREATE TABLE api_tokens2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    value TEXT NOT NULL UNIQUE,
    scopes TEXT NOT NULL,
    app_id INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO api_tokens2 SELECT * FROM api_tokens;
DROP TABLE api_tokens;
ALTER TABLE api_tokens2 RENAME TO api_tokens;

CREATE TABLE blog_authors2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    blog_id INTEGER REFERENCES blogs(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    is_owner BOOLEAN NOT NULL DEFAULT 'f',
    CONSTRAINT blog_authors_unique UNIQUE (blog_id, author_id)
);

INSERT INTO blog_authors2 SELECT * FROM blog_authors;
DROP TABLE blog_authors;
ALTER TABLE blog_authors2 RENAME TO blog_authors;

CREATE TABLE blogs2 (
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
    CONSTRAINT blog_unique UNIQUE (actor_id, instance_id)
);

INSERT INTO blogs2 SELECT * FROM blogs;
DROP TABLE blogs;
ALTER TABLE blogs2 RENAME TO blogs;

CREATE TABLE comments2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL DEFAULT '',
    in_response_to_id INTEGER REFERENCES comments(id),
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url VARCHAR UNIQUE,
    sensitive BOOLEAN NOT NULL DEFAULT 'f',
    spoiler_text TEXT NOT NULL DEFAULT ''
);

INSERT INTO comments2 SELECT * FROM comments;
DROP TABLE comments;
ALTER TABLE comments2 RENAME TO comments;

CREATE TABLE follows2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    follower_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    following_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    ap_url TEXT NOT NULL default '' UNIQUE
);

INSERT INTO follows2 SELECT * FROM follows;
DROP TABLE follows;
ALTER TABLE follows2 RENAME TO follows;

CREATE TABLE instances2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    public_domain VARCHAR NOT NULL UNIQUE,
    name VARCHAR NOT NULL,
    local BOOLEAN NOT NULL DEFAULT 'f',
    blocked BOOLEAN NOT NULL DEFAULT 'f',
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    open_registrations BOOLEAN NOT NULL DEFAULT 't',
    short_description TEXT NOT NULL DEFAULT '',
    long_description TEXT NOT NULL DEFAULT '',
    default_license TEXT NOT NULL DEFAULT 'CC-BY-SA',
    long_description_html VARCHAR NOT NULL DEFAULT '',
    short_description_html VARCHAR NOT NULL DEFAULT ''
);

INSERT INTO instances2 SELECT * FROM instances;
DROP TABLE instances;
ALTER TABLE instances2 RENAME TO instances;

CREATE TABLE likes2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url VARCHAR NOT NULL default '' UNIQUE,
    CONSTRAINT likes_unique UNIQUE (user_id, post_id)
);

INSERT INTO likes2 SELECT * FROM likes;
DROP TABLE likes;
ALTER TABLE likes2 RENAME TO likes;

CREATE TABLE mentions2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    mentioned_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE,
    comment_id INTEGER REFERENCES comments(id) ON DELETE CASCADE,
    ap_url VARCHAR NOT NULL DEFAULT '' UNIQUE
);

INSERT INTO mentions2 SELECT * FROM mentions;
DROP TABLE mentions;
ALTER TABLE mentions2 RENAME TO mentions;

CREATE TABLE post_authors2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    CONSTRAINT blog_authors_unique UNIQUE (post_id, author_id)
);

INSERT INTO post_authors2 SELECT * FROM post_authors;
DROP TABLE post_authors;
ALTER TABLE post_authors2 RENAME TO post_authors;

CREATE TABLE posts2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    blog_id INTEGER REFERENCES blogs(id) ON DELETE CASCADE NOT NULL,
    slug VARCHAR NOT NULL,
    title VARCHAR NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    published BOOLEAN NOT NULL DEFAULT 'f',
    license VARCHAR NOT NULL DEFAULT 'CC-BY-SA',
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url VARCHAR NOT NULL DEFAULT '' UNIQUE,
    subtitle TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    cover_id INTEGER REFERENCES medias(id) DEFAULT NULL,
    CONSTRAINT blog_authors_unique UNIQUE (blog_id, slug)
);

INSERT INTO posts2 SELECT * FROM posts;
DROP TABLE posts;
ALTER TABLE posts2 RENAME TO posts;

CREATE TABLE tags2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    tag TEXT NOT NULL DEFAULT '',
    is_hashtag BOOLEAN NOT NULL DEFAULT 'f',
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    CONSTRAINT blog_authors_unique UNIQUE (tag, is_hashtag, post_id)
);

INSERT INTO tags2 SELECT * FROM tags;
DROP TABLE tags;
ALTER TABLE tags2 RENAME TO tags;

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
    avatar_id INTEGER REFERENCES medias(id) ON DELETE CASCADE,
    last_fetched_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (avatar_id) REFERENCES medias(id) ON DELETE SET NULL,
    CONSTRAINT blog_authors_unique UNIQUE (username, instance_id)
);

INSERT INTO users2 SELECT * FROM users;
DROP TABLE users;
ALTER TABLE users2 RENAME TO users;

