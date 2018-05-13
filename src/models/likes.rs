use chrono;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};
use serde_json;

use activity_pub::actor::Actor;
use activity_pub::object::Object;
use models::posts::Post;
use models::users::User;
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
    pub fn insert(conn: &PgConnection, new: NewLike) -> Like {
        diesel::insert_into(likes::table)
            .values(new)
            .get_result(conn)
            .expect("Unable to insert new like")
    }

    pub fn update_ap_url(&self, conn: &PgConnection) {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(likes::ap_url.eq(self.compute_id(conn)))
                .get_result::<Like>(conn).expect("Couldn't update AP URL");
        }
    }

     pub fn get(conn: &PgConnection, id: i32) -> Option<Like> {
        likes::table.filter(likes::id.eq(id))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like by ID")
            .into_iter().nth(0)
    }

    pub fn find_by_ap_url(conn: &PgConnection, ap_url: String) -> Option<Like> {
        likes::table.filter(likes::ap_url.eq(ap_url))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like by AP URL")
            .into_iter().nth(0)
    }

    pub fn find_by_user_on_post(conn: &PgConnection, user: &User, post: &Post) -> Option<Like> {
        likes::table.filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(user.id))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like for user and post")
            .into_iter().nth(0)
    }

    pub fn delete(&self, conn: &PgConnection) {
        diesel::delete(self).execute(conn).unwrap();
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
