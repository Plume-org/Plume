use activitypub::activity;
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use notifications::*;
use plume_common::activity_pub::{
    inbox::{Deletable, FromActivity, Notify},
    Id, IntoId, PUBLIC_VISIBILTY,
};
use posts::Post;
use schema::likes;
use users::User;
use {Connection, Error, Result};

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
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))?;
        act.object_props.set_cc_link_vec::<Id>(vec![])?;
        act.object_props.set_id_string(self.ap_url.clone())?;

        Ok(act)
    }
}

impl FromActivity<activity::Like, Connection> for Like {
    type Error = Error;

    fn from_activity(conn: &Connection, like: activity::Like, _actor: Id) -> Result<Like> {
        let liker = User::from_url(conn, like.like_props.actor.as_str()?)?;
        let post = Post::find_by_ap_url(conn, like.like_props.object.as_str()?)?;
        let res = Like::insert(
            conn,
            NewLike {
                post_id: post.id,
                user_id: liker.id,
                ap_url: like.object_props.id_string()?,
            },
        )?;
        res.notify(conn)?;
        Ok(res)
    }
}

impl Notify<Connection> for Like {
    type Error = Error;

    fn notify(&self, conn: &Connection) -> Result<()> {
        let post = Post::get(conn, self.post_id)?;
        for author in post.get_authors(conn)? {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::LIKE.to_string(),
                    object_id: self.id,
                    user_id: author.id,
                },
            )?;
        }
        Ok(())
    }
}

impl Deletable<Connection, activity::Undo> for Like {
    type Error = Error;

    fn delete(&self, conn: &Connection) -> Result<activity::Undo> {
        diesel::delete(self).execute(conn)?;

        // delete associated notification if any
        if let Ok(notif) = Notification::find(conn, notification_kind::LIKE, self.id) {
            diesel::delete(&notif).execute(conn)?;
        }

        let mut act = activity::Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.undo_props.set_object_object(self.to_activity(conn)?)?;
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))?;
        act.object_props.set_cc_link_vec::<Id>(vec![])?;

        Ok(act)
    }

    fn delete_id(id: &str, actor_id: &str, conn: &Connection) -> Result<activity::Undo> {
        let like = Like::find_by_ap_url(conn, id)?;
        let user = User::find_by_ap_url(conn, actor_id)?;
        if user.id == like.user_id {
            like.delete(conn)
        } else {
            Err(Error::Unauthorized)
        }
    }
}

impl NewLike {
    pub fn new(p: &Post, u: &User) -> Self {
        let ap_url = format!("{}/like/{}", u.ap_url, p.ap_url);
        NewLike {
            post_id: p.id,
            user_id: u.id,
            ap_url,
        }
    }
}
