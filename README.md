# Plume

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
