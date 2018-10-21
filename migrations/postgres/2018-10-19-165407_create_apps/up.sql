-- Your SQL goes here
CREATE TABLE apps (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    redirect_uri TEXT,
    website TEXT,
    creation_date TIMESTAMP NOT NULL DEFAULT now()
);
