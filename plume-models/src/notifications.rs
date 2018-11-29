use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use comments::Comment;
use follows::Follow;
use likes::Like;
use mentions::Mention;
use posts::Post;
use reshares::Reshare;
use schema::notifications;
use users::User;
use Connection;

pub mod notification_kind {
    pub const COMMENT: &str = "COMMENT";
    pub const FOLLOW: &str = "FOLLOW";
    pub const LIKE: &str = "LIKE";
    pub const MENTION: &str = "MENTION";
    pub const RESHARE: &str = "RESHARE";
}

#[derive(Clone, Queryable, Identifiable, Serialize)]
pub struct Notification {
    pub id: i32,
    pub user_id: i32,
    pub creation_date: NaiveDateTime,
    pub kind: String,
    pub object_id: i32,
}

#[derive(Insertable)]
#[table_name = "notifications"]
pub struct NewNotification {
    pub user_id: i32,
    pub kind: String,
    pub object_id: i32,
}

impl Notification {
    insert!(notifications, NewNotification);
    get!(notifications);

    pub fn find_for_user(conn: &Connection, user: &User) -> Vec<Notification> {
        notifications::table
            .filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .load::<Notification>(conn)
            .expect("Notification::find_for_user: notification loading error")
    }

    pub fn page_for_user(
        conn: &Connection,
        user: &User,
        (min, max): (i32, i32),
    ) -> Vec<Notification> {
        notifications::table
            .filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Notification>(conn)
            .expect("Notification::page_for_user: notification loading error")
    }

    pub fn find<S: Into<String>>(conn: &Connection, kind: S, obj: i32) -> Option<Notification> {
        notifications::table
            .filter(notifications::kind.eq(kind.into()))
            .filter(notifications::object_id.eq(obj))
            .get_result::<Notification>(conn)
            .ok()
    }

    pub fn get_message(&self) -> &'static str {
        match self.kind.as_ref() {
            notification_kind::COMMENT => "{0} commented your article.",
            notification_kind::FOLLOW => "{0} is now following you.",
            notification_kind::LIKE => "{0} liked your article.",
            notification_kind::MENTION => "{0} mentioned you.",
            notification_kind::RESHARE => "{0} boosted your article.",
            _ => unreachable!("Notification::get_message: Unknow type"),
        }
    }

    pub fn get_url(&self, conn: &Connection) -> Option<String> {
        match self.kind.as_ref() {
            notification_kind::COMMENT => self.get_post(conn).map(|p| format!("{}#comment-{}", p.url(conn), self.object_id)),
            notification_kind::FOLLOW => Some(format!("/@/{}/", self.get_actor(conn).get_fqn(conn))),
            notification_kind::MENTION => Mention::get(conn, self.object_id).map(|mention|
                mention.get_post(conn).map(|p| p.url(conn))
                    .unwrap_or_else(|| {
                        let comment = mention.get_comment(conn).expect("Notification::get_url: comment not found error");
                        format!("{}#comment-{}", comment.get_post(conn).url(conn), comment.id)
                    })
            ),
            _ => None,
        }
    }

    pub fn get_post(&self, conn: &Connection) -> Option<Post> {
        match self.kind.as_ref() {
            notification_kind::COMMENT => Comment::get(conn, self.object_id).map(|comment| comment.get_post(conn)),
            notification_kind::LIKE => Like::get(conn, self.object_id).and_then(|like| Post::get(conn, like.post_id)),
            notification_kind::RESHARE => Reshare::get(conn, self.object_id).and_then(|reshare| reshare.get_post(conn)),
            _ => None,
        }
    }

    pub fn get_actor(&self, conn: &Connection) -> User {
        match self.kind.as_ref() {
            notification_kind::COMMENT => Comment::get(conn, self.object_id).expect("Notification::get_actor: comment error").get_author(conn),
            notification_kind::FOLLOW => User::get(conn, Follow::get(conn, self.object_id).expect("Notification::get_actor: follow error").follower_id)
                .expect("Notification::get_actor: follower error"),
            notification_kind::LIKE => User::get(conn, Like::get(conn, self.object_id).expect("Notification::get_actor: like error").user_id)
                .expect("Notification::get_actor: liker error"),
            notification_kind::MENTION => Mention::get(conn, self.object_id).expect("Notification::get_actor: mention error").get_user(conn)
                .expect("Notification::get_actor: mentioner error"),
            notification_kind::RESHARE => Reshare::get(conn, self.object_id).expect("Notification::get_actor: reshare error").get_user(conn)
                .expect("Notification::get_actor: resharer error"),
            _ => unreachable!("Notification::get_actor: Unknow type"),
        }
    }

    pub fn icon_class(&self) -> &'static str {
        match self.kind.as_ref() {
            notification_kind::COMMENT => "icon-message-circle",
            notification_kind::FOLLOW => "icon-user-plus",
            notification_kind::LIKE => "icon-heart",
            notification_kind::MENTION => "icon-at-sign",
            notification_kind::RESHARE => "icon-repeat",
            _ => unreachable!("Notification::get_actor: Unknow type"),
        }
    }

    pub fn delete(&self, conn: &Connection) {
        diesel::delete(self)
            .execute(conn)
            .expect("Notification::delete: notification deletion error");
    }
}
