-- Your SQL goes here
-- SQLite is great, we can't just change the default value,
-- we have to clone the table with the new value.
CREATE TABLE instances2 (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    public_domain VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    local BOOLEAN NOT NULL DEFAULT 'f',
    blocked BOOLEAN NOT NULL DEFAULT 'f',
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    open_registrations BOOLEAN NOT NULL DEFAULT 't',
    short_description TEXT NOT NULL DEFAULT '',
    long_description TEXT NOT NULL DEFAULT '',
    default_license TEXT NOT NULL DEFAULT 'CC-BY-SA',
    long_description_html VARCHAR NOT NULL DEFAULT '',
    short_description_html VARCHAR NOT NULL DEFAULT ''
);

INSERT INTO instances2 SELECT * FROM instances;
DROP TABLE instances;
ALTER TABLE instances2 RENAME TO instances;


 CREATE TABLE posts2 (
     id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
     blog_id INTEGER REFERENCES blogs(id) ON DELETE CASCADE NOT NULL,
     slug VARCHAR NOT NULL,
     title VARCHAR NOT NULL,
     content TEXT NOT NULL DEFAULT '',
     published BOOLEAN NOT NULL DEFAULT 'f',
     license VARCHAR NOT NULL DEFAULT 'CC-BY-SA',
     creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
     ap_url VARCHAR NOT NULL DEFAULT '',
     subtitle TEXT NOT NULL DEFAULT '',
     source TEXT NOT NULL DEFAULT ''
 );

 INSERT INTO posts2 SELECT * FROM posts;
 DROP TABLE posts;
 ALTER TABLE posts2 RENAME TO posts;
