use crate::{schema::blog_authors, Error, Result};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

#[derive(Clone, Queryable, Identifiable)]
pub struct BlogAuthor {
    pub id: i32,
    pub blog_id: i32,
    pub author_id: i32,
    pub is_owner: bool,
}

#[derive(Insertable)]
#[diesel(table_name = blog_authors)]
pub struct NewBlogAuthor {
    pub blog_id: i32,
    pub author_id: i32,
    pub is_owner: bool,
}

impl BlogAuthor {
    insert!(blog_authors, NewBlogAuthor);
    get!(blog_authors);
}
