use activitypub::activity;
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use notifications::*;
use plume_common::activity_pub::{
    inbox::AsObject,
    Id, IntoId, PUBLIC_VISIBILTY,
};
use posts::{Post, LicensedArticle};
use schema::likes;
use users::User;
use {Connection, Context, Error, Result};

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
            .set_actor_link(
                User::get(conn, self.user_id)?
                    .into_id(),
            )?;
        act.like_props
            .set_object_link(
                Post::get(conn, self.post_id)?
                    .into_id(),
            )?;
        act.object_props
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))?;
        act.object_props
            .set_cc_link_vec::<Id>(vec![])?;
        act.object_props
            .set_id_string(self.ap_url.clone())?;

        Ok(act)
    }

    pub fn notify(&self, conn: &Connection) -> Result<()> {
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

    pub fn build_undo(&self, conn: &Connection) -> Result<activity::Undo> {
        let mut act = activity::Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id(),)?;
        act.undo_props
            .set_object_object(self.to_activity(conn)?)?;
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))?;
        act.object_props
            .set_cc_link_vec::<Id>(vec![])?;

        Ok(act)
    }
}

impl<'a> AsObject<User, activity::Like, LicensedArticle, &Context<'a>> for Post {
    type Error = Error;
    type Output = ();

    fn activity(c: &Context, actor: User, article: LicensedArticle, id: &str) -> Result<()> {
        let post = Post::from_activity(&c.conn, &c.searcher, article)?;
        let res = Like::insert(
            &c.conn,
            NewLike {
                post_id: post.id,
                user_id: actor.id,
                ap_url: id.to_string(),
            },
        )?;
        res.notify(&c.conn)?;
        Ok(())
    }
}

impl<'a> AsObject<User, activity::Undo, activity::Like, &Context<'a>> for Like {
    type Error = Error;
    type Output = ();

    fn activity(c: &Context, actor: User, like: activity::Like, _id: &str) -> Result<()> {
        let conn = c.conn;
        let like = Like::find_by_ap_url(conn, &like.object_props.id_string()?)?;
        if actor.id == like.user_id {
            diesel::delete(&like)
                .execute(conn)?;

            // delete associated notification if any
            if let Ok(notif) = Notification::find(conn, notification_kind::LIKE, like.id) {
                diesel::delete(&notif)
                    .execute(conn)?;
            }
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
}

impl NewLike {
    pub fn new(p: &Post, u: &User) -> Self {
        // TODO: this URL is not valid
        let ap_url = format!("{}/like/{}", u.ap_url, p.ap_url);
        NewLike {
            post_id: p.id,
            user_id: u.id,
            ap_url
        }
    }
}
