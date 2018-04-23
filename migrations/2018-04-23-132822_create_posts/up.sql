-- Your SQL goes here
CREATE TABLE posts (
    id SERIAL PRIMARY KEY,
    blog_id INTEGER REFERENCES blogs(id) ON DELETE CASCADE NOT NULL,
    slug VARCHAR NOT NULL,
    title VARCHAR NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    published BOOLEAN NOT NULL DEFAULT 'f',
    license VARCHAR NOT NULL DEFAULT 'CC-0'
)
