use chrono;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use schema::likes;

#[derive(Queryable)]
pub struct Like {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub creation_date: chrono::NaiveDateTime
}

#[derive(Insertable)]
#[table_name = "likes"]
pub struct NewLike {
    pub user_id: i32,
    pub post_id: i32
}

impl Like {
    pub fn insert(conn: &PgConnection, new: NewLike) -> Like {
        diesel::insert_into(likes::table)
            .values(new)
            .get_result(conn)
            .expect("Unable to insert new like")
    }

     pub fn get(conn: &PgConnection, id: i32) -> Option<Like> {
        likes::table.filter(likes::id.eq(id))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like by ID")
            .into_iter().nth(0)
    }
}
