use crate::{
    notifications::*, posts::Post, schema::reshares, timeline::*, users::User, Connection, Error,
    PlumeRocket, Result, CONFIG,
};
use activitypub::activity::{Announce, Undo};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use plume_common::activity_pub::{
    inbox::{AsActor, AsObject, FromId},
    Id, IntoId, PUBLIC_VISIBILITY,
};

#[derive(Clone, Queryable, Identifiable)]
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

    pub fn get_recents_for_author(
        conn: &Connection,
        user: &User,
        limit: i64,
    ) -> Result<Vec<Reshare>> {
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
        act.object_props.set_id_string(self.ap_url.clone())?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props
            .set_cc_link_vec(vec![Id::new(self.get_user(conn)?.followers_endpoint)])?;

        Ok(act)
    }

    pub fn notify(&self, conn: &Connection) -> Result<()> {
        let post = self.get_post(conn)?;
        for author in post.get_authors(conn)? {
            if author.is_local() {
                Notification::insert(
                    conn,
                    NewNotification {
                        kind: notification_kind::RESHARE.to_string(),
                        object_id: self.id,
                        user_id: author.id,
                    },
                )?;
            }
        }
        Ok(())
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<Undo> {
        let mut act = Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.undo_props.set_object_object(self.to_activity(conn)?)?;
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props
            .set_cc_link_vec(vec![Id::new(self.get_user(conn)?.followers_endpoint)])?;

        Ok(act)
    }
}

impl AsObject<User, Announce, &PlumeRocket> for Post {
    type Error = Error;
    type Output = Reshare;

    fn activity(self, c: &PlumeRocket, actor: User, id: &str) -> Result<Reshare> {
        let conn = &*c.conn;
        let reshare = Reshare::insert(
            conn,
            NewReshare {
                post_id: self.id,
                user_id: actor.id,
                ap_url: id.to_string(),
            },
        )?;
        reshare.notify(conn)?;

        Timeline::add_to_all_timelines(c, &self, Kind::Reshare(&actor))?;
        Ok(reshare)
    }
}

impl FromId<PlumeRocket> for Reshare {
    type Error = Error;
    type Object = Announce;

    fn from_db(c: &PlumeRocket, id: &str) -> Result<Self> {
        Reshare::find_by_ap_url(&c.conn, id)
    }

    fn from_activity(c: &PlumeRocket, act: Announce) -> Result<Self> {
        let res = Reshare::insert(
            &c.conn,
            NewReshare {
                post_id: Post::from_id(
                    c,
                    &act.announce_props.object_link::<Id>()?,
                    None,
                    CONFIG.proxy(),
                )
                .map_err(|(_, e)| e)?
                .id,
                user_id: User::from_id(
                    c,
                    &act.announce_props.actor_link::<Id>()?,
                    None,
                    CONFIG.proxy(),
                )
                .map_err(|(_, e)| e)?
                .id,
                ap_url: act.object_props.id_string()?,
            },
        )?;
        res.notify(&c.conn)?;
        Ok(res)
    }
}

impl AsObject<User, Undo, &PlumeRocket> for Reshare {
    type Error = Error;
    type Output = ();

    fn activity(self, c: &PlumeRocket, actor: User, _id: &str) -> Result<()> {
        let conn = &*c.conn;
        if actor.id == self.user_id {
            diesel::delete(&self).execute(conn)?;

            // delete associated notification if any
            if let Ok(notif) = Notification::find(&conn, notification_kind::RESHARE, self.id) {
                diesel::delete(&notif).execute(conn)?;
            }

            Ok(())
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
            ap_url,
        }
    }
}
