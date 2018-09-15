-- This file should undo anything in `up.sql`
ALTER TABLE posts DROP COLUMN creation_date;
ALTER TABLE blogs DROP COLUMN creation_date;
ALTER TABLE users DROP COLUMN creation_date;
ALTER TABLE instances DROP COLUMN creation_date;
