use crate::{
    lists::List,
    posts::Post,
    schema::{posts, timeline, timeline_definition},
    Connection, Error, Result,
};
use diesel::{self, BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use std::cmp::Ordering;

pub(crate) mod query;

pub use self::query::Kind;
pub use self::query::{QueryError, TimelineQuery};

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, AsChangeset)]
#[diesel(table_name = timeline_definition)]
pub struct Timeline {
    pub id: i32,
    pub user_id: Option<i32>,
    pub name: String,
    pub query: String,
}

#[derive(Default, Insertable)]
#[diesel(table_name = timeline_definition)]
pub struct NewTimeline {
    user_id: Option<i32>,
    name: String,
    query: String,
}

#[derive(Default, Insertable)]
#[diesel(table_name = timeline)]
struct TimelineEntry {
    pub post_id: i32,
    pub timeline_id: i32,
}

impl Timeline {
    insert!(timeline_definition, NewTimeline);
    get!(timeline_definition);

    pub fn find_for_user_by_name(
        conn: &mut Connection,
        user_id: Option<i32>,
        name: &str,
    ) -> Result<Self> {
        if let Some(user_id) = user_id {
            timeline_definition::table
                .filter(timeline_definition::user_id.eq(user_id))
                .filter(timeline_definition::name.eq(name))
                .first(conn)
                .map_err(Error::from)
        } else {
            timeline_definition::table
                .filter(timeline_definition::user_id.is_null())
                .filter(timeline_definition::name.eq(name))
                .first(conn)
                .map_err(Error::from)
        }
    }

    pub fn list_for_user(conn: &mut Connection, user_id: Option<i32>) -> Result<Vec<Self>> {
        if let Some(user_id) = user_id {
            timeline_definition::table
                .filter(timeline_definition::user_id.eq(user_id))
                .load::<Self>(conn)
                .map_err(Error::from)
        } else {
            timeline_definition::table
                .filter(timeline_definition::user_id.is_null())
                .load::<Self>(conn)
                .map_err(Error::from)
        }
    }

    /// Same as `list_for_user`, but also includes instance timelines if `user_id` is `Some`.
    pub fn list_all_for_user(conn: &mut Connection, user_id: Option<i32>) -> Result<Vec<Self>> {
        if let Some(user_id) = user_id {
            timeline_definition::table
                .filter(
                    timeline_definition::user_id
                        .eq(user_id)
                        .or(timeline_definition::user_id.is_null()),
                )
                .load::<Self>(conn)
                .map_err(Error::from)
        } else {
            timeline_definition::table
                .filter(timeline_definition::user_id.is_null())
                .load::<Self>(conn)
                .map_err(Error::from)
        }
        .map(|mut timelines| {
            timelines.sort_by(|t1, t2| {
                if t1.user_id.is_some() && t2.user_id.is_none() {
                    Ordering::Less
                } else {
                    t1.id.cmp(&t2.id)
                }
            });
            timelines
        })
    }

    pub fn new_for_user(
        conn: &mut Connection,
        user_id: i32,
        name: String,
        query_string: String,
    ) -> Result<Timeline> {
        {
            let query = TimelineQuery::parse(&query_string)?; // verify the query is valid
            if let Some(err) =
                query
                    .list_used_lists()
                    .into_iter()
                    .find_map(|(name, kind)| {
                        let list = List::find_for_user_by_name(conn, Some(user_id), &name)
                            .map(|l| l.kind() == kind);
                        match list {
                            Ok(true) => None,
                            Ok(false) => Some(Error::TimelineQuery(QueryError::RuntimeError(
                                format!("list '{}' has the wrong type for this usage", name),
                            ))),
                            Err(_) => Some(Error::TimelineQuery(QueryError::RuntimeError(
                                format!("list '{}' was not found", name),
                            ))),
                        }
                    })
            {
                return Err(err);
            }
        }
        Self::insert(
            conn,
            NewTimeline {
                user_id: Some(user_id),
                name,
                query: query_string,
            },
        )
    }

