# Plume

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
