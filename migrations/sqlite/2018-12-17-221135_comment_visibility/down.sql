-- This file should undo anything in `up.sql`
CREATE TABLE comments2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL DEFAULT '',
    in_response_to_id INTEGER REFERENCES comments(id),
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ap_url VARCHAR,
    sensitive BOOLEAN NOT NULL DEFAULT 'f',
    spoiler_text TEXT NOT NULL DEFAULT ''
);

INSERT INTO comments2 SELECT
        id,
        content,
        in_response_to_id,
        post_id,
        author_id,
        creation_date,
        ap_url,
        sensitive,
        spoiler_text
    FROM comments;
DROP TABLE comments;
ALTER TABLE comments2 RENAME TO comments;

DROP TABLE comment_seers;
