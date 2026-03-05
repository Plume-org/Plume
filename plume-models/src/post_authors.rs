use crate::{posts::Post, schema::post_authors, users::User, Error, Result};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

#[derive(Clone, Queryable, Identifiable, Associations)]
#[diesel(belongs_to(Post))]
#[diesel(belongs_to(User, foreign_key = author_id))]
pub struct PostAuthor {
    pub id: i32,
    pub post_id: i32,
    pub author_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = post_authors)]
pub struct NewPostAuthor {
    pub post_id: i32,
    pub author_id: i32,
}

impl PostAuthor {
    insert!(post_authors, NewPostAuthor);
    get!(post_authors);
}
