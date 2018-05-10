# Plume [![Build Status](https://travis-ci.org/Plume-org/Plume.svg?branch=master)](https://travis-ci.org/Plume-org/Plume)

Federated blogging engine, based on ActivityPub.

## Setup the database

You'll need Postgres.

```
sudo su postgres

psql

CREATE DATABASE plume;
CREATE USER plume WITH PASSWORD 'plume';
GRANT ALL PRIVILEGES ON DATABASE plume to plume;
\q

exit
```

Then run the migrations

```
diesel migrations run # Install diesel with `cargo install diesel_cli` if needed
```

You should repeat this operation every time the database schema has been modified.
A good practice is to run it after every `git pull`.

## Starting the app

Just use:

```
cargo run
```

You'll need Rust nightly.

Once the app started, try to visit [localhost:8000](http://localhost:8000).

To configure the instance (needed before you can do anything else),
go on [/configure](http://localhost:8000/configure).

## Testing the federation

To test the federation, you'll need to setup another database (see "Setup the database"),
also owned by the "plume" user, but with a different name. Then, you'll need to run the
migrations for this database too.

```
diesel migration run --database-url postgres://plume:plume@localhost/my_other_plume_db
```

To run this other instance, you'll need to give two environment variables:

- `ROCKET_PORT`, the port on which your app will run
- `DB_NAME`, the name of the database you just created

```
ROCKET_PORT=3033 DB_NAME=my_other_plume_db cargo run
```
