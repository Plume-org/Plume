use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use comments::Comment;
use schema::comment_seers;
use users::User;
use {Connection, Error, Result};

#[derive(Queryable, Clone)]
pub struct CommentSeers {
    pub id: i32,
    pub comment_id: i32,
    pub user_id: i32,
}

#[derive(Insertable, Default)]
#[table_name = "comment_seers"]
pub struct NewCommentSeers {
    pub comment_id: i32,
    pub user_id: i32,
}

impl CommentSeers {
    insert!(comment_seers, NewCommentSeers);

    pub fn can_see(conn: &Connection, c: &Comment, u: &User) -> Result<bool> {
        comment_seers::table.filter(comment_seers::comment_id.eq(c.id))
            .filter(comment_seers::user_id.eq(u.id))
            .load::<CommentSeers>(conn)
            .map_err(Error::from)
            .map(|r| !r.is_empty())
    }
}
