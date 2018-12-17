-- Your SQL goes here
CREATE TABLE mentions2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    mentioned_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE,
    comment_id INTEGER REFERENCES comments(id) ON DELETE CASCADE
);

INSERT INTO mentions2 SELECT id,mentioned_id,post_id,comment_id FROM mentions;
DROP TABLE mentions;
ALTER TABLE mentions2 RENAME TO mentions;
