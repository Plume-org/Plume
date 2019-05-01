-- Your SQL goes here

CREATE TABLE timeline_definition(
	id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
	user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
	name VARCHAR NOT NULL,
	query VARCHAR NOT NULL,
	ord INTEGER NOT NULL,
	CONSTRAINT timeline_unique_user_name UNIQUE(user_id, name),
	CONSTRAINT timeline_unique_order UNIQUE(user_id, ord)
);

CREATE TABLE timeline(
	id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
	post_id integer NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
	timeline_id integer NOT NULL REFERENCES timeline_definition(id) ON DELETE CASCADE
);

CREATE TABLE lists(
	id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
	name VARCHAR NOT NULL,
	user_id integer REFERENCES users(id) ON DELETE CASCADE,
	type integer NOT NULL,
	CONSTRAINT timeline_unique_user_name UNIQUE(user_id, name)
);

CREATE TABLE list_elems(
	id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
	list_id integer NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
	user_id integer REFERENCES users(id) ON DELETE CASCADE,
	blog_id integer REFERENCES blogs(id) ON DELETE CASCADE,
	word VARCHAR
);
