-- Your SQL goes here
ALTER TABLE blogs ADD COLUMN fqn TEXT NOT NULL DEFAULT '';
UPDATE blogs SET fqn =
	(CASE WHEN (SELECT local FROM instances WHERE id = instance_id) THEN
		actor_id
	ELSE
		(actor_id || '@' || (SELECT public_domain FROM instances WHERE id = instance_id LIMIT 1))
	END)
WHERE fqn = '';

ALTER TABLE users ADD COLUMN fqn TEXT NOT NULL DEFAULT '';
UPDATE users SET fqn =
	(CASE WHEN (SELECT local FROM instances WHERE id = instance_id) THEN
		username
	ELSE
		(username || '@' || (SELECT public_domain FROM instances WHERE id = instance_id LIMIT 1))
	END)
WHERE fqn = '';