    pub fn new_for_instance(
        conn: &mut Connection,
        name: String,
        query_string: String,
    ) -> Result<Timeline> {
        {
            let query = TimelineQuery::parse(&query_string)?; // verify the query is valid
            if let Some(err) =
                query
                    .list_used_lists()
                    .into_iter()
                    .find_map(|(name, kind)| {
                        let list = List::find_for_user_by_name(conn, None, &name)
                            .map(|l| l.kind() == kind);
                        match list {
                            Ok(true) => None,
                            Ok(false) => Some(Error::TimelineQuery(QueryError::RuntimeError(
                                format!("list '{}' has the wrong type for this usage", name),
                            ))),
                            Err(_) => Some(Error::TimelineQuery(QueryError::RuntimeError(
                                format!("list '{}' was not found", name),
                            ))),
                        }
                    })
            {
                return Err(err);
            }
        }
        Self::insert(
            conn,
            NewTimeline {
                user_id: None,
                name,
                query: query_string,
            },
        )
    }

    pub fn update(&self, conn: &mut Connection) -> Result<Self> {
        diesel::update(self).set(self).execute(conn)?;
        let timeline = Self::get(conn, self.id)?;
        Ok(timeline)
    }

    pub fn delete(&self, conn: &mut Connection) -> Result<()> {
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn get_latest(&self, conn: &mut Connection, count: i32) -> Result<Vec<Post>> {
        self.get_page(conn, (0, count))
    }

    pub fn get_page(&self, conn: &mut Connection, (min, max): (i32, i32)) -> Result<Vec<Post>> {
        timeline::table
            .filter(timeline::timeline_id.eq(self.id))
            .inner_join(posts::table)
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .select(posts::all_columns)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn count_posts(&self, conn: &mut Connection) -> Result<i64> {
        timeline::table
            .filter(timeline::timeline_id.eq(self.id))
            .inner_join(posts::table)
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub async fn add_to_all_timelines(conn: &mut Connection, post: &Post, kind: &Kind) -> Result<()> {
        let timelines = timeline_definition::table
            .load::<Self>(conn)
            .map_err(Error::from)?;

        for t in timelines {
            if t.matches(conn, post, kind).await? {
                t.add_post(conn, post)?;
            }
        }
        Ok(())
    }

    pub fn add_post(&self, conn: &mut Connection, post: &Post) -> Result<()> {
        if self.includes_post(conn, post)? {
            return Ok(());
        }
        diesel::insert_into(timeline::table)
            .values(TimelineEntry {
                post_id: post.id,
                timeline_id: self.id,
            })
            .execute(conn)?;
        Ok(())
    }

    pub fn remove_post(&self, conn: &mut Connection, post: &Post) -> Result<bool> {
        if self.includes_post(conn, post)? {
            return Ok(false);
        }
        diesel::delete(
            timeline::table
                .filter(timeline::timeline_id.eq(self.id))
                .filter(timeline::post_id.eq(post.id)),
        )
        .execute(conn)?;
        Ok(true)
    }

    pub fn remove_all_posts(&self, conn: &mut Connection) -> Result<u64> {
        let count = diesel::delete(timeline::table.filter(timeline::timeline_id.eq(self.id)))
            .execute(conn)?;
        Ok(count as u64)
    }

    pub async fn matches(&self, conn: &mut Connection, post: &Post, kind: &Kind) -> Result<bool> {
        let query = TimelineQuery::parse(&self.query)?;
        query.matches(conn, self, post, kind).await
    }

    fn includes_post(&self, conn: &mut Connection, post: &Post) -> Result<bool> {
        diesel::dsl::select(diesel::dsl::exists(
            timeline::table
                .filter(timeline::timeline_id.eq(self.id))
                .filter(timeline::post_id.eq(post.id)),
        ))
        .get_result(conn)
        .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        blogs::tests as blogTests,
        follows::*,
        lists::ListType,
        post_authors::{NewPostAuthor, PostAuthor},
        posts::NewPost,
        safe_string::SafeString,
        tags::Tag,
        tests::db,
        users::tests as userTests,
    };
    use diesel::Connection;

    #[test]
    fn test_timeline() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let users = userTests::fill_database(conn);

            let mut tl1_u1 = Timeline::new_for_user(
                conn,
                users[0].id,
                "my timeline".to_owned(),
                "all".to_owned(),
            )
            .unwrap();
            List::new(conn, "languages I speak", Some(&users[1]), ListType::Prefix).unwrap();
            let tl2_u1 = Timeline::new_for_user(
                conn,
                users[0].id,
                "another timeline".to_owned(),
                "followed".to_owned(),
            )
            .unwrap();
            let tl1_u2 = Timeline::new_for_user(
                conn,
                users[1].id,
                "english posts".to_owned(),
                "lang in \"languages I speak\"".to_owned(),
            )
            .unwrap();
            let tl1_instance = Timeline::new_for_instance(
                conn,
                "english posts".to_owned(),
                "license in [cc]".to_owned(),
            )
            .unwrap();

            assert_eq!(tl1_u1, Timeline::get(conn, tl1_u1.id).unwrap());
            assert_eq!(
                tl2_u1,
                Timeline::find_for_user_by_name(conn, Some(users[0].id), "another timeline")
                    .unwrap()
            );
            assert_eq!(
                tl1_instance,
                Timeline::find_for_user_by_name(conn, None, "english posts").unwrap()
            );

            let tl_u1 = Timeline::list_for_user(conn, Some(users[0].id)).unwrap();
            assert_eq!(3, tl_u1.len()); // it is not 2 because there is a "Your feed" tl created for each user automatically
            assert!(tl_u1.iter().any(|tl| *tl == tl1_u1));
            assert!(tl_u1.iter().any(|tl| *tl == tl2_u1));

            let tl_instance = Timeline::list_for_user(conn, None).unwrap();
            assert_eq!(3, tl_instance.len()); // there are also the local and federated feed by default
            assert!(tl_instance.iter().any(|tl| *tl == tl1_instance));

            tl1_u1.name = "My Super TL".to_owned();
            let new_tl1_u2 = tl1_u2.update(conn).unwrap();

            let tl_u2 = Timeline::list_for_user(conn, Some(users[1].id)).unwrap();
            assert_eq!(2, tl_u2.len()); // same here
            assert!(tl_u2.iter().any(|tl| *tl == new_tl1_u2));

            Ok(())
        });
    }

