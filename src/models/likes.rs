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
    pub creation_date: chrono::NaiveDateTime
}

#[derive(Insertable)]
#[table_name = "likes"]
pub struct NewLike {
    pub user_id: i32,
    pub post_id: i32
}

impl Like {
    pub fn insert(conn: &PgConnection, new: NewLike) -> Like {
        diesel::insert_into(likes::table)
            .values(new)
            .get_result(conn)
            .expect("Unable to insert new like")
    }

     pub fn get(conn: &PgConnection, id: i32) -> Option<Like> {
        likes::table.filter(likes::id.eq(id))
            .limit(1)
            .load::<Like>(conn)
            .expect("Error loading like by ID")
            .into_iter().nth(0)
    }

    pub fn for_user_on_post(conn: &PgConnection, user: &User, post: &Post) -> Option<Like> {
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
