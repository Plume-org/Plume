use chrono;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods};
use serde_json;

use activity_pub::{ap_url, PUBLIC_VISIBILTY};
use activity_pub::actor::Actor;
use activity_pub::object::Object;
use models::posts::Post;
use models::users::User;
use schema::comments;

#[derive(Queryable, Identifiable, Serialize, Clone)]
pub struct Comment {
    pub id: i32,
    pub content: String,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub creation_date: chrono::NaiveDateTime,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String
}

#[derive(Insertable)]
#[table_name = "comments"]
pub struct NewComment {
    pub content: String,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String
}

impl Comment {
    pub fn insert (conn: &PgConnection, new: NewComment) -> Comment {
        diesel::insert_into(comments::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new comment")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Comment> {
        comments::table.filter(comments::id.eq(id))
            .limit(1)
            .load::<Comment>(conn)
            .expect("Error loading comment by id")
            .into_iter().nth(0)
    }

    pub fn find_by_post(conn: &PgConnection, post_id: i32) -> Vec<Comment> {
        comments::table.filter(comments::post_id.eq(post_id))
            .load::<Comment>(conn)
            .expect("Error loading comment by post id")
    }

    pub fn find_by_ap_url(conn: &PgConnection, ap_url: String) -> Option<Comment> {
        comments::table.filter(comments::ap_url.eq(ap_url))
            .limit(1)
            .load::<Comment>(conn)
            .expect("Error loading comment by AP URL")
            .into_iter().nth(0)
    }

    pub fn get_author(&self, conn: &PgConnection) -> User {
        User::get(conn, self.author_id).unwrap()
    }

    pub fn get_post(&self, conn: &PgConnection) -> Post {
        Post::get(conn, self.post_id).unwrap()        
    }
}

impl Object for Comment {
    fn serialize(&self, conn: &PgConnection) -> serde_json::Value {
        let mut to = self.get_author(conn).get_followers(conn).into_iter().map(|f| f.ap_url).collect::<Vec<String>>();
        to.append(&mut self.get_post(conn).get_receivers_urls(conn));
        to.push(PUBLIC_VISIBILTY.to_string());

        json!({
            "id": self.compute_id(conn),
            "type": "Note",
            "summary": self.spoiler_text,
            "content": self.content,
            "inReplyTo": self.in_response_to_id.map_or_else(|| self.get_post(conn).ap_url, |id| {
                let comm = Comment::get(conn, id).unwrap();
                comm.ap_url.clone().unwrap_or(comm.compute_id(conn))
            }),
            "published": self.creation_date,
            "attributedTo": self.get_author(conn).compute_id(conn),
            "to": to,
            "cc": [],
            "sensitive": self.sensitive,
        })
    }

    fn compute_id(&self, conn: &PgConnection) -> String {
        ap_url(format!("{}#comment-{}", self.get_post(conn).compute_id(conn), self.id))
    }
}
