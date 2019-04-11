use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use posts::Post;
use schema::{posts, timeline, timeline_definition};
use {Connection, Error, Result};

mod query;

use self::query::TimelineQuery;

#[derive(Clone, Queryable, Identifiable)]
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
    find_by!(timeline_definition, find_by_name_and_user,
             user_id as Option<i32> , name as &str);
    list_by!(timeline_definition, list_for_user, user_id as Option<i32>);

    pub fn new_for_user(conn: &Connection, user_id: i32, name: String, query: String) -> Result<Timeline> {
        TimelineQuery::parse(&query)?;// verify the query is valid
        Self::insert(conn, NewTimeline {
            user_id: Some(user_id),
            name,
            query,
        })
    }

    pub fn new_for_instance(conn: &Connection, name: String, query: String) -> Result<Timeline> {
        TimelineQuery::parse(&query)?;// verify the query is valid
        Self::insert(conn, NewTimeline {
            user_id: None,
            name,
            query,
        })
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

    pub fn add_to_all_timelines(conn: &Connection, post: &Post) -> Result<()> {
        let timelines = timeline_definition::table
            .load::<Self>(conn)
            .map_err(Error::from)?;

        for t in timelines {
            if t.matches(conn, post)? {
                t.add_post(conn, post)?;
            }
        }
        Ok(())
    }

    pub fn add_post(&self, conn: &Connection, post: &Post) -> Result<()> {
        diesel::insert_into(timeline::table)
            .values(TimelineEntry{
                post_id: post.id,
                timeline_id: self.id,
            })
            .execute(conn)?;
        Ok(())
    }

    pub fn matches(&self, conn: &Connection, post: &Post) -> Result<bool> {
        let query = TimelineQuery::parse(&self.query)?;
        query.matches(conn, self, post)
    }
}
