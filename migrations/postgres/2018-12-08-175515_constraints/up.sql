-- Your SQL goes here

-- First, we delete the already duplicated data so that the constraint can be correctly applied

DELETE FROM api_tokens a USING api_tokens b WHERE
	a.id > b.id
	AND a.value = b.value;
DELETE FROM blog_authors a USING blog_authors b WHERE
	a.id > b.id
	AND a.blog_id = b.blog_id
	AND a.author_id = b.author_id;
DELETE FROM blogs a USING blogs b WHERE
	a.id > b.id
	AND ((a.actor_id = b.actor_id AND a.instance_id = b.instance_id)
	OR a.ap_url = b.ap_url
	OR a.outbox_url = b.outbox_url
	OR a.inbox_url = b.inbox_url);
DELETE FROM comments a USING comments b WHERE
	a.id > b.id
	AND (a.ap_url = b.ap_url);
DELETE FROM follows a USING follows b
	WHERE a.id > b.id
	AND (a.ap_url = b.ap_url);
DELETE FROM instances a USING instances b WHERE
	a.id > b.id
	AND (a.public_domain = b.public_domain);
DELETE FROM likes a USING likes b WHERE
	a.id > b.id
	AND (a.ap_url = b.ap_url
	OR (a.user_id = b.user_id AND a.post_id = b.post_id));
DELETE FROM mentions a USING mentions b WHERE
	a.id > b.id
	AND (a.ap_url = b.ap_url);
DELETE FROM post_authors a USING post_authors b WHERE
	a.id > b.id
	AND a.author_id = b.author_id
	AND a.post_id = b.post_id;
DELETE FROM posts a USING posts b WHERE
	a.id > b.id
	AND ((a.ap_url = b.ap_url)
	OR (a.blog_id = b.blog_id AND a.slug = b.slug));
DELETE FROM reshares a USING reshares b WHERE
	a.id > b.id
	AND (a.ap_url = b.ap_url
	OR (a.user_id = b.user_id AND a.post_id = b.post_id));
DELETE FROM tags a USING tags b WHERE
	a.id > b.id
	AND a.tag = b.tag
	AND a.post_id = b.post_id
	AND a.is_hashtag = b.is_hashtag;
DELETE FROM users a USING users b WHERE
	a.id > b.id
	AND (a.ap_url = b.ap_url
	OR (a.username = b.username AND a.instance_id = b.instance_id)
	OR a.outbox_url = b.outbox_url
	OR a.inbox_url = b.inbox_url
	OR a.followers_endpoint = b.followers_endpoint);

-- Then we add the UNIQUE constraints

ALTER TABLE api_tokens ADD CONSTRAINT api_tokens_unique_value UNIQUE (value);
ALTER TABLE blog_authors ADD CONSTRAINT blog_author_unique UNIQUE (blog_id, author_id);
ALTER TABLE blogs ADD CONSTRAINT blog_unique UNIQUE (actor_id, instance_id);
ALTER TABLE blogs ADD CONSTRAINT blog_unique_ap_url UNIQUE (ap_url);
ALTER TABLE blogs ADD CONSTRAINT blog_unique_outbox UNIQUE (outbox_url);
ALTER TABLE blogs ADD CONSTRAINT blog_unique_inbox UNIQUE (inbox_url);
ALTER TABLE comments ADD CONSTRAINT comments_unique_ap_url UNIQUE (ap_url);
ALTER TABLE follows ADD CONSTRAINT follows_unique_ap_url UNIQUE (ap_url);
ALTER TABLE instances ADD CONSTRAINT instance_unique_domain UNIQUE (public_domain);
ALTER TABLE likes ADD CONSTRAINT likes_unique UNIQUE (user_id, post_id);
ALTER TABLE likes ADD CONSTRAINT likes_unique_ap_url UNIQUE (ap_url);
ALTER TABLE mentions ADD CONSTRAINT mentions_unique_ap_url UNIQUE (ap_url);
ALTER TABLE post_authors ADD CONSTRAINT post_authors_unique UNIQUE (post_id, author_id);
ALTER TABLE posts ADD CONSTRAINT post_unique_slug UNIQUE (blog_id, slug);
ALTER TABLE posts ADD CONSTRAINT post_unique_ap_url UNIQUE (ap_url);
ALTER TABLE reshares ADD CONSTRAINT reshares_unique UNIQUE (user_id, post_id);
ALTER TABLE reshares ADD CONSTRAINT reshares_unique_ap_url UNIQUE (ap_url);
ALTER TABLE tags ADD CONSTRAINT tags_unique UNIQUE (tag, post_id, is_hashtag);
ALTER TABLE users ADD CONSTRAINT users_unique UNIQUE (username, instance_id);
ALTER TABLE users ADD CONSTRAINT users_unique_inbox UNIQUE (inbox_url);
ALTER TABLE users ADD CONSTRAINT users_unique_outbox UNIQUE (outbox_url);
ALTER TABLE users ADD CONSTRAINT users_unique_ap_url UNIQUE (ap_url);
ALTER TABLE users ADD CONSTRAINT users_unique_followers_url UNIQUE (followers_endpoint);
