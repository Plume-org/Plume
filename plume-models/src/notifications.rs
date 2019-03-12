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
use {Connection, Error, Result};

pub mod notification_kind {
    pub const COMMENT: &str = "COMMENT";
    pub const FOLLOW: &str = "FOLLOW";
    pub const LIKE: &str = "LIKE";
    pub const MENTION: &str = "MENTION";
    pub const RESHARE: &str = "RESHARE";
}

#[derive(Clone, Queryable, Identifiable)]
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

    pub fn find_for_user(conn: &Connection, user: &User) -> Result<Vec<Notification>> {
        notifications::table
            .filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .load::<Notification>(conn)
            .map_err(Error::from)
    }

    pub fn count_for_user(conn: &Connection, user: &User) -> Result<i64> {
        notifications::table
            .filter(notifications::user_id.eq(user.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn page_for_user(
        conn: &Connection,
        user: &User,
        (min, max): (i32, i32),
    ) -> Result<Vec<Notification>> {
        notifications::table
            .filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Notification>(conn)
            .map_err(Error::from)
    }

    pub fn find<S: Into<String>>(conn: &Connection, kind: S, obj: i32) -> Result<Notification> {
        notifications::table
            .filter(notifications::kind.eq(kind.into()))
            .filter(notifications::object_id.eq(obj))
            .get_result::<Notification>(conn)
            .map_err(Error::from)
    }

    pub fn get_url(&self, conn: &Connection) -> Option<String> {
        match self.kind.as_ref() {
            notification_kind::COMMENT => self.get_post(conn).and_then(|p| Some(format!("{}#comment-{}", p.url(conn).ok()?, self.object_id))),
            notification_kind::FOLLOW => Some(format!("/@/{}/", self.get_actor(conn).ok()?.fqn)),
            notification_kind::MENTION => Mention::get(conn, self.object_id).and_then(|mention|
                mention.get_post(conn).and_then(|p| p.url(conn))
                    .or_else(|_| {
                        let comment = mention.get_comment(conn)?;
                        Ok(format!("{}#comment-{}", comment.get_post(conn)?.url(conn)?, comment.id))
                    })
            ).ok(),
            _ => None,
        }
    }

    pub fn get_post(&self, conn: &Connection) -> Option<Post> {
        match self.kind.as_ref() {
            notification_kind::COMMENT => Comment::get(conn, self.object_id).and_then(|comment| comment.get_post(conn)).ok(),
            notification_kind::LIKE => Like::get(conn, self.object_id).and_then(|like| Post::get(conn, like.post_id)).ok(),
            notification_kind::RESHARE => Reshare::get(conn, self.object_id).and_then(|reshare| reshare.get_post(conn)).ok(),
            _ => None,
        }
    }

    pub fn get_actor(&self, conn: &Connection) -> Result<User> {
        Ok(match self.kind.as_ref() {
            notification_kind::COMMENT => Comment::get(conn, self.object_id)?.get_author(conn)?,
            notification_kind::FOLLOW => User::get(conn, Follow::get(conn, self.object_id)?.follower_id)?,
            notification_kind::LIKE => User::get(conn, Like::get(conn, self.object_id)?.user_id)?,
            notification_kind::MENTION => Mention::get(conn, self.object_id)?.get_user(conn)?,
            notification_kind::RESHARE => Reshare::get(conn, self.object_id)?.get_user(conn)?,
            _ => unreachable!("Notification::get_actor: Unknow type"),
        })
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

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }
}
