-- This file should undo anything in `up.sql`
DELETE FROM timeline_definition WHERE name = 'Your feed';
DELETE FROM timeline_definition WHERE name = 'Local feed' AND query = 'local';
DELETE FROM timeline_definition WHERE name = 'Federared feed' AND query = 'all';
