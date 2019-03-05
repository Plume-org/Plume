-- This file should undo anything in `up.sql`
ALTER TABLE blogs DROP COLUMN fqn;
ALTER TABLE users DROP COLUMN fqn;