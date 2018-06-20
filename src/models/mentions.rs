use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use models::{
    comments::Comment,
    posts::Post
};
use schema::mentions;

#[derive(Queryable, Identifiable)]
pub struct Mention {
    pub id: i32,
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>
}

#[derive(Insertable)]
#[table_name = "mentions"]
pub struct NewMention {
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>
}

impl Mention {
    insert!(mentions, NewMention);
    get!(mentions);
    list_by!(mentions, list_for_user, mentioned_id as i32);

    pub fn get_post(&self, conn: &PgConnection) -> Option<Post> {
        self.post_id.and_then(|id| Post::get(conn, id))
    }

    pub fn get_comment(&self, conn: &PgConnection) -> Option<Comment> {
        self.post_id.and_then(|id| Comment::get(conn, id))
    }
}
