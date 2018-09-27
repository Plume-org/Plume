use chrono::NaiveDateTime;
use diesel::{self, RunQueryDsl, QueryDsl, ExpressionMethods};
use serde_json;

use Connection;
use comments::Comment;
use follows::Follow;
use likes::Like;
use mentions::Mention;
use posts::Post;
use reshares::Reshare;
use users::User;
use schema::notifications;

pub mod notification_kind {
    pub const COMMENT: &'static str = "COMMENT";
    pub const FOLLOW: &'static str = "FOLLOW";
    pub const LIKE: &'static str = "LIKE";
    pub const MENTION: &'static str = "MENTION";
    pub const RESHARE: &'static str = "RESHARE";
}

#[derive(Clone, Queryable, Identifiable, Serialize)]
pub struct Notification {
    pub id: i32,
    pub user_id: i32,
    pub creation_date: NaiveDateTime,
    pub kind: String,
    pub object_id: i32
}

#[derive(Insertable)]
#[table_name = "notifications"]
pub struct NewNotification {
    pub user_id: i32,
    pub kind: String,
    pub object_id: i32
}

impl Notification {
    insert!(notifications, NewNotification);
    get!(notifications);

    pub fn find_for_user(conn: &Connection, user: &User) -> Vec<Notification> {
        notifications::table.filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .load::<Notification>(conn)
            .expect("Couldn't load user notifications")
    }

    pub fn page_for_user(conn: &Connection, user: &User, (min, max): (i32, i32)) -> Vec<Notification> {
        notifications::table.filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Notification>(conn)
            .expect("Couldn't load user notifications page")
    }

    pub fn find<S: Into<String>>(conn: &Connection, kind: S, obj: i32) -> Option<Notification> {
        notifications::table.filter(notifications::kind.eq(kind.into()))
            .filter(notifications::object_id.eq(obj))
            .get_result::<Notification>(conn)
            .ok()
    }

    pub fn to_json(&self, conn: &Connection) -> serde_json::Value {
        let mut json = json!(self);
        json["object"] = json!(match self.kind.as_ref() {
            notification_kind::COMMENT => Comment::get(conn, self.object_id).map(|comment|
                json!({
                    "post": comment.get_post(conn).to_json(conn),
                    "user": comment.get_author(conn).to_json(conn),
                    "id": comment.id
                })
            ),
            notification_kind::FOLLOW => Follow::get(conn, self.object_id).map(|follow|
                json!({
                    "follower": User::get(conn, follow.follower_id).map(|u| u.to_json(conn))
                })
            ),
            notification_kind::LIKE => Like::get(conn, self.object_id).map(|like|
                json!({
                    "post": Post::get(conn, like.post_id).map(|p| p.to_json(conn)),
                    "user": User::get(conn, like.user_id).map(|u| u.to_json(conn))
                })
            ),
            notification_kind::MENTION => Mention::get(conn, self.object_id).map(|mention|
                json!({
                    "user": mention.get_user(conn).map(|u| u.to_json(conn)),
                    "url": mention.get_post(conn).map(|p| p.to_json(conn)["url"].clone())
                        .unwrap_or_else(|| {
                            let comment = mention.get_comment(conn).expect("No comment nor post for mention");
                            let post = comment.get_post(conn).to_json(conn);
                            json!(format!("{}#comment-{}", post["url"].as_str().unwrap(), comment.id))
                        })
                })
            ),
            notification_kind::RESHARE => Reshare::get(conn, self.object_id).map(|reshare|
                json!({
                    "post": reshare.get_post(conn).map(|p| p.to_json(conn)),
                    "user": reshare.get_user(conn).map(|u| u.to_json(conn))
                })
            ),
            _ => Some(json!({}))
        });
        json
    }
}
