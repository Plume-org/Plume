use activitypub::link;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use plume_common::activity_pub::inbox::Notify;
use comments::Comment;
use notifications::*;
use posts::Post;
use users::User;
use schema::mentions;

#[derive(Queryable, Identifiable, Serialize, Deserialize)]
pub struct Mention {
    pub id: i32,
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,
    pub ap_url: String // TODO: remove, since mentions don't have an AP URL actually, this field was added by mistake
}

#[derive(Insertable)]
#[table_name = "mentions"]
pub struct NewMention {
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,
    pub ap_url: String
}

impl Mention {
    insert!(mentions, NewMention);
    get!(mentions);
    find_by!(mentions, find_by_ap_url, ap_url as String);
    list_by!(mentions, list_for_user, mentioned_id as i32);
    list_by!(mentions, list_for_post, post_id as i32);
    list_by!(mentions, list_for_comment, comment_id as i32);

    pub fn get_mentioned(&self, conn: &PgConnection) -> Option<User> {
        User::get(conn, self.mentioned_id)
    }

    pub fn get_post(&self, conn: &PgConnection) -> Option<Post> {
        self.post_id.and_then(|id| Post::get(conn, id))
    }

    pub fn get_comment(&self, conn: &PgConnection) -> Option<Comment> {
        self.comment_id.and_then(|id| Comment::get(conn, id))
    }

    pub fn get_user(&self, conn: &PgConnection) -> Option<User> {
        match self.get_post(conn) {
            Some(p) => p.get_authors(conn).into_iter().next(),
            None => self.get_comment(conn).map(|c| c.get_author(conn))
        }
    }

    pub fn build_activity(conn: &PgConnection, ment: String) -> link::Mention {
        let user = User::find_by_fqn(conn, ment.clone());
        let mut mention = link::Mention::default();
        mention.link_props.set_href_string(user.clone().map(|u| u.ap_url).unwrap_or(String::new())).expect("Error setting mention's href");
        mention.link_props.set_name_string(format!("@{}", ment)).expect("Error setting mention's name");
        mention
    }

    pub fn to_activity(&self, conn: &PgConnection) -> link::Mention {
        let user = self.get_mentioned(conn);
        let mut mention = link::Mention::default();
        mention.link_props.set_href_string(user.clone().map(|u| u.ap_url).unwrap_or(String::new())).expect("Error setting mention's href");
        mention.link_props.set_name_string(user.map(|u| format!("@{}", u.get_fqn(conn))).unwrap_or(String::new())).expect("Error setting mention's name");
        mention
    }

    pub fn from_activity(conn: &PgConnection, ment: link::Mention, inside: i32, in_post: bool, notify: bool) -> Option<Self> {
        let ap_url = ment.link_props.href_string().ok()?;
        let mentioned = User::find_by_ap_url(conn, ap_url)?;

        if in_post {
            Post::get(conn, inside.clone().into()).map(|post| {
                let res = Mention::insert(conn, NewMention {
                    mentioned_id: mentioned.id,
                    post_id: Some(post.id),
                    comment_id: None,
                    ap_url: ment.link_props.href_string().unwrap_or(String::new())
                });
                if notify {
                    res.notify(conn);
                }
                res
            })
        } else {
            Comment::get(conn, inside.into()).map(|comment| {
                let res = Mention::insert(conn, NewMention {
                    mentioned_id: mentioned.id,
                    post_id: None,
                    comment_id: Some(comment.id),
                    ap_url: ment.link_props.href_string().unwrap_or(String::new())
                });
                if notify {
                    res.notify(conn);
                }
                res
            })
        }
    }
}

impl Notify<PgConnection> for Mention {
    fn notify(&self, conn: &PgConnection) {
        self.get_mentioned(conn).map(|m| {
            Notification::insert(conn, NewNotification {
                kind: notification_kind::MENTION.to_string(),
                object_id: self.id,
                user_id: m.id
            });
        });
    }
}
