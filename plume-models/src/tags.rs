use diesel::{self, PgConnection, ExpressionMethods, RunQueryDsl, QueryDsl};
use schema::tags;

#[derive(Queryable)]
pub struct Tag {
    pub id: i32,
    pub tag: String,
    pub is_hastag: bool,
    pub post_id: i32
}

#[derive(Insertable)]
#[table_name = "tags"]
pub struct NewTag {
    pub tag: String,
    pub is_hastag: bool,
    pub post_id: i32
}

impl Tag {
    insert!(tags, NewTag);
    get!(tags);
}