    #[test]
    fn test_timeline_creation_error() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let users = userTests::fill_database(conn);

            assert!(Timeline::new_for_user(
                conn,
                users[0].id,
                "my timeline".to_owned(),
                "invalid keyword".to_owned(),
            )
            .is_err());
            assert!(Timeline::new_for_instance(
                conn,
                "my timeline".to_owned(),
                "invalid keyword".to_owned(),
            )
            .is_err());

            assert!(Timeline::new_for_user(
                conn,
                users[0].id,
                "my timeline".to_owned(),
                "author in non_existant_list".to_owned(),
            )
            .is_err());
            assert!(Timeline::new_for_instance(
                conn,
                "my timeline".to_owned(),
                "lang in dont-exist".to_owned(),
            )
            .is_err());

            List::new(conn, "friends", Some(&users[0]), ListType::User).unwrap();
            List::new(conn, "idk", None, ListType::Blog).unwrap();

            assert!(Timeline::new_for_user(
                conn,
                users[0].id,
                "my timeline".to_owned(),
                "blog in friends".to_owned(),
            )
            .is_err());
            assert!(Timeline::new_for_instance(
                conn,
                "my timeline".to_owned(),
                "not author in idk".to_owned(),
            )
            .is_err());

            Ok(())
        });
    }

    #[test]
    fn test_simple_match() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blogTests::fill_database(conn);

            let gnu_tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "GNU timeline".to_owned(),
                "license in [AGPL, LGPL, GPL]".to_owned(),
            )
            .unwrap();

            let gnu_post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug".to_string(),
                    title: "About Linux".to_string(),
                    content: SafeString::new("you must say GNU/Linux, not Linux!!!"),
                    published: true,
                    license: "GPL".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    source: "you must say GNU/Linux, not Linux!!!".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();
            assert!(gnu_tl.matches(conn, &gnu_post, Kind::Original).unwrap());

            let non_free_post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug2".to_string(),
                    title: "Private is bad".to_string(),
                    content: SafeString::new("so is Microsoft"),
                    published: true,
                    license: "all right reserved".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    source: "so is Microsoft".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();
            assert!(!gnu_tl
                .matches(conn, &non_free_post, Kind::Original)
                .unwrap());

            Ok(())
        });
    }

    #[test]
    fn test_complex_match() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blogTests::fill_database(conn);
            Follow::insert(
                conn,
                NewFollow {
                    follower_id: users[0].id,
                    following_id: users[1].id,
                    ap_url: String::new(),
                },
            )
            .unwrap();

            let fav_blogs_list =
                List::new(conn, "fav_blogs", Some(&users[0]), ListType::Blog).unwrap();
            fav_blogs_list.add_blogs(conn, &[blogs[0].id]).unwrap();

            let my_tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "My timeline".to_owned(),
                "blog in fav_blogs and not has_cover or local and followed exclude likes"
                    .to_owned(),
            )
            .unwrap();

            let post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "about-linux".to_string(),
                    title: "About Linux".to_string(),
                    content: SafeString::new("you must say GNU/Linux, not Linux!!!"),
                    published: true,
                    license: "GPL".to_string(),
                    source: "you must say GNU/Linux, not Linux!!!".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();
            assert!(my_tl.matches(conn, &post, Kind::Original).unwrap()); // matches because of "blog in fav_blogs" (and there is no cover)

            let post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[1].id,
                    slug: "about-linux-2".to_string(),
                    title: "About Linux (2)".to_string(),
                    content: SafeString::new(
                        "Actually, GNU+Linux, GNU×Linux, or GNU¿Linux are better.",
                    ),
                    published: true,
                    license: "GPL".to_string(),
                    source: "Actually, GNU+Linux, GNU×Linux, or GNU¿Linux are better.".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();
            assert!(!my_tl.matches(conn, &post, Kind::Like(&users[1])).unwrap());

            Ok(())
        });
    }

    #[test]
    fn test_add_to_all_timelines() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blogTests::fill_database(conn);

            let gnu_tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "GNU timeline".to_owned(),
                "license in [AGPL, LGPL, GPL]".to_owned(),
            )
            .unwrap();
            let non_gnu_tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Stallman disapproved timeline".to_owned(),
                "not license in [AGPL, LGPL, GPL]".to_owned(),
            )
            .unwrap();

            let gnu_post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug".to_string(),
                    title: "About Linux".to_string(),
                    content: SafeString::new("you must say GNU/Linux, not Linux!!!"),
                    published: true,
                    license: "GPL".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    source: "you must say GNU/Linux, not Linux!!!".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();

            let non_free_post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug2".to_string(),
                    title: "Private is bad".to_string(),
                    content: SafeString::new("so is Microsoft"),
                    published: true,
                    license: "all right reserved".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    source: "so is Microsoft".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();

            Timeline::add_to_all_timelines(conn, &gnu_post, Kind::Original).unwrap();
            Timeline::add_to_all_timelines(conn, &non_free_post, Kind::Original).unwrap();

            let res = gnu_tl.get_latest(conn, 2).unwrap();
            assert_eq!(res.len(), 1);
            assert_eq!(res[0].id, gnu_post.id);
            let res = non_gnu_tl.get_latest(conn, 2).unwrap();
            assert_eq!(res.len(), 1);
            assert_eq!(res[0].id, non_free_post.id);

            Ok(())
        });
    }

    #[test]
    fn test_matches_lists_direct() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blogTests::fill_database(conn);

            let gnu_post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug".to_string(),
                    title: "About Linux".to_string(),
                    content: SafeString::new("you must say GNU/Linux, not Linux!!!"),
                    published: true,
                    license: "GPL".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    source: "you must say GNU/Linux, not Linux!!!".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();
            gnu_post
                .update_tags(conn, vec![Tag::build_activity("free".to_owned()).unwrap()])
                .unwrap();
            PostAuthor::insert(
                conn,
                NewPostAuthor {
                    post_id: gnu_post.id,
                    author_id: blogs[0].list_authors(conn).unwrap()[0].id,
                },
            )
            .unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "blog timeline".to_owned(),
                format!("blog in [{}]", blogs[0].fqn),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "blog timeline".to_owned(),
                "blog in [no_one@nowhere]".to_owned(),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "author timeline".to_owned(),
                format!(
                    "author in [{}]",
                    blogs[0].list_authors(conn).unwrap()[0].fqn
                ),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "author timeline".to_owned(),
                format!("author in [{}]", users[2].fqn),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            assert!(tl
                .matches(conn, &gnu_post, Kind::Reshare(&users[2]))
                .unwrap());
            assert!(!tl.matches(conn, &gnu_post, Kind::Like(&users[2])).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "author timeline".to_owned(),
                format!(
                    "author in [{}] include likes exclude reshares",
                    users[2].fqn
                ),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            assert!(!tl
                .matches(conn, &gnu_post, Kind::Reshare(&users[2]))
                .unwrap());
            assert!(tl.matches(conn, &gnu_post, Kind::Like(&users[2])).unwrap());
            tl.delete(conn).unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "tag timeline".to_owned(),
                "tags in [free]".to_owned(),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "tag timeline".to_owned(),
                "tags in [private]".to_owned(),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "english timeline".to_owned(),
                "lang in [en]".to_owned(),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "franco-italian timeline".to_owned(),
                "lang in [fr, it]".to_owned(),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();

            Ok(())
        });
    }

    /*
    #[test]
    fn test_matches_lists_saved() {
        let r = &rockets();
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blogTests::fill_database(&conn);

            let gnu_post = Post::insert(
                &conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug".to_string(),
                    title: "About Linux".to_string(),
                    content: SafeString::new("you must say GNU/Linux, not Linux!!!"),
                    published: true,
                    license: "GPL".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "".to_string(),
                    source: "you must say GNU/Linux, not Linux!!!".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();
            gnu_post.update_tags(&conn, vec![Tag::build_activity("free".to_owned()).unwrap()]).unwrap();
            PostAuthor::insert(&conn, NewPostAuthor {post_id: gnu_post.id, author_id: blogs[0].list_authors(&conn).unwrap()[0].id}).unwrap();

            unimplemented!();

            Ok(())
        });
    }*/

    #[test]
    fn test_matches_keyword() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blogTests::fill_database(conn);

            let gnu_post = Post::insert(
                conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "slug".to_string(),
                    title: "About Linux".to_string(),
                    content: SafeString::new("you must say GNU/Linux, not Linux!!!"),
                    published: true,
                    license: "GPL".to_string(),
                    ap_url: "".to_string(),
                    creation_date: None,
                    subtitle: "Stallman is our god".to_string(),
                    source: "you must say GNU/Linux, not Linux!!!".to_string(),
                    cover_id: None,
                },
            )
            .unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Linux title".to_owned(),
                "title contains Linux".to_owned(),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Microsoft title".to_owned(),
                "title contains Microsoft".to_owned(),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Linux subtitle".to_owned(),
                "subtitle contains Stallman".to_owned(),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Microsoft subtitle".to_owned(),
                "subtitle contains Nadella".to_owned(),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();

            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Linux content".to_owned(),
                "content contains Linux".to_owned(),
            )
            .unwrap();
            assert!(tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();
            let tl = Timeline::new_for_user(
                conn,
                users[0].id,
                "Microsoft content".to_owned(),
                "subtitle contains Windows".to_owned(),
            )
            .unwrap();
            assert!(!tl.matches(conn, &gnu_post, Kind::Original).unwrap());
            tl.delete(conn).unwrap();

            Ok(())
        });
    }
}
