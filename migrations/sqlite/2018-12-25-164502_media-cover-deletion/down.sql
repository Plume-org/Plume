-- This file should undo anything in `up.sql`

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

INSERT INTO posts2 SELECT * from posts;
DROP TABLE posts;
ALTER TABLE posts2 RENAME TO posts;
