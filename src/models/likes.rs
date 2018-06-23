use activitypub::activity;
use chrono;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use activity_pub::{
    PUBLIC_VISIBILTY,
    Id,
    IntoId,
    inbox::{FromActivity, Deletable, Notify}
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

    pub fn delete(&self, conn: &PgConnection) -> activity::Undo {
        diesel::delete(self).execute(conn).unwrap();

        let mut act = activity::Undo::default();
        act.undo_props.set_actor_link(User::get(conn, self.user_id).unwrap().into_id()).expect("Like::delete: actor error");
        act.undo_props.set_object_object(self.into_activity(conn)).expect("Like::delete: object error");
        act.object_props.set_id_string(format!("{}#delete", self.ap_url)).expect("Like::delete: id error");
        act.object_props.set_to_link(Id::new(PUBLIC_VISIBILTY.to_string())).expect("Like::delete: to error");
        act.object_props.set_cc_link_vec::<Id>(vec![]).expect("Like::delete: cc error");

        act
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

impl FromActivity<activity::Like> for Like {
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

impl Notify for Like {
    fn notify(&self, conn: &PgConnection) {
        let liker = User::get(conn, self.user_id).unwrap();
        let post = Post::get(conn, self.post_id).unwrap();
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
