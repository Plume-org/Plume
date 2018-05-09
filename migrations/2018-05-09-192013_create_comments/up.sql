-- Your SQL goes here
CREATE TABLE comments (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL DEFAULT '',
    in_response_to_id INTEGER REFERENCES comments(id),    
    post_id INTEGER REFERENCES posts(id) ON DELETE CASCADE NOT NULL,
    author_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    creation_date TIMESTAMP NOT NULL DEFAULT now(),
    ap_url VARCHAR,
    sensitive BOOLEAN NOT NULL DEFAULT 'f',
    spoiler_text TEXT NOT NULL DEFAULT ''
)
