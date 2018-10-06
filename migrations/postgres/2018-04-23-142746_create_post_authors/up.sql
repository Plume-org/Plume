-- Your SQL goes here
CREATE TABLE post_authors (
    id SERIAL PRIMARY KEY,
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL
)
