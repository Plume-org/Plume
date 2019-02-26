-- Your SQL goes here
ALTER TABLE comments ADD public_visibility BOOLEAN NOT NULL DEFAULT 't';

CREATE TABLE comment_seers (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    comment_id INTEGER REFERENCES comments(id) ON DELETE CASCADE NOT NULL,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    UNIQUE (comment_id, user_id)
)
