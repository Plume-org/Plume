use crate::{
    comments::Comment, notifications::*, posts::Post, schema::mentions, users::User, Connection,
    Error, Result,
};
use activitystreams::{
    base::BaseExt,
    iri_string::types::IriString,
    link::{self, LinkExt},
};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use plume_common::activity_pub::inbox::AsActor;

#[derive(Clone, Queryable, Identifiable)]
pub struct Mention {
    pub id: i32,
    pub mentioned_id: i32,
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = mentions)]
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

    pub fn get_mentioned(&self, conn: &mut Connection) -> Result<User> {
        User::get(conn, self.mentioned_id)
    }

    pub fn get_post(&self, conn: &mut Connection) -> Result<Post> {
        self.post_id
            .ok_or(Error::NotFound)
            .and_then(|id| Post::get(conn, id))
    }

    pub fn get_comment(&self, conn: &mut Connection) -> Result<Comment> {
        self.comment_id
            .ok_or(Error::NotFound)
            .and_then(|id| Comment::get(conn, id))
    }

    pub fn get_user(&self, conn: &mut Connection) -> Result<User> {
        match self.get_post(conn) {
            Ok(p) => Ok(p
                .get_authors(conn)?
                .into_iter()
                .next()
                .ok_or(Error::NotFound)?),
            Err(_) => self.get_comment(conn).and_then(|c| c.get_author(conn)),
        }
    }

    pub async fn build_activity(conn: &mut Connection, ment: &str) -> Result<link::Mention> {
        let user = User::find_by_fqn(conn, ment).await?;
        let mut mention = link::Mention::new();
        mention.set_href(user.ap_url.parse::<IriString>()?);
        mention.set_name(format!("@{}", ment));
        Ok(mention)
    }

    pub fn to_activity(&self, conn: &mut Connection) -> Result<link::Mention> {
        let user = self.get_mentioned(conn)?;
        let mut mention = link::Mention::new();
        mention.set_href(user.ap_url.parse::<IriString>()?);
        mention.set_name(format!("@{}", user.fqn));
        Ok(mention)
    }

    pub fn from_activity(
        conn: &mut Connection,
        ment: &link::Mention,
        inside: i32,
        in_post: bool,
        notify: bool,
    ) -> Result<Self> {
        let ap_url = ment.href().ok_or(Error::NotFound)?.as_str();
        let mentioned = User::find_by_ap_url(conn, ap_url)?;

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

    pub fn delete(&self, conn: &mut Connection) -> Result<()> {
        //find related notifications and delete them
        if let Ok(n) = Notification::find(conn, notification_kind::MENTION, self.id) {
            n.delete(conn)?;
        }
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    fn notify(&self, conn: &mut Connection) -> Result<()> {
        let m = self.get_mentioned(conn)?;
        if m.is_local() {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::MENTION.to_string(),
                    object_id: self.id,
                    user_id: m.id,
                },
            )
            .map(|_| ())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{inbox::tests::fill_database, tests::db, Error};
    use assert_json_diff::assert_json_eq;
    use diesel::Connection;
    use serde_json::{json, to_value};

    #[test]
    fn build_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (_posts, users, _blogs) = fill_database(&conn);
            let user = &users[0];
            let name = &user.username;
            let act = Mention::build_activity(&conn, name)?;

            let expected = json!({
                "href": "https://plu.me/@/admin/",
                "name": "@admin",
                "type": "Mention",
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn to_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (posts, users, _blogs) = fill_database(&conn);
            let post = &posts[0];
            let user = &users[0];
            let mention = Mention::insert(
                &conn,
                NewMention {
                    mentioned_id: user.id,
                    post_id: Some(post.id),
                    comment_id: None,
                },
            )?;
            let act = mention.to_activity(&conn)?;

            let expected = json!({
                "href": "https://plu.me/@/admin/",
                "name": "@admin",
                "type": "Mention",
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }
}
