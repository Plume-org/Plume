use crate::{
    ap_url, notifications::*, schema::follows, users::User, Connection, Error, PlumeRocket, Result,
    CONFIG,
};
use activitypub::activity::{Accept, Follow as FollowAct, Undo};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};
use plume_common::activity_pub::{
    broadcast,
    inbox::{AsActor, AsObject, FromId},
    sign::Signer,
    Id, IntoId, PUBLIC_VISIBILITY,
};

#[derive(Clone, Queryable, Identifiable, Associations, AsChangeset)]
#[belongs_to(User, foreign_key = "following_id")]
pub struct Follow {
    pub id: i32,
    pub follower_id: i32,
    pub following_id: i32,
    pub ap_url: String,
}

#[derive(Insertable)]
#[table_name = "follows"]
pub struct NewFollow {
    pub follower_id: i32,
    pub following_id: i32,
    pub ap_url: String,
}

impl Follow {
    insert!(
        follows,
        NewFollow,
        |inserted, conn| if inserted.ap_url.is_empty() {
            inserted.ap_url = ap_url(&format!("{}/follows/{}", CONFIG.base_url, inserted.id));
            inserted.save_changes(conn).map_err(Error::from)
        } else {
            Ok(inserted)
        }
    );
    get!(follows);
    find_by!(follows, find_by_ap_url, ap_url as &str);

    pub fn find(conn: &Connection, from: i32, to: i32) -> Result<Follow> {
        follows::table
            .filter(follows::follower_id.eq(from))
            .filter(follows::following_id.eq(to))
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<FollowAct> {
        let user = User::get(conn, self.follower_id)?;
        let target = User::get(conn, self.following_id)?;

        let mut act = FollowAct::default();
        act.follow_props.set_actor_link::<Id>(user.into_id())?;
        act.follow_props
            .set_object_link::<Id>(target.clone().into_id())?;
        act.object_props.set_id_string(self.ap_url.clone())?;
        act.object_props.set_to_link_vec(vec![target.into_id()])?;
        act.object_props
            .set_cc_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        Ok(act)
    }

    pub fn notify(&self, conn: &Connection) -> Result<()> {
        if User::get(conn, self.following_id)?.is_local() {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::FOLLOW.to_string(),
                    object_id: self.id,
                    user_id: self.following_id,
                },
            )?;
        }
        Ok(())
    }

    /// from -> The one sending the follow request
    /// target -> The target of the request, responding with Accept
    pub fn accept_follow<A: Signer + IntoId + Clone + Sync, B: Clone + AsActor<T> + IntoId, T>(
        conn: &Connection,
        from: &B,
        target: &'static A,
        follow: FollowAct,
        from_id: i32,
        target_id: i32,
    ) -> Result<Follow> {
        let res = Follow::insert(
            conn,
            NewFollow {
                follower_id: from_id,
                following_id: target_id,
                ap_url: follow.object_props.id_string()?,
            },
        )?;
        res.notify(conn)?;

        let mut accept = Accept::default();
        let accept_id = ap_url(&format!(
            "{}/follow/{}/accept",
            CONFIG.base_url.as_str(),
            &res.id
        ));
        accept.object_props.set_id_string(accept_id)?;
        accept
            .object_props
            .set_to_link_vec(vec![from.clone().into_id()])?;
        accept
            .object_props
            .set_cc_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        accept
            .accept_props
            .set_actor_link::<Id>(target.clone().into_id())?;
        accept.accept_props.set_object_object(follow)?;
        broadcast(&*target, accept, vec![from.clone()]);
        Ok(res)
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<Undo> {
        let mut undo = Undo::default();
        undo.undo_props
            .set_actor_link(User::get(conn, self.follower_id)?.into_id())?;
        undo.object_props
            .set_id_string(format!("{}/undo", self.ap_url))?;
        undo.undo_props
            .set_object_link::<Id>(self.clone().into_id())?;
        undo.object_props
            .set_to_link_vec(vec![User::get(conn, self.following_id)?.into_id()])?;
        undo.object_props
            .set_cc_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        Ok(undo)
    }
}

impl AsObject<User, FollowAct, &PlumeRocket> for User {
    type Error = Error;
    type Output = Follow;

    fn activity(self, c: &PlumeRocket, actor: User, id: &str) -> Result<Follow> {
        // Mastodon (at least) requires the full Follow object when accepting it,
        // so we rebuilt it here
        let mut follow = FollowAct::default();
        follow.object_props.set_id_string(id.to_string())?;
        follow
            .follow_props
            .set_actor_link::<Id>(actor.clone().into_id())?;
        Follow::accept_follow(&c.conn, &actor, &self, follow, actor.id, self.id)
    }
}

impl FromId<PlumeRocket> for Follow {
    type Error = Error;
    type Object = FollowAct;

    fn from_db(c: &PlumeRocket, id: &str) -> Result<Self> {
        Follow::find_by_ap_url(&c.conn, id)
    }

    fn from_activity(c: &PlumeRocket, follow: FollowAct) -> Result<Self> {
        let actor =
            User::from_id(c, &follow.follow_props.actor_link::<Id>()?, None).map_err(|(_, e)| e)?;

        let target = User::from_id(c, &follow.follow_props.object_link::<Id>()?, None)
            .map_err(|(_, e)| e)?;
        Follow::accept_follow(&c.conn, &actor, &target, follow, actor.id, target.id)
    }
}

impl AsObject<User, Undo, &PlumeRocket> for Follow {
    type Error = Error;
    type Output = ();

    fn activity(self, c: &PlumeRocket, actor: User, _id: &str) -> Result<()> {
        let conn = &*c.conn;
        if self.follower_id == actor.id {
            diesel::delete(&self).execute(conn)?;

            // delete associated notification if any
            if let Ok(notif) = Notification::find(conn, notification_kind::FOLLOW, self.id) {
                diesel::delete(&notif).execute(conn)?;
            }

            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
}

impl IntoId for Follow {
    fn into_id(self) -> Id {
        Id::new(self.ap_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tests::db, users::tests as user_tests};
    use diesel::Connection;

    #[test]
    fn test_id() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let users = user_tests::fill_database(&conn);
            let follow = Follow::insert(
                &conn,
                NewFollow {
                    follower_id: users[0].id,
                    following_id: users[1].id,
                    ap_url: String::new(),
                },
            )
            .expect("Couldn't insert new follow");
            assert_eq!(
                follow.ap_url,
                format!("https://{}/follows/{}", CONFIG.base_url, follow.id)
            );

            let follow = Follow::insert(
                &conn,
                NewFollow {
                    follower_id: users[1].id,
                    following_id: users[0].id,
                    ap_url: String::from("https://some.url/"),
                },
            )
            .expect("Couldn't insert new follow");
            assert_eq!(follow.ap_url, String::from("https://some.url/"));

            Ok(())
        })
    }
}
