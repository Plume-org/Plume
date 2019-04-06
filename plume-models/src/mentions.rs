use activitypub::link;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use comments::Comment;
use notifications::*;
use plume_common::activity_pub::inbox::Notify;
use posts::Post;
use schema::mentions;
use users::User;
use {Connection, Error, Result};

#[derive(Clone, Queryable, Identifiable)]
pub struct Mention {
    pub id: i32,
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,
}

#[derive(Insertable)]
#[table_name = "mentions"]
pub struct NewMention {
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,
}

impl Mention {
    insert!(mentions, NewMention);
    get!(mentions);
    list_by!(mentions, list_for_user, mentioned_id as i32);
    list_by!(mentions, list_for_post, post_id as i32);
    list_by!(mentions, list_for_comment, comment_id as i32);

    pub fn get_mentioned(&self, conn: &Connection) -> Result<User> {
        User::get(conn, self.mentioned_id)
    }

    pub fn get_post(&self, conn: &Connection) -> Result<Post> {
        self.post_id
            .ok_or(Error::NotFound)
            .and_then(|id| Post::get(conn, id))
    }

    pub fn get_comment(&self, conn: &Connection) -> Result<Comment> {
        self.comment_id
            .ok_or(Error::NotFound)
            .and_then(|id| Comment::get(conn, id))
    }

    pub fn get_user(&self, conn: &Connection) -> Result<User> {
        match self.get_post(conn) {
            Ok(p) => Ok(p.get_authors(conn)?.into_iter().next()?),
            Err(_) => self.get_comment(conn).and_then(|c| c.get_author(conn)),
        }
    }

    pub fn build_activity(conn: &Connection, ment: &str) -> Result<link::Mention> {
        let user = User::find_by_fqn(conn, ment)?;
        let mut mention = link::Mention::default();
        mention.link_props.set_href_string(user.ap_url)?;
        mention.link_props.set_name_string(format!("@{}", ment))?;
        Ok(mention)
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<link::Mention> {
        let user = self.get_mentioned(conn)?;
        let mut mention = link::Mention::default();
        mention.link_props.set_href_string(user.ap_url.clone())?;
        mention
            .link_props
            .set_name_string(format!("@{}", user.fqn))?;
        Ok(mention)
    }

    pub fn from_activity(
        conn: &Connection,
        ment: &link::Mention,
        inside: i32,
        in_post: bool,
        notify: bool,
    ) -> Result<Self> {
        let ap_url = ment.link_props.href_string().ok()?;
        let mentioned = User::find_by_ap_url(conn, &ap_url)?;

        if in_post {
            Post::get(conn, inside).and_then(|post| {
                let res = Mention::insert(
                    conn,
                    NewMention {
                        mentioned_id: mentioned.id,
                        post_id: Some(post.id),
                        comment_id: None,
                    },
                )?;
                if notify {
                    res.notify(conn)?;
                }
                Ok(res)
            })
        } else {
            Comment::get(conn, inside).and_then(|comment| {
                let res = Mention::insert(
                    conn,
                    NewMention {
                        mentioned_id: mentioned.id,
                        post_id: None,
                        comment_id: Some(comment.id),
                    },
                )?;
                if notify {
                    res.notify(conn)?;
                }
                Ok(res)
            })
        }
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        //find related notifications and delete them
        if let Ok(n) = Notification::find(conn, notification_kind::MENTION, self.id) {
            n.delete(conn)?;
        }
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }
}

impl Notify<Connection> for Mention {
    type Error = Error;
    fn notify(&self, conn: &Connection) -> Result<()> {
        let m = self.get_mentioned(conn)?;
        Notification::insert(
            conn,
            NewNotification {
                kind: notification_kind::MENTION.to_string(),
                object_id: self.id,
                user_id: m.id,
            },
        )
        .map(|_| ())
    }
}
