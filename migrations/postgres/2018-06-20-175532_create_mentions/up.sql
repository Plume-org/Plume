-- Your SQL goes here
CREATE TABLE mentions (
    id SERIAL PRIMARY KEY,
    mentioned_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE,
    comment_id INTEGER REFERENCES comments(id) ON DELETE CASCADE
)
