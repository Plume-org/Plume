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
use Connection;

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
    find_by!(likes, find_by_ap_url, ap_url as String);
    find_by!(likes, find_by_user_on_post, user_id as i32, post_id as i32);

    pub fn update_ap_url(&self, conn: &Connection) {
        if self.ap_url.is_empty() {
            diesel::update(self)
                .set(likes::ap_url.eq(format!(
                    "{}/like/{}",
                    User::get(conn, self.user_id).expect("Like::update_ap_url: user error").ap_url,
                    Post::get(conn, self.post_id).expect("Like::update_ap_url: post error").ap_url
                )))
                .execute(conn)
                .expect("Like::update_ap_url: update error");
        }
    }

    pub fn to_activity(&self, conn: &Connection) -> activity::Like {
        let mut act = activity::Like::default();
        act.like_props
            .set_actor_link(
                User::get(conn, self.user_id)
                    .expect("Like::to_activity: user error")
                    .into_id(),
            )
            .expect("Like::to_activity: actor error");
        act.like_props
            .set_object_link(
                Post::get(conn, self.post_id)
                    .expect("Like::to_activity: post error")
                    .into_id(),
            )
            .expect("Like::to_activity: object error");
        act.object_props
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))
            .expect("Like::to_activity: to error");
        act.object_props
            .set_cc_link_vec::<Id>(vec![])
            .expect("Like::to_activity: cc error");
        act.object_props
            .set_id_string(self.ap_url.clone())
            .expect("Like::to_activity: id error");

        act
    }
}

impl FromActivity<activity::Like, Connection> for Like {
    fn from_activity(conn: &Connection, like: activity::Like, _actor: Id) -> Like {
        let liker = User::from_url(
            conn,
            like.like_props
                .actor
                .as_str()
                .expect("Like::from_activity: actor error"),
        );
        let post = Post::find_by_ap_url(
            conn,
            like.like_props
                .object
                .as_str()
                .expect("Like::from_activity: object error")
                .to_string(),
        );
        let res = Like::insert(
            conn,
            NewLike {
                post_id: post.expect("Like::from_activity: post error").id,
                user_id: liker.expect("Like::from_activity: user error").id,
                ap_url: like.object_props.id_string().unwrap_or_default(),
            },
        );
        res.notify(conn);
        res
    }
}

impl Notify<Connection> for Like {
    fn notify(&self, conn: &Connection) {
        let post = Post::get(conn, self.post_id).expect("Like::notify: post error");
        for author in post.get_authors(conn) {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::LIKE.to_string(),
                    object_id: self.id,
                    user_id: author.id,
                },
            );
        }
    }
}

impl Deletable<Connection, activity::Undo> for Like {
    fn delete(&self, conn: &Connection) -> activity::Undo {
        diesel::delete(self)
            .execute(conn)
            .expect("Like::delete: delete error");

        // delete associated notification if any
        if let Some(notif) = Notification::find(conn, notification_kind::LIKE, self.id) {
            diesel::delete(&notif)
                .execute(conn)
                .expect("Like::delete: notification error");
        }

        let mut act = activity::Undo::default();
        act.undo_props
            .set_actor_link(
                User::get(conn, self.user_id)
                    .expect("Like::delete: user error")
                    .into_id(),
            )
            .expect("Like::delete: actor error");
        act.undo_props
            .set_object_object(self.to_activity(conn))
            .expect("Like::delete: object error");
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))
            .expect("Like::delete: id error");
        act.object_props
            .set_to_link(Id::new(PUBLIC_VISIBILTY.to_string()))
            .expect("Like::delete: to error");
        act.object_props
            .set_cc_link_vec::<Id>(vec![])
            .expect("Like::delete: cc error");

        act
    }

    fn delete_id(id: String, actor_id: String, conn: &Connection) {
        if let Some(like) = Like::find_by_ap_url(conn, id) {
            if let Some(user) = User::find_by_ap_url(conn, actor_id) {
                if user.id == like.user_id {
                    like.delete(conn);
                }
            }
        }
    }
}
