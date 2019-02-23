-- Adding custom domain to Blog as an optional field
CREATE TABLE tmp_blogs_add_custom_domains (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  actor_id VARCHAR NOT NULL,
  title VARCHAR NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  outbox_url VARCHAR NOT NULL UNIQUE,
  inbox_url VARCHAR NOT NULL UNIQUE,
  instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL,
  creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  ap_url text not null default '' UNIQUE,
  custom_domain VARCHAR DEFAULT NULL UNIQUE,
  private_key TEXT,
  public_key TEXT NOT NULL DEFAULT '',
  CONSTRAINT blog_unique UNIQUE (actor_id, instance_id)
);

INSERT INTO tmp_blogs_add_custom_domains (
  id,
  actor_id,
  title,
  summary,
  outbox_url,
  inbox_url,
  instance_id,
  creation_date,
  ap_url,
  private_key,
  public_key
) SELECT
  id,
  actor_id,
  title,
  summary,
  outbox_url,
  inbox_url,
  instance_id,
  creation_date,
  ap_url,
  private_key,
  public_key
FROM blogs;

DROP TABLE blogs;
ALTER TABLE tmp_blogs_add_custom_domains RENAME TO blogs;
