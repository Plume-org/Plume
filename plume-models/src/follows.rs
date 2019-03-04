use activitypub::{
    activity::{Accept, Follow as FollowAct, Undo},
};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};

use notifications::*;
use plume_common::activity_pub::{
    broadcast,
    inbox::{AsActor, AsObject},
    // inbox::{Deletable, FromActivity, Notify, WithInbox},
    sign::Signer,
    Id, IntoId,
};
use schema::follows;
use users::{User, CustomPerson};
use {ap_url, Connection, Context, BASE_URL, Error, Result};

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
    insert!(follows, NewFollow, |inserted, conn| {
        if inserted.ap_url.is_empty() {
            inserted.ap_url = ap_url(&format!("{}/follows/{}", *BASE_URL, inserted.id));
            inserted.save_changes(conn).map_err(Error::from)
        } else {
            Ok(inserted)
        }
    });
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
        act.follow_props
            .set_actor_link::<Id>(user.clone().into_id())?;
        act.follow_props
            .set_object_link::<Id>(target.clone().into_id())?;
        act.object_props
            .set_id_string(self.ap_url.clone())?;
        act.object_props
            .set_to_link(target.into_id())?;
        act.object_props
            .set_cc_link_vec::<Id>(vec![])?;
        Ok(act)
    }

    pub fn notify(&self, conn: &Connection) -> Result<Notification> {
        Notification::insert(
            conn,
            NewNotification {
                kind: notification_kind::FOLLOW.to_string(),
                object_id: self.id,
                user_id: self.following_id,
            },
        )
    }

    /// from -> The one sending the follow request
    /// target -> The target of the request, responding with Accept
    pub fn accept_follow<A: Signer + IntoId + Clone, B: Clone + AsActor<T> + IntoId, T>(
        conn: &Connection,
        from: &B,
        target: &A,
        follow: String,
        from_id: i32,
        target_id: i32,
    ) -> Result<Follow> {
        let res = Follow::insert(
            conn,
            NewFollow {
                follower_id: from_id,
                following_id: target_id,
                ap_url: follow.clone(),
            },
        )?;
        res.notify(conn)?;

        let mut accept = Accept::default();
        let accept_id = ap_url(&format!("{}/follow/{}/accept", BASE_URL.as_str(), &res.id));
        accept
            .object_props
            .set_id_string(accept_id)?;
        accept
            .object_props
            .set_to_link(from.clone().into_id())?;
        accept
            .object_props
            .set_cc_link_vec::<Id>(vec![])?;
        accept
            .accept_props
            .set_actor_link::<Id>(target.clone().into_id())?;
        accept
            .accept_props
            .set_object_link(Id::new(follow))?;
        broadcast(&*target, accept, vec![from.clone()]);
        Ok(res)
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<Undo> {
        let mut undo = Undo::default();
        undo.undo_props
            .set_actor_link(
                User::get(conn, self.follower_id)?
                    .into_id(),
            )?;
        undo.object_props
            .set_id_string(format!("{}/undo", self.ap_url))?;
        undo.undo_props
            .set_object_link::<Id>(self.clone().into_id())?;
        Ok(undo)
    }
}

impl<'a> AsObject<User, FollowAct, CustomPerson, &Context<'a>> for User {
    type Error = Error;
    type Output = Follow;

    fn activity(c: &Context, actor: User, target: CustomPerson, id: &str) -> Result<Follow> {
        let target = User::from_activity(c.conn, &target)?;
        Follow::accept_follow(c.conn, &actor, &target, id.to_string(), actor.id, target.id)
    }
}

impl<'a> AsObject<User, Undo, FollowAct, &Context<'a>> for Follow {
    type Error = Error;
    type Output = ();

    fn activity(c: &Context, actor: User, follow: FollowAct, _id: &str) -> Result<()> {
        let conn = c.conn;
        let follow = Follow::find_by_ap_url(conn, &follow.object_props.id_string()?)?;
        if follow.follower_id == actor.id {
            diesel::delete(&follow)
                .execute(conn)?;

            // delete associated notification if any
            if let Ok(notif) = Notification::find(conn, notification_kind::FOLLOW, follow.id) {
                diesel::delete(&notif)
                    .execute(conn)?;
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
    use diesel::Connection;
    use super::*;
    use tests::db;
    use users::tests as user_tests;

    #[test]
    fn test_id() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let users = user_tests::fill_database(&conn);
            let follow = Follow::insert(&conn, NewFollow {
                follower_id: users[0].id,
                following_id: users[1].id,
                ap_url: String::new(),
            }).expect("Couldn't insert new follow");
            assert_eq!(follow.ap_url, format!("https://{}/follows/{}", *BASE_URL, follow.id));

            let follow = Follow::insert(&conn, NewFollow {
                follower_id: users[1].id,
                following_id: users[0].id,
                ap_url: String::from("https://some.url/"),
            }).expect("Couldn't insert new follow");
            assert_eq!(follow.ap_url, String::from("https://some.url/"));
            Ok(())
        });
    }
}