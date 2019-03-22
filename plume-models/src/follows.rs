use activitypub::{
    activity::{Accept, Follow as FollowAct, Undo},
    actor::Person,
};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};

use blogs::Blog;
use notifications::*;
use plume_common::activity_pub::{
    broadcast,
    inbox::{Deletable, FromActivity, Notify, WithInbox},
    sign::Signer,
    Id, IntoId,
};
use schema::follows;
use users::User;
use {ap_url, Connection, Error, Result, CONFIG};

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
            inserted.ap_url = ap_url(&format!("{}/follows/{}", CONFIG.base_url, inserted.id));
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
        act.object_props.set_id_string(self.ap_url.clone())?;
        act.object_props.set_to_link(target.into_id())?;
        act.object_props.set_cc_link_vec::<Id>(vec![])?;
        Ok(act)
    }

    /// from -> The one sending the follow request
    /// target -> The target of the request, responding with Accept
    pub fn accept_follow<A: Signer + IntoId + Clone, B: Clone + WithInbox + IntoId>(
        conn: &Connection,
        from: &B,
        target: &A,
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

        let mut accept = Accept::default();
        let accept_id = ap_url(&format!("{}/follow/{}/accept", CONFIG.base_url.as_str(), &res.id));
        accept.object_props.set_id_string(accept_id)?;
        accept.object_props.set_to_link(from.clone().into_id())?;
        accept.object_props.set_cc_link_vec::<Id>(vec![])?;
        accept
            .accept_props
            .set_actor_link::<Id>(target.clone().into_id())?;
        accept.accept_props.set_object_object(follow)?;
        broadcast(&*target, accept, vec![from.clone()]);
        Ok(res)
    }
}

impl FromActivity<FollowAct, Connection> for Follow {
    type Error = Error;

    fn from_activity(conn: &Connection, follow: FollowAct, _actor: Id) -> Result<Follow> {
        let from_id = follow
            .follow_props
            .actor_link::<Id>()
            .map(|l| l.into())
            .or_else(|_| {
                Ok(follow
                    .follow_props
                    .actor_object::<Person>()?
                    .object_props
                    .id_string()?) as Result<String>
            })?;
        let from = User::from_url(conn, &from_id)?;
        match User::from_url(conn, follow.follow_props.object.as_str()?) {
            Ok(user) => Follow::accept_follow(conn, &from, &user, follow, from.id, user.id),
            Err(_) => {
                let blog = Blog::from_url(conn, follow.follow_props.object.as_str()?)?;
                Follow::accept_follow(conn, &from, &blog, follow, from.id, blog.id)
            }
        }
    }
}

impl Notify<Connection> for Follow {
    type Error = Error;

    fn notify(&self, conn: &Connection) -> Result<()> {
        Notification::insert(
            conn,
            NewNotification {
                kind: notification_kind::FOLLOW.to_string(),
                object_id: self.id,
                user_id: self.following_id,
            },
        )
        .map(|_| ())
    }
}

impl Deletable<Connection, Undo> for Follow {
    type Error = Error;

    fn delete(&self, conn: &Connection) -> Result<Undo> {
        diesel::delete(self).execute(conn)?;

        // delete associated notification if any
        if let Ok(notif) = Notification::find(conn, notification_kind::FOLLOW, self.id) {
            diesel::delete(&notif).execute(conn)?;
        }

        let mut undo = Undo::default();
        undo.undo_props
            .set_actor_link(User::get(conn, self.follower_id)?.into_id())?;
        undo.object_props
            .set_id_string(format!("{}/undo", self.ap_url))?;
        undo.undo_props
            .set_object_link::<Id>(self.clone().into_id())?;
        Ok(undo)
    }

    fn delete_id(id: &str, actor_id: &str, conn: &Connection) -> Result<Undo> {
        let follow = Follow::find_by_ap_url(conn, id)?;
        let user = User::find_by_ap_url(conn, actor_id)?;
        if user.id == follow.follower_id {
            follow.delete(conn)
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
    use diesel::Connection;
    use tests::db;
    use users::tests as user_tests;

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
        });
    }
}
