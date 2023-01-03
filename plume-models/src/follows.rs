use crate::{
    ap_url, db_conn::DbConn, instance::Instance, notifications::*, schema::follows, users::User,
    Connection, Error, Result, CONFIG,
};
use activitystreams::{
    activity::{Accept, ActorAndObjectRef, Follow as FollowAct, Undo},
    base::AnyBase,
    iri_string::types::IriString,
    prelude::*,
};
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
        let target_id = target.ap_url.parse::<IriString>()?;

        let mut act = FollowAct::new(user.ap_url.parse::<IriString>()?, target_id.clone());
        act.set_id(self.ap_url.parse::<IriString>()?);
        act.set_many_tos(vec![target_id]);
        act.set_many_ccs(vec![PUBLIC_VISIBILITY.parse::<IriString>()?]);

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
    pub fn accept_follow<A: Signer + IntoId + Clone, B: Clone + AsActor<T> + IntoId, T>(
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
                ap_url: follow
                    .object_field_ref()
                    .as_single_id()
                    .ok_or(Error::MissingApProperty)?
                    .to_string(),
            },
        )?;
        res.notify(conn)?;

        let accept = res.build_accept(from, target, follow)?;
        broadcast(target, accept, vec![from.clone()], CONFIG.proxy().cloned());
        Ok(res)
    }

    pub fn build_accept<A: Signer + IntoId + Clone, B: Clone + AsActor<T> + IntoId, T>(
        &self,
        from: &B,
        target: &A,
        follow: FollowAct,
    ) -> Result<Accept> {
        let mut accept = Accept::new(
            target.clone().into_id().parse::<IriString>()?,
            AnyBase::from_extended(follow)?,
        );
        let accept_id = ap_url(&format!(
            "{}/follows/{}/accept",
            CONFIG.base_url.as_str(),
            self.id
        ));
        accept.set_id(accept_id.parse::<IriString>()?);
        accept.set_many_tos(vec![from.clone().into_id().parse::<IriString>()?]);
        accept.set_many_ccs(vec![PUBLIC_VISIBILITY.parse::<IriString>()?]);

        Ok(accept)
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<Undo> {
        let mut undo = Undo::new(
            User::get(conn, self.follower_id)?
                .ap_url
                .parse::<IriString>()?,
            self.ap_url.parse::<IriString>()?,
        );
        undo.set_id(format!("{}/undo", self.ap_url).parse::<IriString>()?);
        undo.set_many_tos(vec![User::get(conn, self.following_id)?
            .ap_url
            .parse::<IriString>()?]);
        undo.set_many_ccs(vec![PUBLIC_VISIBILITY.parse::<IriString>()?]);

        Ok(undo)
    }
}

impl AsObject<User, FollowAct, &DbConn> for User {
    type Error = Error;
    type Output = Follow;

    fn activity(self, conn: &DbConn, actor: User, id: &str) -> Result<Follow> {
        // Mastodon (at least) requires the full Follow object when accepting it,
        // so we rebuilt it here
        let follow = FollowAct::new(actor.ap_url.parse::<IriString>()?, id.parse::<IriString>()?);
        Follow::accept_follow(conn, &actor, &self, follow, actor.id, self.id)
    }
}

impl FromId<DbConn> for Follow {
    type Error = Error;
    type Object = FollowAct;

    fn from_db(conn: &DbConn, id: &str) -> Result<Self> {
        Follow::find_by_ap_url(conn, id)
    }

    fn from_activity(conn: &DbConn, follow: FollowAct) -> Result<Self> {
        let actor = User::from_id(
            conn,
            follow
                .actor_field_ref()
                .as_single_id()
                .ok_or(Error::MissingApProperty)?
                .as_str(),
            None,
            CONFIG.proxy(),
        )
        .map_err(|(_, e)| e)?;

        let target = User::from_id(
            conn,
            follow
                .object_field_ref()
                .as_single_id()
                .ok_or(Error::MissingApProperty)?
                .as_str(),
            None,
            CONFIG.proxy(),
        )
        .map_err(|(_, e)| e)?;
        Follow::accept_follow(conn, &actor, &target, follow, actor.id, target.id)
    }

    fn get_sender() -> &'static dyn Signer {
        Instance::get_local_instance_user().expect("Failed to local instance user")
    }
}

impl AsObject<User, Undo, &DbConn> for Follow {
    type Error = Error;
    type Output = ();

    fn activity(self, conn: &DbConn, actor: User, _id: &str) -> Result<()> {
        let conn = conn;
        if self.follower_id == actor.id {
            diesel::delete(&self).execute(&**conn)?;

            // delete associated notification if any
            if let Ok(notif) = Notification::find(conn, notification_kind::FOLLOW, self.id) {
                diesel::delete(&notif).execute(&**conn)?;
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
    use crate::{tests::db, users::tests as user_tests, users::tests::fill_database};
    use assert_json_diff::assert_json_eq;
    use diesel::Connection;
    use serde_json::{json, to_value};

    fn prepare_activity(conn: &DbConn) -> (Follow, User, User, Vec<User>) {
        let users = fill_database(conn);
        let following = &users[1];
        let follower = &users[2];
        let mut follow = Follow::insert(
            conn,
            NewFollow {
                follower_id: follower.id,
                following_id: following.id,
                ap_url: "".into(),
            },
        )
        .unwrap();
        // following.ap_url = format!("https://plu.me/follows/{}", follow.id);
        follow.ap_url = format!("https://plu.me/follows/{}", follow.id);

        (follow, following.to_owned(), follower.to_owned(), users)
    }

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

    #[test]
    fn to_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (follow, _following, _follower, _users) = prepare_activity(&conn);
            let act = follow.to_activity(&conn)?;

            let expected = json!({
                "actor": "https://plu.me/@/other/",
                "cc": ["https://www.w3.org/ns/activitystreams#Public"],
                "id": format!("https://plu.me/follows/{}", follow.id),
                "object": "https://plu.me/@/user/",
                "to": ["https://plu.me/@/user/"],
                "type": "Follow"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn build_accept() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (follow, following, follower, _users) = prepare_activity(&conn);
            let act = follow.build_accept(&follower, &following, follow.to_activity(&conn)?)?;

            let expected = json!({
                "actor": "https://plu.me/@/user/",
                "cc": ["https://www.w3.org/ns/activitystreams#Public"],
                "id": format!("https://127.0.0.1:7878/follows/{}/accept", follow.id),
                "object": {
                    "actor": "https://plu.me/@/other/",
                    "cc": ["https://www.w3.org/ns/activitystreams#Public"],
                    "id": format!("https://plu.me/follows/{}", follow.id),
                    "object": "https://plu.me/@/user/",
                    "to": ["https://plu.me/@/user/"],
                    "type": "Follow"
                },
                "to": ["https://plu.me/@/other/"],
                "type": "Accept"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn build_undo() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (follow, _following, _follower, _users) = prepare_activity(&conn);
            let act = follow.build_undo(&conn)?;

            let expected = json!({
                "actor": "https://plu.me/@/other/",
                "cc": ["https://www.w3.org/ns/activitystreams#Public"],
                "id": format!("https://plu.me/follows/{}/undo", follow.id),
                "object": format!("https://plu.me/follows/{}", follow.id),
                "to": ["https://plu.me/@/user/"],
                "type": "Undo"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }
}
