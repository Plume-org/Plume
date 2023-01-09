ALTER TABLE users ADD CONSTRAINT users_fqn UNIQUE (fqn);
ALTER TABLE blogs ADD CONSTRAINT blogs_actor_id UNIQUE (actor_id);
ALTER TABLE blogs ADD CONSTRAINT blogs_fqn UNIQUE (fqn);
