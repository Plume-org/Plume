-- This file should undo anything in `up.sql`
ALTER TABLE comment_seers RENAME TO tmp_comment_seers;

CREATE TABLE comment_seers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    comment_id INTEGER REFERENCES comments(id) ON DELETE CASCADE NOT NULL,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    UNIQUE (comment_id, user_id)
);

INSERT INTO comment_seers(id, comment_id, user_id)
SELECT id, comment_id, user_id
FROM tmp_comment_seers;

DROP TABLE tmp_comment_seers;
