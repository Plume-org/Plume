use activitypub::activity::{Announce, Undo};
use chrono::NaiveDateTime;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use activity_pub::{Id, IntoId, actor::Actor, inbox::{FromActivity, Notify}, object::Object};
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
    pub fn insert(conn: &PgConnection, new: NewReshare) -> Reshare {
        diesel::insert_into(reshares::table)
            .values(new)
            .get_result(conn)
            .expect("Couldn't save reshare")
    }

    get!(reshares);

    pub fn update_ap_url(&self, conn: &PgConnection) {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(reshares::ap_url.eq(format!(
                    "{}/reshare/{}",
                    User::get(conn, self.user_id).unwrap().compute_id(conn),
                    Post::get(conn, self.post_id).unwrap().compute_id(conn)
                )))
                .get_result::<Reshare>(conn).expect("Couldn't update AP URL");
        }
    }

    find_by!(reshares, find_by_ap_url, ap_url as String);

    pub fn find_by_user_on_post(conn: &PgConnection, user: &User, post: &Post) -> Option<Reshare> {
        reshares::table.filter(reshares::post_id.eq(post.id))
            .filter(reshares::user_id.eq(user.id))
            .limit(1)
            .load::<Reshare>(conn)
            .expect("Error loading reshare for user and post")
            .into_iter().nth(0)
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

        act
    }
}

impl FromActivity<Announce> for Reshare {
    fn from_activity(conn: &PgConnection, announce: Announce, actor: Id) -> Reshare {
        let user = User::from_url(conn, announce.announce_props.actor.as_str().unwrap().to_string());
        let post = Post::find_by_ap_url(conn, announce.announce_props.object.as_str().unwrap().to_string());
        let reshare = Reshare::insert(conn, NewReshare {
            post_id: post.unwrap().id,
            user_id: user.unwrap().id,
            ap_url: announce.object_props.id_string().unwrap_or(String::from(""))
        });
        Reshare::notify(conn, announce, actor);
        reshare
    }
}

impl Notify<Announce> for Reshare {
    fn notify(conn: &PgConnection, announce: Announce, actor: Id) {
        let actor = User::from_url(conn, actor.into()).unwrap();
        let post = Post::find_by_ap_url(conn, announce.announce_props.object_link::<Id>().unwrap().into()).unwrap();
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
