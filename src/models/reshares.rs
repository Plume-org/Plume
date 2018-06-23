use activitypub::activity::{Announce, Undo};
use chrono::NaiveDateTime;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use activity_pub::{Id, IntoId, inbox::{FromActivity, Notify, Deletable}, PUBLIC_VISIBILTY};
use models::{notifications::*, posts::Post, users::User};
use schema::reshares;

#[derive(Serialize, Deserialize, Queryable, Identifiable)]
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

    pub fn update_ap_url(&self, conn: &PgConnection) {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(reshares::ap_url.eq(format!(
                    "{}/reshare/{}",
                    User::get(conn, self.user_id).unwrap().ap_url,
                    Post::get(conn, self.post_id).unwrap().ap_url
                )))
                .get_result::<Reshare>(conn).expect("Couldn't update AP URL");
        }
    }

    pub fn get_recents_for_author(conn: &PgConnection, user: &User, limit: i64) -> Vec<Reshare> {
        reshares::table.filter(reshares::user_id.eq(user.id))
            .order(reshares::creation_date.desc())
            .limit(limit)
            .load::<Reshare>(conn)
            .expect("Error loading recent reshares for user")
    }

    pub fn get_post(&self, conn: &PgConnection) -> Option<Post> {
        Post::get(conn, self.post_id)
    }

    pub fn delete(&self, conn: &PgConnection) -> Undo {
        diesel::delete(self).execute(conn).unwrap();

        let mut act = Undo::default();
        act.undo_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).unwrap();
        act.undo_props.set_object_object(self.into_activity(conn)).unwrap();
        act
    }

    pub fn into_activity(&self, conn: &PgConnection) -> Announce {
        let mut act = Announce::default();
        act.announce_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).unwrap();
        act.announce_props.set_object_link(Post::get(conn, self.post_id).unwrap().into_id()).unwrap();
        act.object_props.set_id_string(self.ap_url.clone()).unwrap();
        act.object_props.set_to_link(Id::new(PUBLIC_VISIBILTY.to_string())).expect("Reshare::into_activity: to error");
        act.object_props.set_cc_link_vec::<Id>(vec![]).expect("Reshare::into_activity: cc error");

        act
    }
}

impl FromActivity<Announce> for Reshare {
    fn from_activity(conn: &PgConnection, announce: Announce, _actor: Id) -> Reshare {
        let user = User::from_url(conn, announce.announce_props.actor.as_str().unwrap().to_string());
        let post = Post::find_by_ap_url(conn, announce.announce_props.object.as_str().unwrap().to_string());
        let reshare = Reshare::insert(conn, NewReshare {
            post_id: post.unwrap().id,
            user_id: user.unwrap().id,
            ap_url: announce.object_props.id_string().unwrap_or(String::from(""))
        });
        reshare.notify(conn);
        reshare
    }
}

impl Notify for Reshare {
    fn notify(&self, conn: &PgConnection) {
        let actor = User::get(conn, self.user_id).unwrap();
        let post = self.get_post(conn).unwrap();
        for author in post.get_authors(conn) {
            let post = post.clone();
            Notification::insert(conn, NewNotification {
                title: "{{ data }} reshared your article".to_string(),
                data: Some(actor.display_name.clone()),
                content: Some(post.title),
                link: Some(post.ap_url),
                user_id: author.id
            });
        }
    }
}

impl Deletable for Reshare {
    fn delete_activity(conn: &PgConnection, id: Id) -> bool {
        if let Some(reshare) = Reshare::find_by_ap_url(conn, id.into()) {
            reshare.delete(conn);
            true
        } else {
            false
        }
    }
}
