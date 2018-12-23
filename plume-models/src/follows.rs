use activitypub::{
    activity::{Accept, Follow as FollowAct, Undo},
    actor::Person,
    Actor,
};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

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
use {ap_url, Connection, BASE_URL};

#[derive(Clone, Queryable, Identifiable, Associations)]
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
    insert!(follows, NewFollow);
    get!(follows);
    find_by!(follows, find_by_ap_url, ap_url as &str);

    pub fn find(conn: &Connection, from: i32, to: i32) -> Option<Follow> {
        follows::table
            .filter(follows::follower_id.eq(from))
            .filter(follows::following_id.eq(to))
            .get_result(conn)
            .ok()
    }

    pub fn to_activity(&self, conn: &Connection) -> FollowAct {
        let user = User::get(conn, self.follower_id)
            .expect("Follow::to_activity: actor not found error");
        let target = User::get(conn, self.following_id)
            .expect("Follow::to_activity: target not found error");

        let mut act = FollowAct::default();
        act.follow_props
            .set_actor_link::<Id>(user.clone().into_id())
            .expect("Follow::to_activity: actor error");
        act.follow_props
            .set_object_link::<Id>(target.clone().into_id())
            .expect("Follow::to_activity: object error");
        act.object_props
            .set_id_string(self.ap_url.clone())
            .expect("Follow::to_activity: id error");
        act.object_props
            .set_to_link(target.into_id())
            .expect("Follow::to_activity: target error");
        act.object_props
            .set_cc_link_vec::<Id>(vec![])
            .expect("Follow::to_activity: cc error");
        act
    }

    /// from -> The one sending the follow request
    /// target -> The target of the request, responding with Accept
    pub fn accept_follow<A: Signer + IntoId + Clone, B: Clone + WithInbox + Actor + IntoId>(
        conn: &Connection,
        from: &B,
        target: &A,
        follow: FollowAct,
        from_id: i32,
        target_id: i32,
    ) -> Follow {
        let res = Follow::insert(
            conn,
            NewFollow {
                follower_id: from_id,
                following_id: target_id,
                ap_url: follow.object_props.id_string().expect("Follow::accept_follow: get id error"),
            },
        );

        let mut accept = Accept::default();
        let accept_id = ap_url(&format!("{}/follow/{}/accept", BASE_URL.as_str(), &res.id));
        accept
            .object_props
            .set_id_string(accept_id)
            .expect("Follow::accept_follow: set id error");
        accept
            .object_props
            .set_to_link(from.clone().into_id())
            .expect("Follow::accept_follow: to error");
        accept
            .object_props
            .set_cc_link_vec::<Id>(vec![])
            .expect("Follow::accept_follow: cc error");
        accept
            .accept_props
            .set_actor_link::<Id>(target.clone().into_id())
            .expect("Follow::accept_follow: actor error");
        accept
            .accept_props
            .set_object_object(follow)
            .expect("Follow::accept_follow: object error");
        broadcast(&*target, accept, vec![from.clone()]);
        res
    }
}

impl FromActivity<FollowAct, Connection> for Follow {
    fn from_activity(conn: &Connection, follow: FollowAct, _actor: Id) -> Follow {
        let from_id = follow
            .follow_props
            .actor_link::<Id>()
            .map(|l| l.into())
            .unwrap_or_else(|_| {
                follow
                    .follow_props
                    .actor_object::<Person>()
                    .expect("Follow::from_activity: actor not found error")
                    .object_props
                    .id_string()
                    .expect("Follow::from_activity: actor not found error")
            });
        let from =
            User::from_url(conn, &from_id).expect("Follow::from_activity: actor not found error");
        match User::from_url(
            conn,
            follow
                .follow_props
                .object
                .as_str()
                .expect("Follow::from_activity: target url parsing error"),
        ) {
            Some(user) => Follow::accept_follow(conn, &from, &user, follow, from.id, user.id),
            None => {
                let blog = Blog::from_url(
                    conn,
                    follow
                        .follow_props
                        .object
                        .as_str()
                        .expect("Follow::from_activity: target url parsing error"),
                ).expect("Follow::from_activity: target not found error");
                Follow::accept_follow(conn, &from, &blog, follow, from.id, blog.id)
            }
        }
    }
}

impl Notify<Connection> for Follow {
    fn notify(&self, conn: &Connection) {
        Notification::insert(
            conn,
            NewNotification {
                kind: notification_kind::FOLLOW.to_string(),
                object_id: self.id,
                user_id: self.following_id,
            },
        );
    }
}

impl Deletable<Connection, Undo> for Follow {
    fn delete(&self, conn: &Connection) -> Undo {
        diesel::delete(self)
            .execute(conn)
            .expect("Follow::delete: follow deletion error");

        // delete associated notification if any
        if let Some(notif) = Notification::find(conn, notification_kind::FOLLOW, self.id) {
            diesel::delete(&notif)
                .execute(conn)
                .expect("Follow::delete: notification deletion error");
        }

        let mut undo = Undo::default();
        undo.undo_props
            .set_actor_link(
                User::get(conn, self.follower_id)
                    .expect("Follow::delete: actor error")
                    .into_id(),
            )
            .expect("Follow::delete: actor error");
        undo.object_props
            .set_id_string(format!("{}/undo", self.ap_url))
            .expect("Follow::delete: id error");
        undo.undo_props
            .set_object_link::<Id>(self.clone().into_id())
            .expect("Follow::delete: object error");
        undo
    }

    fn delete_id(id: &str, actor_id: &str, conn: &Connection) {
        if let Some(follow) = Follow::find_by_ap_url(conn, id) {
            if let Some(user) = User::find_by_ap_url(conn, actor_id) {
                if user.id == follow.follower_id {
                    follow.delete(conn);
                }
            }
        }
    }
}

impl IntoId for Follow {
    fn into_id(self) -> Id {
        Id::new(self.ap_url)
    }
}
