INSERT INTO timeline (post_id, timeline_id)
	SELECT posts.id,timeline_definition.id FROM posts,timeline_definition
	WHERE timeline_definition.query = 'all';

INSERT INTO timeline (post_id, timeline_id)
	SELECT posts.id,timeline_definition.id FROM posts
	CROSS JOIN timeline_definition
	INNER JOIN blogs ON posts.blog_id = blogs.id
	INNER JOIN instances ON blogs.instance_id = instances.id
	WHERE timeline_definition.query = 'local' and instances.local = true;

INSERT INTO timeline (post_id, timeline_id)
	SELECT posts.id,timeline_definition.id FROM posts
	INNER JOIN blog_authors ON posts.blog_id = blog_authors.blog_id
	LEFT JOIN follows ON blog_authors.author_id = follows.following_id
	INNER JOIN timeline_definition ON follows.follower_id = timeline_definition.user_id
		or blog_authors.author_id = timeline_definition.user_id
	WHERE timeline_definition.query LIKE 'followed or [%]';
