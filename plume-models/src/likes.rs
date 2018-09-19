use activitypub::activity;
use canapi::Provider;
use chrono;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use plume_common::activity_pub::{
    PUBLIC_VISIBILTY,
    Id,
    IntoId,
    inbox::{FromActivity, Deletable, Notify}
};
use notifications::*;
use posts::Post;
use users::User;
use schema::likes;

#[derive(Queryable, Identifiable)]
pub struct Like {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub creation_date: chrono::NaiveDateTime,
    pub ap_url: String
}

#[derive(Default, Insertable)]
#[table_name = "likes"]
pub struct NewLike {
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String
}

impl Like {
    insert!(likes, NewLike);
    get!(likes);
    find_by!(likes, find_by_ap_url, ap_url as String);
    find_by!(likes, find_by_user_on_post, user_id as i32, post_id as i32);

    pub fn update_ap_url(&self, conn: &PgConnection) {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(likes::ap_url.eq(format!(
                    "{}/like/{}",
                    User::get(conn, self.user_id).unwrap().ap_url,
                    Post::get(conn, self.post_id).unwrap().ap_url
                )))
                .get_result::<Like>(conn).expect("Couldn't update AP URL");
        }
    }

    pub fn into_activity(&self, conn: &PgConnection) -> activity::Like {
        let mut act = activity::Like::default();
        act.like_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).expect("Like::into_activity: actor error");
        act.like_props.set_object_link(Post::get(conn, self.post_id).unwrap().into_id()).expect("Like::into_activity: object error");
        act.object_props.set_to_link(Id::new(PUBLIC_VISIBILTY.to_string())).expect("Like::into_activity: to error");
        act.object_props.set_cc_link_vec::<Id>(vec![]).expect("Like::into_activity: cc error");
        act.object_props.set_id_string(self.ap_url.clone()).expect("Like::into_activity: id error");

        act
    }
}

impl FromActivity<activity::Like, PgConnection> for Like {
    fn from_activity(conn: &PgConnection, like: activity::Like, _actor: Id) -> Like {
        let liker = User::from_url(conn, like.like_props.actor.as_str().unwrap().to_string());
        let post = Post::find_by_ap_url(conn, like.like_props.object.as_str().unwrap().to_string());
        let res = Like::insert(conn, NewLike {
            post_id: post.unwrap().id,
            user_id: liker.unwrap().id,
            ap_url: like.object_props.id_string().unwrap_or(String::from(""))
        });
        res.notify(conn);
        res
    }
}

impl Notify<PgConnection> for Like {
    fn notify(&self, conn: &PgConnection) {
        let post = Post::get(conn, self.post_id).unwrap();
        for author in post.get_authors(conn) {
            Notification::insert(conn, NewNotification {
                kind: notification_kind::LIKE.to_string(),
                object_id: self.id,
                user_id: author.id
            });
        }
    }
}

impl Deletable<PgConnection, activity::Undo> for Like {
    fn delete(&self, conn: &PgConnection) -> activity::Undo {
        diesel::delete(self).execute(conn).unwrap();

        // delete associated notification if any
        if let Some(notif) = Notification::find(conn, notification_kind::LIKE, self.id) {
            diesel::delete(&notif).execute(conn).expect("Couldn't delete like notification");
        }

        let mut act = activity::Undo::default();
        act.undo_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).expect("Like::delete: actor error");
        act.undo_props.set_object_object(self.into_activity(conn)).expect("Like::delete: object error");
        act.object_props.set_id_string(format!("{}#delete", self.ap_url)).expect("Like::delete: id error");
        act.object_props.set_to_link(Id::new(PUBLIC_VISIBILTY.to_string())).expect("Like::delete: to error");
        act.object_props.set_cc_link_vec::<Id>(vec![]).expect("Like::delete: cc error");

        act
    }

    fn delete_id(id: String, conn: &PgConnection) {
        if let Some(like) = Like::find_by_ap_url(conn, id.into()) {
            like.delete(conn);
        }
    }
}
