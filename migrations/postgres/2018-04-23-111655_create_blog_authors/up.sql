-- Your SQL goes here
CREATE TABLE blog_authors (
    id SERIAL PRIMARY KEY,
    blog_id INTEGER REFERENCES blogs(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    is_owner BOOLEAN NOT NULL DEFAULT 'f'
)
