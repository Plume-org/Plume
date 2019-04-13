-- Your SQL goes here

CREATE TABLE timeline_definition(
	id SERIAL PRIMARY KEY,
	user_id integer REFERENCES users ON DELETE CASCADE,
	name VARCHAR NOT NULL,
	query VARCHAR NOT NULL
);

CREATE TABLE timeline(
	id SERIAL PRIMARY KEY,
	post_id integer NOT NULL REFERENCES posts ON DELETE CASCADE,
	timeline_id integer NOT NULL REFERENCES timeline_definition ON DELETE CASCADE
);

CREATE TABLE lists(
	id SERIAL PRIMARY KEY,
	name VARCHAR NOT NULL,
	user_id integer REFERENCES users ON DELETE CASCADE,
	type CHAR NOT NULL
);

CREATE TABLE list_elems(
	id SERIAL PRIMARY KEY,
	list_id integer NOT NULL REFERENCES lists ON DELETE CASCADE,
	blog_id integer REFERENCES blogs ON DELETE CASCADE,
	user_id integer REFERENCES users ON DELETE CASCADE,
	word VARCHAR,
	CONSTAINT list_elems_one_no_null CHECK(
		CASE WHEN blog_id IS NOT NULL THEN 1 ELSE 0 END +
		CASE WHEN user_id IS NOT NULL THEN 1 ELSE 0 END +
		CASE WHEN word IS NOT NULL THEN 1 ELSE 0 END
		= 1)
);
