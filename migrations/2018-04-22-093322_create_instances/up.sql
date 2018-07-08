l-- Your SQL goes here
CREATE TABLE instances (
    id SERIAL PRIMARY KEY,
    local_domain VARCHAR NOT NULL,
    public_domain VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    local BOOLEAN NOT NULL DEFAULT 'f',
    blocked BOOLEAN NOT NULL DEFAULT 'f'
)
