use chrono;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods};

use schema::comments;

#[derive(Queryable, Identifiable, Serialize)]
pub struct Comment {
    pub id: i32,
    pub content: String,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub creation_date: chrono::NaiveDateTime,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String
}

#[derive(Insertable)]
#[table_name = "comments"]
pub struct NewComment {
    pub content: String,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String
}

impl Comment {
    pub fn insert (conn: &PgConnection, new: NewComment) -> Comment {
        diesel::insert_into(comments::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new comment")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Comment> {
        comments::table.filter(comments::id.eq(id))
            .limit(1)
            .load::<Comment>(conn)
            .expect("Error loading comment by id")
            .into_iter().nth(0)
    }
}
