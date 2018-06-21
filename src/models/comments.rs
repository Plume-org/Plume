use activitypub::{
    activity::Create,
    link,
    object::{Note}
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
    get_next_id,
    instance::Instance,
    mentions::Mention,
    notifications::*,
    posts::Post,
    users::User
};
use schema::comments;
use safe_string::SafeString;
use utils;

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

#[derive(Insertable, Default)]
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
    list_by!(comments, list_by_post, post_id as i32);
    find_by!(comments, find_by_ap_url, ap_url as String);

    pub fn get_author(&self, conn: &PgConnection) -> User {
        User::get(conn, self.author_id).unwrap()
    }

    pub fn get_post(&self, conn: &PgConnection) -> Post {
        Post::get(conn, self.post_id).unwrap()        
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
        let mentions = Mention::list_for_comment(conn, self.id).into_iter()
            .map(|m| m.get_mentioned(conn).map(|u| u.get_fqn(conn)).unwrap_or(String::new()))
            .collect::<Vec<String>>();
        println!("{:?}", mentions);
        json["mentions"] = serde_json::to_value(mentions).unwrap();
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

        // save mentions
        if let Some(serde_json::Value::Array(tags)) = note.object_props.tag.clone() {
            for tag in tags.into_iter() {
                serde_json::from_value::<link::Mention>(tag)
                    .map(|m| Mention::from_activity(conn, m, comm.id, false))
                    .ok();
            }
        }

        comm.notify(conn);
        comm
    }
}

impl Notify for Comment {
    fn notify(&self, conn: &PgConnection) {
        for author in self.get_post(conn).get_authors(conn) {
            Notification::insert(conn, NewNotification {
                title: "{{ data }} commented your article".to_string(),
                data: Some(self.get_author(conn).display_name.clone()),
                content: Some(self.get_post(conn).title),
                link: self.ap_url.clone(),
                user_id: author.id
            });
        }
    }
}

impl NewComment {
    pub fn build() -> Self {
        NewComment::default()
    }

    pub fn content<T: AsRef<str>>(mut self, val: T) -> Self {
        self.content = SafeString::new(val.as_ref());
        self
    }

    pub fn in_response_to_id(mut self, val: Option<i32>) -> Self {
        self.in_response_to_id = val;
        self
    }

    pub fn post(mut self, post: Post) -> Self {
        self.post_id = post.id;
        self
    }

    pub fn author(mut self, author: User) -> Self {
        self.author_id = author.id;
        self
    }

    pub fn create(mut self, conn: &PgConnection) -> (Create, i32) {
        let post = Post::get(conn, self.post_id).unwrap();
        // We have to manually compute it since the new comment haven't been inserted yet, and it needs the activity we are building to be created
        let next_id = get_next_id(conn, "comments_id_seq");
        self.ap_url = Some(format!("{}#comment-{}", post.ap_url, next_id));
        self.sensitive = false;
        self.spoiler_text = String::new();

        let (html, mentions) = utils::md_to_html(self.content.get().as_ref());

        let author = User::get(conn, self.author_id).unwrap();
        let mut note = Note::default();
        let mut to = author.get_followers(conn).into_iter().map(User::into_id).collect::<Vec<Id>>();
        to.append(&mut post
            .get_authors(conn)
            .into_iter()
            .flat_map(|a| a.get_followers(conn))
            .map(User::into_id)
            .collect::<Vec<Id>>());
        to.push(Id::new(PUBLIC_VISIBILTY.to_string()));

        note.object_props.set_id_string(self.ap_url.clone().unwrap_or(String::new())).expect("NewComment::create: note.id error");
        note.object_props.set_summary_string(self.spoiler_text.clone()).expect("NewComment::create: note.summary error");
        note.object_props.set_content_string(html).expect("NewComment::create: note.content error");
        note.object_props.set_in_reply_to_link(Id::new(self.in_response_to_id.map_or_else(|| Post::get(conn, self.post_id).unwrap().ap_url, |id| {
            let comm = Comment::get(conn, id).unwrap();
            comm.ap_url.clone().unwrap_or(comm.compute_id(conn))
        }))).expect("NewComment::create: note.in_reply_to error");
        note.object_props.set_published_string(chrono::Utc::now().to_rfc3339()).expect("NewComment::create: note.published error");
        note.object_props.set_attributed_to_link(author.clone().into_id()).expect("NewComment::create: note.attributed_to error");
        note.object_props.set_to_link_vec(to).expect("NewComment::create: note.to error");
        note.object_props.set_tag_link_vec(mentions.into_iter().map(|m| Mention::build_activity(conn, m)).collect::<Vec<link::Mention>>())
            .expect("NewComment::create: note.tag error");

        let mut act = Create::default();
        act.create_props.set_actor_link(author.into_id()).expect("NewComment::create: actor error");
        act.create_props.set_object_object(note).expect("NewComment::create: object error");
        act.object_props.set_id_string(format!("{}/activity", self.ap_url.clone().unwrap())).expect("NewComment::create: id error");
        (act, next_id)
    }
}
