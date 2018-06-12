use activitypub::{
    activity::Create,
    object::{Note, properties::ObjectProperties}
};
use chrono;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods, dsl::any};
use serde_json;

use activity_pub::{
    ap_url, IntoId, PUBLIC_VISIBILTY,
    actor::Actor,
    object::Object
};
use models::{
    instance::Instance,
    posts::Post,
    users::User
};
use schema::comments;
use safe_string::SafeString;

#[derive(Queryable, Identifiable, Serialize, Clone)]
pub struct Comment {
    pub id: i32,
    pub content: SafeString,
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
    pub content: SafeString,
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

    pub fn into_activity(&self, conn: &PgConnection) -> Note {
        let mut to = self.get_author(conn).get_followers(conn).into_iter().map(|f| f.ap_url).collect::<Vec<String>>();
        to.append(&mut self.get_post(conn).get_receivers_urls(conn));
        to.push(PUBLIC_VISIBILTY.to_string());

        let mut comment = Note::default();
        comment.object_props = ObjectProperties {
            id: Some(serde_json::to_value(self.ap_url.clone()).unwrap()),
            summary: Some(serde_json::to_value(self.spoiler_text.clone()).unwrap()),
            content: Some(serde_json::to_value(self.content.clone()).unwrap()),
            in_reply_to: Some(serde_json::to_value(self.in_response_to_id.map_or_else(|| self.get_post(conn).ap_url, |id| {
                let comm = Comment::get(conn, id).unwrap();
                comm.ap_url.clone().unwrap_or(comm.compute_id(conn))
            })).unwrap()),
            published: Some(serde_json::to_value(self.creation_date).unwrap()),
            attributed_to: Some(serde_json::to_value(self.get_author(conn).compute_id(conn)).unwrap()),
            to: Some(serde_json::to_value(to).unwrap()),
            cc: Some(serde_json::to_value(Vec::<serde_json::Value>::new()).unwrap()),
            ..ObjectProperties::default()
        };
        comment
    }

    pub fn create_activity(&self, conn: &PgConnection) -> Create {
        let mut act = Create::default();
        act.create_props.set_actor_link(self.get_author(conn).into_id()).unwrap();
        act.create_props.set_object_object(self.into_activity(conn)).unwrap();
        act.object_props.set_id_string(format!("{}/activity", self.ap_url.clone().unwrap())).unwrap();
        act
    }

    pub fn count_local(conn: &PgConnection) -> usize {
        use schema::users;
        let local_authors = users::table.filter(users::instance_id.eq(Instance::local_id(conn))).select(users::id);
        comments::table.filter(comments::author_id.eq(any(local_authors)))
            .load::<Comment>(conn)
            .expect("Couldn't load local comments")
            .len()
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
