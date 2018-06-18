use activitypub::activity;
use chrono;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};
use serde_json;

use activity_pub::{
    Id,
    IntoId,
    actor::Actor,
    inbox::{FromActivity, Deletable, Notify},
    object::Object
};
use models::{
    notifications::*,
    posts::Post,
    users::User
};
use schema::likes;

#[derive(Queryable, Identifiable)]
pub struct Like {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub creation_date: chrono::NaiveDateTime,
    pub ap_url: String
}

#[derive(Insertable)]
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

    pub fn update_ap_url(&self, conn: &PgConnection) {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(likes::ap_url.eq(self.compute_id(conn)))
                .get_result::<Like>(conn).expect("Couldn't update AP URL");
        }
    }

    pub fn find_by_user_on_post(conn: &PgConnection, user: &User, post: &Post) -> Option<Like> {
        likes::table.filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(user.id))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like for user and post")
            .into_iter().nth(0)
    }

    pub fn delete(&self, conn: &PgConnection) -> activity::Undo {
        diesel::delete(self).execute(conn).unwrap();

        let mut act = activity::Undo::default();
        act.undo_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).unwrap();
        act.undo_props.set_object_object(self.into_activity(conn)).unwrap();
        act
    }

    pub fn into_activity(&self, conn: &PgConnection) -> activity::Like {
        let mut act = activity::Like::default();
        act.like_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).unwrap();
        act.like_props.set_object_link(Post::get(conn, self.post_id).unwrap().into_id()).unwrap();
        act.object_props.set_id_string(format!("{}/like/{}",
            User::get(conn, self.user_id).unwrap().ap_url,
            Post::get(conn, self.post_id).unwrap().ap_url
        )).unwrap();

        act
    }
}

impl FromActivity<activity::Like> for Like {
    fn from_activity(conn: &PgConnection, like: activity::Like, actor: Id) -> Like {
        let liker = User::from_url(conn, like.like_props.actor.as_str().unwrap().to_string());
        let post = Post::find_by_ap_url(conn, like.like_props.object.as_str().unwrap().to_string());
        let res = Like::insert(conn, NewLike {
            post_id: post.unwrap().id,
            user_id: liker.unwrap().id,
            ap_url: like.object_props.id_string().unwrap_or(String::from(""))
        });
        Like::notify(conn, like, actor);
        res
    }
}

impl Notify<activity::Like> for Like {
    fn notify(conn: &PgConnection, like: activity::Like, actor: Id) {
        let liker = User::from_url(conn, actor.into()).unwrap();
        let post = Post::find_by_ap_url(conn, like.like_props.object_link::<Id>().unwrap().into()).unwrap();
        for author in post.get_authors(conn) {
            let post = post.clone();
            Notification::insert(conn, NewNotification {
                title: "{{ data }} liked your article".to_string(),
                data: Some(liker.display_name.clone()),
                content: Some(post.title),
                link: Some(post.ap_url),
                user_id: author.id
            });
        }
    }
}

impl Deletable for Like {
    fn delete_activity(conn: &PgConnection, id: Id) -> bool {
        if let Some(like) = Like::find_by_ap_url(conn, id.into()) {
            like.delete(conn);
            true
        } else {
            false
        }
    }
}

impl Object for Like {
    fn serialize(&self, conn: &PgConnection) -> serde_json::Value {
        json!({
            "id": self.compute_id(conn)
        })
    }

    fn compute_id(&self, conn: &PgConnection) -> String {
        format!(
            "{}/like/{}",
            User::get(conn, self.user_id).unwrap().compute_id(conn),
            Post::get(conn, self.post_id).unwrap().compute_id(conn)
        )
    }
}
