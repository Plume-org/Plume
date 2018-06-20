use activitypub::{
    activity::Create,
    link,
    object::{Note, properties::ObjectProperties}
};
use chrono;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods, dsl::any};
use serde_json;

use activity_pub::{
    ap_url, Id, IntoId, PUBLIC_VISIBILTY,
    actor::Actor,
    inbox::{FromActivity, Notify}
};
use models::{
    instance::Instance,
    mentions::Mention,
    notifications::*,
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
    insert!(comments, NewComment);
    get!(comments);
    find_by!(comments, find_by_post, post_id as i32);
    find_by!(comments, find_by_ap_url, ap_url as String);

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

    pub fn to_json(&self, conn: &PgConnection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).unwrap();
        json["author"] = self.get_author(conn).to_json(conn);
        json
    }

    pub fn compute_id(&self, conn: &PgConnection) -> String {
        ap_url(format!("{}#comment-{}", self.get_post(conn).compute_id(conn), self.id))
    }
}

impl FromActivity<Note> for Comment {
    fn from_activity(conn: &PgConnection, note: Note, actor: Id) -> Comment {
        let previous_url = note.object_props.in_reply_to.clone().unwrap().as_str().unwrap().to_string();
        let previous_comment = Comment::find_by_ap_url(conn, previous_url.clone());

        // save mentions
        if let Some(serde_json::Value::Array(tags)) = note.object_props.tag.clone() {
            for tag in tags.into_iter() {
                serde_json::from_value::<link::Mention>(tag)
                    .map(|m| Mention::from_activity(conn, m, Id::new(note.clone().object_props.clone().url_string().unwrap_or(String::from("")))))
                    .ok();
            }
        }

        let comm = Comment::insert(conn, NewComment {
            content: SafeString::new(&note.object_props.content_string().unwrap()),
            spoiler_text: note.object_props.summary_string().unwrap_or(String::from("")),
            ap_url: note.object_props.id_string().ok(),
            in_response_to_id: previous_comment.clone().map(|c| c.id),
            post_id: previous_comment
                .map(|c| c.post_id)
                .unwrap_or_else(|| Post::find_by_ap_url(conn, previous_url).unwrap().id),
            author_id: User::from_url(conn, actor.clone().into()).unwrap().id,
            sensitive: false // "sensitive" is not a standard property, we need to think about how to support it with the activitypub crate
        });
        Comment::notify(conn, note, actor);
        comm
    }
}

impl Notify<Note> for Comment {
    fn notify(conn: &PgConnection, note: Note, _actor: Id) {
        match Comment::find_by_ap_url(conn, note.object_props.id_string().unwrap()) {
            Some(comment) => {
                for author in comment.clone().get_post(conn).get_authors(conn) {
                    let comment = comment.clone();
                    Notification::insert(conn, NewNotification {
                        title: "{{ data }} commented your article".to_string(),
                        data: Some(comment.get_author(conn).display_name.clone()),
                        content: Some(comment.get_post(conn).title),
                        link: comment.ap_url,
                        user_id: author.id
                    });
                }
            },
            None => println!("Couldn't find comment by AP id, to create a new notification")
        };
    }
}
