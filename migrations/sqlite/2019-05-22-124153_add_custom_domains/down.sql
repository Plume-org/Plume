-- undo the adding of "custom_domain" to blogs
CREATE TABLE IF NOT EXISTS "blogs_drop_custom_domain" (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  actor_id VARCHAR NOT NULL,
  title VARCHAR NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  outbox_url VARCHAR NOT NULL UNIQUE,
  inbox_url VARCHAR NOT NULL UNIQUE,
  instance_id INTEGER REFERENCES instances(id) ON DELETE CASCADE NOT NULL,
  creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  ap_url text not null default '' UNIQUE,
  private_key TEXT,
  public_key TEXT NOT NULL DEFAULT '',
  fqn TEXT NOT NULL DEFAULT '',
  summary_html TEXT NOT NULL DEFAULT '',
  icon_id INTEGER REFERENCES medias(id) ON DELETE SET NULL DEFAULT NULL,
  banner_id INTEGER REFERENCES medias(id) ON DELETE SET NULL DEFAULT NULL,
  CONSTRAINT blog_unique UNIQUE (actor_id, instance_id)
);

INSERT INTO blogs_drop_custom_domain (
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
  public_key,
  fqn,
  summary_html,
  icon_id,
  banner_id
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
  public_key,
  fqn,
  summary_html,
  icon_id,
  banner_id
FROM blogs;

DROP TABLE blogs;
ALTER TABLE "blogs_drop_custom_domain" RENAME to blogs;
