use activitypub::activity::{Announce, Undo};
use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods};

use plume_common::activity_pub::{Id, IntoId, inbox::{FromActivity, Notify, Deletable}, PUBLIC_VISIBILTY};
use Connection;
use notifications::*;
use posts::Post;
use users::User;
use schema::reshares;

#[derive(Clone, Serialize, Deserialize, Queryable, Identifiable)]
pub struct Reshare {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
    pub creation_date: NaiveDateTime
}

#[derive(Insertable)]
#[table_name = "reshares"]
pub struct NewReshare {
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String
}

impl Reshare {
    insert!(reshares, NewReshare);
    get!(reshares);
    find_by!(reshares, find_by_ap_url, ap_url as String);
    find_by!(reshares, find_by_user_on_post, user_id as i32, post_id as i32);

    pub fn update_ap_url(&self, conn: &Connection) {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(reshares::ap_url.eq(format!(
                    "{}/reshare/{}",
                    User::get(conn, self.user_id).expect("Reshare::update_ap_url: user error").ap_url,
                    Post::get(conn, self.post_id).expect("Reshare::update_ap_url: post error").ap_url
                )))
                .execute(conn).expect("Reshare::update_ap_url: update error");
        }
    }

    pub fn get_recents_for_author(conn: &Connection, user: &User, limit: i64) -> Vec<Reshare> {
        reshares::table.filter(reshares::user_id.eq(user.id))
            .order(reshares::creation_date.desc())
            .limit(limit)
            .load::<Reshare>(conn)
            .expect("Reshare::get_recents_for_author: loading error")
    }

    pub fn get_post(&self, conn: &Connection) -> Option<Post> {
        Post::get(conn, self.post_id)
    }

    pub fn get_user(&self, conn: &Connection) -> Option<User> {
        User::get(conn, self.user_id)
    }

    pub fn into_activity(&self, conn: &Connection) -> Announce {
        let mut act = Announce::default();
        act.announce_props.set_actor_link(User::get(conn, self.user_id).expect("Reshare::into_activity: user error").into_id())
            .expect("Reshare::into_activity: actor error");
        act.announce_props.set_object_link(Post::get(conn, self.post_id).expect("Reshare::into_activity: post error").into_id())
            .expect("Reshare::into_activity: object error");
        act.object_props.set_id_string(self.ap_url.clone()).expect("Reshare::into_activity: id error");
        act.object_props.set_to_link(Id::new(PUBLIC_VISIBILTY.to_string())).expect("Reshare::into_activity: to error");
        act.object_props.set_cc_link_vec::<Id>(vec![]).expect("Reshare::into_activity: cc error");

        act
    }
}

impl FromActivity<Announce, Connection> for Reshare {
    fn from_activity(conn: &Connection, announce: Announce, _actor: Id) -> Reshare {
        let user = User::from_url(conn, announce.announce_props.actor_link::<Id>().expect("Reshare::from_activity: actor error").into());
        let post = Post::find_by_ap_url(conn, announce.announce_props.object_link::<Id>().expect("Reshare::from_activity: object error").into());
        let reshare = Reshare::insert(conn, NewReshare {
            post_id: post.expect("Reshare::from_activity: post error").id,
            user_id: user.expect("Reshare::from_activity: user error").id,
            ap_url: announce.object_props.id_string().unwrap_or(String::from(""))
        });
        reshare.notify(conn);
        reshare
    }
}

impl Notify<Connection> for Reshare {
    fn notify(&self, conn: &Connection) {
        let post = self.get_post(conn).expect("Reshare::notify: post error");
        for author in post.get_authors(conn) {
            Notification::insert(conn, NewNotification {
                kind: notification_kind::RESHARE.to_string(),
                object_id: self.id,
                user_id: author.id
            });
        }
    }
}

impl Deletable<Connection, Undo> for Reshare {
    fn delete(&self, conn: &Connection) -> Undo {
        diesel::delete(self).execute(conn).expect("Reshare::delete: delete error");

        // delete associated notification if any
        if let Some(notif) = Notification::find(conn, notification_kind::RESHARE, self.id) {
            diesel::delete(&notif).execute(conn).expect("Reshare::delete: notification error");
        }

        let mut act = Undo::default();
        act.undo_props.set_actor_link(User::get(conn, self.user_id).expect("Reshare::delete: user error").into_id()).expect("Reshare::delete: actor error");
        act.undo_props.set_object_object(self.into_activity(conn)).expect("Reshare::delete: object error");
        act.object_props.set_id_string(format!("{}#delete", self.ap_url)).expect("Reshare::delete: id error");
        act.object_props.set_to_link(Id::new(PUBLIC_VISIBILTY.to_string())).expect("Reshare::delete: to error");
        act.object_props.set_cc_link_vec::<Id>(vec![]).expect("Reshare::delete: cc error");

        act
    }

    fn delete_id(id: String, conn: &Connection) {
        if let Some(reshare) = Reshare::find_by_ap_url(conn, id) {
            reshare.delete(conn);
        }
    }
}
