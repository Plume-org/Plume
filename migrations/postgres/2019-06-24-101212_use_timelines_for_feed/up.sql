-- Your SQL goes here
--#!|conn: &Connection, path: &Path| {
--#!	super::timeline::Timeline::new_for_instance(conn, "Local feed".into(), "local".into()).expect("Local feed creation error");
--#!	super::timeline::Timeline::new_for_instance(conn, "Federated feed".into(), "all".into()).expect("Federated feed creation error");
--#!
--#!	for i in 0.. {
--#!		if let Some(users) = super::users::User::get_local_page(conn, (i * 20, (i + 1) * 20)).ok().filter(|l| !l.is_empty()) {
--#!			for u in users {
--#!				super::timeline::Timeline::new_for_user(conn, u.id, "Your feed".into(), format!("followed or author in [ {} ]", u.fqn)).expect("User feed creation error");
--#!			}
--#!		} else {
--#!			break;
--#!		}
--#!	}
--#!
--#!	Ok(())
--#!}

INSERT INTO timeline (post_id, timeline_id) SELECT posts.id, (SELECT id FROM timeline_definition WHERE query = 'local' LIMIT 1) FROM blogs INNER JOIN posts ON posts.blog_id = blogs.id WHERE blogs.instance_id = 1;
INSERT INTO timeline (post_id, timeline_id) SELECT posts.id, (SELECT id FROM timeline_definition WHERE query = 'all' LIMIT 1) FROM posts;
