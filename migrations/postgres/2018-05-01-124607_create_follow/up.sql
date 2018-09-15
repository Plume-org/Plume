-- Your SQL goes here
CREATE TABLE follows (
    id SERIAL PRIMARY KEY,
    follower_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    following_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL
)
