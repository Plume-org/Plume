-- Your SQL goes here
CREATE TABLE notifications (
    id SERIAL PRIMARY KEY,
    title VARCHAR NOT NULL DEFAULT '',
    content TEXT,
    link VARCHAR,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL
)
