use activitypub::link;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use activity_pub::Id;
use models::{
    comments::Comment,
    posts::Post,
    users::User
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

    pub fn get_mentioned(&self, conn: &PgConnection) -> Option<User> {
        User::get(conn, self.mentioned_id)
    }

    pub fn get_post(&self, conn: &PgConnection) -> Option<Post> {
        self.post_id.and_then(|id| Post::get(conn, id))
    }

    pub fn get_comment(&self, conn: &PgConnection) -> Option<Comment> {
        self.post_id.and_then(|id| Comment::get(conn, id))
    }

    pub fn to_activity(&self, conn: &PgConnection) -> link::Mention {
        let user = self.get_mentioned(conn);
        let mut mention = link::Mention::default();
        mention.link_props.set_href_string(user.clone().map(|u| u.ap_url).unwrap_or(String::new())).expect("Error setting mention's href");
        mention.link_props.set_name_string(user.map(|u| format!("@{}", u.get_fqn(conn))).unwrap_or(String::new())).expect("Error setting mention's name");
        mention
    }

    pub fn from_activity(conn: &PgConnection, ment: link::Mention, inside: Id) -> Option<Self> {
        let mentioned = User::find_by_ap_url(conn, ment.link_props.href_string().unwrap()).unwrap();

        if let Some(post) = Post::find_by_ap_url(conn, inside.clone().into()) {
            Some(Mention::insert(conn, NewMention {
                mentioned_id: mentioned.id,
                post_id: Some(post.id),
                comment_id: None
            }))
        } else {
            if let Some(comment) = Comment::find_by_ap_url(conn, inside.into()) {
                Some(Mention::insert(conn, NewMention {
                    mentioned_id: mentioned.id,
                    post_id: None,
                    comment_id: Some(comment.id)
                }))
            } else {
                None
            }
        }
    }
}
