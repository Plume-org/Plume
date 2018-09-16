-- Your SQL goes here
CREATE TABLE instances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    public_domain VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    local BOOLEAN NOT NULL DEFAULT 'f',
    blocked BOOLEAN NOT NULL DEFAULT 'f',
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    open_registrations BOOLEAN NOT NULL DEFAULT 't',
    short_description TEXT NOT NULL DEFAULT '',
    long_description TEXT NOT NULL DEFAULT '',
    default_license TEXT NOT NULL DEFAULT 'CC-0',
    long_description_html VARCHAR NOT NULL DEFAULT '',
    short_description_html VARCHAR NOT NULL DEFAULT ''
)
