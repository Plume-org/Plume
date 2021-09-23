CREATE TABLE instances_old (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
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
);
INSERT INTO instances_old SELECT
    id,
    public_domain,
    name,
    local,
    blocked,
    creation_date,
    open_registrations,
    short_description,
    long_description,
    default_license,
    long_description_html,
    short_description_html
FROM instances;
DROP TABLE instances;
ALTER TABLE instances_old RENAME TO instances;
