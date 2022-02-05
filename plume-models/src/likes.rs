use crate::{
    db_conn::DbConn, instance::Instance, notifications::*, posts::Post, schema::likes, timeline::*,
    users::User, Connection, Error, Result, CONFIG,
};
use activitypub::activity;
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use plume_common::activity_pub::{
    inbox::{AsActor, AsObject, FromId},
    sign::Signer,
    Id, IntoId, PUBLIC_VISIBILITY,
};

#[derive(Clone, Queryable, Identifiable)]
pub struct Like {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: String,
}

#[derive(Default, Insertable)]
#[table_name = "likes"]
pub struct NewLike {
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
}

impl Like {
    insert!(likes, NewLike);
    get!(likes);
    find_by!(likes, find_by_ap_url, ap_url as &str);
    find_by!(likes, find_by_user_on_post, user_id as i32, post_id as i32);

    pub fn to_activity(&self, conn: &Connection) -> Result<activity::Like> {
        let mut act = activity::Like::default();
        act.like_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.like_props
            .set_object_link(Post::get(conn, self.post_id)?.into_id())?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props.set_cc_link_vec(vec![Id::new(
            User::get(conn, self.user_id)?.followers_endpoint,
        )])?;
        act.object_props.set_id_string(self.ap_url.clone())?;

        Ok(act)
    }

    pub fn notify(&self, conn: &Connection) -> Result<()> {
        let post = Post::get(conn, self.post_id)?;
        for author in post.get_authors(conn)? {
            if author.is_local() {
                Notification::insert(
                    conn,
                    NewNotification {
                        kind: notification_kind::LIKE.to_string(),
                        object_id: self.id,
                        user_id: author.id,
                    },
                )?;
            }
        }
        Ok(())
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<activity::Undo> {
        let mut act = activity::Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.undo_props.set_object_object(self.to_activity(conn)?)?;
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props.set_cc_link_vec(vec![Id::new(
            User::get(conn, self.user_id)?.followers_endpoint,
        )])?;

        Ok(act)
    }
}

impl AsObject<User, activity::Like, &DbConn> for Post {
    type Error = Error;
    type Output = Like;

    fn activity(self, conn: &DbConn, actor: User, id: &str) -> Result<Like> {
        let res = Like::insert(
            conn,
            NewLike {
                post_id: self.id,
                user_id: actor.id,
                ap_url: id.to_string(),
            },
        )?;
        res.notify(conn)?;

        Timeline::add_to_all_timelines(conn, &self, Kind::Like(&actor))?;
        Ok(res)
    }
}

impl FromId<DbConn> for Like {
    type Error = Error;
    type Object = activity::Like;

    fn from_db(conn: &DbConn, id: &str) -> Result<Self> {
        Like::find_by_ap_url(conn, id)
    }

    fn from_activity(conn: &DbConn, act: activity::Like) -> Result<Self> {
        let res = Like::insert(
            conn,
            NewLike {
                post_id: Post::from_id(
                    conn,
                    &act.like_props.object_link::<Id>()?,
                    None,
                    CONFIG.proxy(),
                )
                .map_err(|(_, e)| e)?
                .id,
                user_id: User::from_id(
                    conn,
                    &act.like_props.actor_link::<Id>()?,
                    None,
                    CONFIG.proxy(),
                )
                .map_err(|(_, e)| e)?
                .id,
                ap_url: act.object_props.id_string()?,
            },
        )?;
        res.notify(conn)?;
        Ok(res)
    }

    fn get_sender() -> &'static dyn Signer {
        Instance::get_local_instance_user().expect("Failed to local instance user")
    }
}

impl AsObject<User, activity::Undo, &DbConn> for Like {
    type Error = Error;
    type Output = ();

    fn activity(self, conn: &DbConn, actor: User, _id: &str) -> Result<()> {
        if actor.id == self.user_id {
            diesel::delete(&self).execute(&**conn)?;

            // delete associated notification if any
            if let Ok(notif) = Notification::find(conn, notification_kind::LIKE, self.id) {
                diesel::delete(&notif).execute(&**conn)?;
            }
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
}

impl NewLike {
    pub fn new(p: &Post, u: &User) -> Self {
        let ap_url = format!("{}like/{}", u.ap_url, p.ap_url);
        NewLike {
            post_id: p.id,
            user_id: u.id,
            ap_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diesel::Connection;
    use crate::{inbox::tests::fill_database, tests::db};
    use assert_json_diff::assert_json_eq;
    use serde_json::{json, to_value};

    #[test]
    fn to_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (posts, _users, _blogs) = fill_database(&conn);
            let post = &posts[0];
            let user = &post.get_authors(&conn)?[0];
            let like = Like::insert(&*conn, NewLike::new(post, user))?;
            let act = like.to_activity(&conn).unwrap();

            let expected = json!({
                "actor": "https://plu.me/@/admin/",
                "cc": ["https://plu.me/@/admin/followers"],
                "id": "https://plu.me/@/admin/like/https://plu.me/~/BlogName/testing",
                "object": "https://plu.me/~/BlogName/testing",
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "type": "Like",
            });
            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn build_undo() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (posts, _users, _blogs) = fill_database(&conn);
            let post = &posts[0];
            let user = &post.get_authors(&conn)?[0];
            let like = Like::insert(&*conn, NewLike::new(post, user))?;
            let act = like.build_undo(&*conn)?;

            let expected = json!({
                "actor": "https://plu.me/@/admin/",
                "cc": ["https://plu.me/@/admin/followers"],
                "id": "https://plu.me/@/admin/like/https://plu.me/~/BlogName/testing#delete",
                "object": {
                    "actor": "https://plu.me/@/admin/",
                    "cc": ["https://plu.me/@/admin/followers"],
                    "id": "https://plu.me/@/admin/like/https://plu.me/~/BlogName/testing",
                    "object": "https://plu.me/~/BlogName/testing",
                    "to": ["https://www.w3.org/ns/activitystreams#Public"],
                    "type": "Like",
                },
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "type": "Undo",
            });
            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }
}
