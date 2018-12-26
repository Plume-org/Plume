use activitypub::activity::{Announce, Undo};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use notifications::*;
use plume_common::activity_pub::{
    inbox::{Deletable, FromActivity, Notify},
    Id, IntoId, PUBLIC_VISIBILTY,
};
use posts::Post;
use schema::reshares;
use users::User;
use {Connection, Error, Result};

#[derive(Clone, Serialize, Deserialize, Queryable, Identifiable)]
pub struct Reshare {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
    pub creation_date: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "reshares"]
pub struct NewReshare {
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
}

impl Reshare {
    insert!(reshares, NewReshare);
    get!(reshares);
    find_by!(reshares, find_by_ap_url, ap_url as &str);
    find_by!(
        reshares,
        find_by_user_on_post,
        user_id as i32,
        post_id as i32
    );

    pub fn get_recents_for_author(conn: &Connection, user: &User, limit: i64) -> Result<Vec<Reshare>> {
        reshares::table
            .filter(reshares::user_id.eq(user.id))
            .order(reshares::creation_date.desc())
            .limit(limit)
            .load::<Reshare>(conn)
            .map_err(Error::from)
    }

    pub fn get_post(&self, conn: &Connection) -> Result<Post> {
        Post::get(conn, self.post_id)
    }

    pub fn get_user(&self, conn: &Connection) -> Result<User> {
        User::get(conn, self.user_id)
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<Announce> {
        let mut act = Announce::default();
        act.announce_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.announce_props
            .set_object_link(Post::get(conn, self.post_id)?.into_id())?;
        act.object_props
            .set_id_string(self.ap_url.clone())?;
        act.object_props
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))?;
        act.object_props
            .set_cc_link_vec::<Id>(vec![])?;

        Ok(act)
    }
}

impl FromActivity<Announce, Connection> for Reshare {
    type Error = Error;

    fn from_activity(conn: &Connection, announce: Announce, _actor: Id) -> Result<Reshare> {
        let user = User::from_url(
            conn,
            announce
                .announce_props
                .actor_link::<Id>()?
                .as_ref(),
        )?;
        let post = Post::find_by_ap_url(
            conn,
            announce
                .announce_props
                .object_link::<Id>()?
                .as_ref(),
        )?;
        let reshare = Reshare::insert(
            conn,
            NewReshare {
                post_id: post.id,
                user_id: user.id,
                ap_url: announce
                    .object_props
                    .id_string()
                    .unwrap_or_default(),
            },
        )?;
        reshare.notify(conn)?;
        Ok(reshare)
    }
}

impl Notify<Connection> for Reshare {
    type Error = Error;

    fn notify(&self, conn: &Connection) -> Result<()> {
        let post = self.get_post(conn)?;
        for author in post.get_authors(conn)? {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::RESHARE.to_string(),
                    object_id: self.id,
                    user_id: author.id,
                },
            )?;
        }
        Ok(())
    }
}

impl Deletable<Connection, Undo> for Reshare {
    type Error = Error;

    fn delete(&self, conn: &Connection) -> Result<Undo> {
        diesel::delete(self)
            .execute(conn)?;

        // delete associated notification if any
        if let Ok(notif) = Notification::find(conn, notification_kind::RESHARE, self.id) {
            diesel::delete(&notif)
                .execute(conn)?;
        }

        let mut act = Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
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

    fn delete_id(id: &str, actor_id: &str, conn: &Connection) -> Result<Undo> {
        let reshare = Reshare::find_by_ap_url(conn, id)?;
        let actor = User::find_by_ap_url(conn, actor_id)?;
        if actor.id == reshare.user_id {
            reshare.delete(conn)
        } else {
            Err(Error::Unauthorized)
        }
    }
}

impl NewReshare {
    pub fn new(p: &Post, u: &User) -> Self {
        let ap_url = format!("{}/reshare/{}", u.ap_url, p.ap_url);
        NewReshare {
            post_id: p.id,
            user_id: u.id,
            ap_url
        }
    }
}
