use chrono;
use diesel::{PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use schema::likes;

#[derive(Queryable)]
pub struct Like {
    id: i32,
    user_id: i32,
    post_id: i32,
    creation_date: chrono::NaiveDateTime
}

#[derive(Insertable)]
#[table_name = "likes"]
pub struct NewLike {
    user_id: i32,
    post_id: i32
}

impl Like {
     pub fn get(conn: &PgConnection, id: i32) -> Option<Like> {
        likes::table.filter(likes::id.eq(id))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like by ID")
            .into_iter().nth(0)
    }
}
