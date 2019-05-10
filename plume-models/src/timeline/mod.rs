use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use lists::List;
use posts::Post;
use schema::{posts, timeline, timeline_definition};
use {Connection, Error, Result};

pub(crate) mod query;

use self::query::{Kind, QueryError, TimelineQuery};

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, AsChangeset)]
#[table_name = "timeline_definition"]
pub struct Timeline {
    pub id: i32,
    pub user_id: Option<i32>,
    pub name: String,
    pub query: String,
}

#[derive(Default, Insertable)]
#[table_name = "timeline_definition"]
pub struct NewTimeline {
    user_id: Option<i32>,
    name: String,
    query: String,
}

#[derive(Default, Insertable)]
#[table_name = "timeline"]
struct TimelineEntry {
    pub post_id: i32,
    pub timeline_id: i32,
}

impl Timeline {
    insert!(timeline_definition, NewTimeline);
    get!(timeline_definition);

    pub fn find_by_name(conn: &Connection, user_id: Option<i32>, name: &str) -> Result<Self> {
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

    pub fn list_for_user(conn: &Connection, user_id: Option<i32>) -> Result<Vec<Self>> {
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

    pub fn new_for_user(
        conn: &Connection,
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
                        let list = List::find_by_name(conn, Some(user_id), &name)
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
                Err(err)?;
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
        conn: &Connection,
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
                        let list = List::find_by_name(conn, None, &name).map(|l| l.kind() == kind);
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
                Err(err)?;
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

    pub fn update(&self, conn: &Connection) -> Result<Self> {
        diesel::update(self).set(self).execute(conn)?;
        let timeline = Self::get(conn, self.id)?;
        Ok(timeline)
    }

    pub fn get_latest(&self, conn: &Connection, count: i32) -> Result<Vec<Post>> {
        self.get_page(conn, (0, count))
    }

    pub fn get_page(&self, conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<Post>> {
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

    pub fn add_to_all_timelines(conn: &Connection, post: &Post, kind: Kind) -> Result<()> {
        let timelines = timeline_definition::table
            .load::<Self>(conn)
            .map_err(Error::from)?;

        for t in timelines {
            if t.matches(conn, post, kind)? {
                t.add_post(conn, post)?;
            }
        }
        Ok(())
    }

    pub fn add_post(&self, conn: &Connection, post: &Post) -> Result<()> {
        diesel::insert_into(timeline::table)
            .values(TimelineEntry {
                post_id: post.id,
                timeline_id: self.id,
            })
            .execute(conn)?;
        Ok(())
    }

    pub fn matches(&self, conn: &Connection, post: &Post, kind: Kind) -> Result<bool> {
        let query = TimelineQuery::parse(&self.query)?;
        query.matches(conn, self, post, kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::Connection;
    use lists::ListType;
    use tests::db;
    use users::tests as userTests;

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
                Timeline::find_by_name(conn, Some(users[0].id), "another timeline").unwrap()
            );
            assert_eq!(
                tl1_instance,
                Timeline::find_by_name(conn, None, "english posts").unwrap()
            );

            let tl_u1 = Timeline::list_for_user(conn, Some(users[0].id)).unwrap();
            assert_eq!(2, tl_u1.len());
            if tl1_u1.id == tl_u1[0].id {
                assert_eq!(tl1_u1, tl_u1[0]);
                assert_eq!(tl2_u1, tl_u1[1]);
            } else {
                assert_eq!(tl2_u1, tl_u1[0]);
                assert_eq!(tl1_u1, tl_u1[1]);
            }

            let tl_instance = Timeline::list_for_user(conn, None).unwrap();
            assert_eq!(1, tl_instance.len());
            assert_eq!(tl1_instance, tl_instance[0]);

            tl1_u1.name = "My Super TL".to_owned();
            let new_tl1_u2 = tl1_u2.update(conn).unwrap();

            let tl_u2 = Timeline::list_for_user(conn, Some(users[1].id)).unwrap();
            assert_eq!(1, tl_u2.len());
            assert_eq!(new_tl1_u2, tl_u2[0]);

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
}
