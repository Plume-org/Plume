-- Your SQL goes here
CREATE TABLE api_tokens (
    id SERIAL PRIMARY KEY,
    creation_date TIMESTAMP NOT NULL DEFAULT now(),
    value TEXT NOT NULL,
    scopes TEXT NOT NULL,
    app_id INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
)
