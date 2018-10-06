-- Your SQL goes here
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username VARCHAR NOT NULL,
    display_name VARCHAR NOT NULL DEFAULT '',
    outbox_url VARCHAR NOT NULL,
    inbox_url VARCHAR NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT 'f',
    summary TEXT NOT NULL DEFAULT '',
    email TEXT,
    hashed_password TEXT,
    instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL
);
