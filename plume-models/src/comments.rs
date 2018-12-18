use activitypub::{activity::Create, link, object::Note};
use chrono::{self, NaiveDateTime};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use serde_json;

use std::collections::HashSet;

use instance::Instance;
use mentions::Mention;
use notifications::*;
use plume_common::activity_pub::{
    inbox::{FromActivity, Notify},
    Id, IntoId, PUBLIC_VISIBILTY,
};
use plume_common::utils;
use comment_seers::{CommentSeers, NewCommentSeers};
use posts::Post;
use safe_string::SafeString;
use schema::comments;
use users::User;
use Connection;

#[derive(Queryable, Identifiable, Serialize, Clone)]
pub struct Comment {
    pub id: i32,
    pub content: SafeString,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String,
    pub public_visibility: bool,
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
    pub spoiler_text: String,
    pub public_visibility: bool,
}

impl Comment {
    insert!(comments, NewComment);
    get!(comments);
    list_by!(comments, list_by_post, post_id as i32);
    find_by!(comments, find_by_ap_url, ap_url as &str);

    pub fn get_author(&self, conn: &Connection) -> User {
        User::get(conn, self.author_id).expect("Comment::get_author: author error")
    }

    pub fn get_post(&self, conn: &Connection) -> Post {
        Post::get(conn, self.post_id).expect("Comment::get_post: post error")
    }

    pub fn count_local(conn: &Connection) -> i64 {
        use schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::local_id(conn)))
            .select(users::id);
        comments::table
            .filter(comments::author_id.eq_any(local_authors))
            .count()
            .get_result(conn)
            .expect("Comment::count_local: loading error")
    }

    pub fn get_responses(&self, conn: &Connection) -> Vec<Comment> {
        comments::table.filter(comments::in_response_to_id.eq(self.id))
            .load::<Comment>(conn)
            .expect("Comment::get_responses: loading error")
    }

    pub fn update_ap_url(&self, conn: &Connection) -> Comment {
        if self.ap_url.is_none() {
            diesel::update(self)
                .set(comments::ap_url.eq(self.compute_id(conn)))
                .execute(conn)
                .expect("Comment::update_ap_url: update error");
            Comment::get(conn, self.id).expect("Comment::update_ap_url: get error")
        } else {
            self.clone()
        }
    }

    pub fn compute_id(&self, conn: &Connection) -> String {
        format!("{}comment/{}", self.get_post(conn).ap_url, self.id)
    }

    pub fn can_see(&self, conn: &Connection, user: &User) -> bool {
        self.public_visibility || CommentSeers::can_see(conn, self, user)
    }


    pub fn to_activity(&self, conn: &Connection) -> Note {
        let (html, mentions, _hashtags) = utils::md_to_html(self.content.get().as_ref());

        let author = User::get(conn, self.author_id).expect("Comment::to_activity: author error");
        let mut note = Note::default();
        let to = vec![Id::new(PUBLIC_VISIBILTY.to_string())];

        note.object_props
            .set_id_string(self.ap_url.clone().unwrap_or_default())
            .expect("Comment::to_activity: id error");
        note.object_props
            .set_summary_string(self.spoiler_text.clone())
            .expect("Comment::to_activity: summary error");
        note.object_props
            .set_content_string(html)
            .expect("Comment::to_activity: content error");
        note.object_props
            .set_in_reply_to_link(Id::new(self.in_response_to_id.map_or_else(
                || {
                    Post::get(conn, self.post_id)
                        .expect("Comment::to_activity: post error")
                        .ap_url
                },
                |id| {
                    let comm =
                        Comment::get(conn, id).expect("Comment::to_activity: comment error");
                    comm.ap_url.clone().unwrap_or_else(|| comm.compute_id(conn))
                },
            )))
            .expect("Comment::to_activity: in_reply_to error");
        note.object_props
            .set_published_string(chrono::Utc::now().to_rfc3339())
            .expect("Comment::to_activity: published error");
        note.object_props
            .set_attributed_to_link(author.clone().into_id())
            .expect("Comment::to_activity: attributed_to error");
        note.object_props
            .set_to_link_vec(to.clone())
            .expect("Comment::to_activity: to error");
        note.object_props
            .set_tag_link_vec(
                mentions
                    .into_iter()
                    .map(|m| Mention::build_activity(conn, &m))
                    .collect::<Vec<link::Mention>>(),
            )
            .expect("Comment::to_activity: tag error");
        note
    }

    pub fn create_activity(&self, conn: &Connection) -> Create {
        let author =
            User::get(conn, self.author_id).expect("Comment::create_activity: author error");

        let note = self.to_activity(conn);
        let mut act = Create::default();
        act.create_props
            .set_actor_link(author.into_id())
            .expect("Comment::create_activity: actor error");
        act.create_props
            .set_object_object(note.clone())
            .expect("Comment::create_activity: object error");
        act.object_props
            .set_id_string(format!(
                "{}/activity",
                self.ap_url
                    .clone()
                    .expect("Comment::create_activity: ap_url error")
            ))
            .expect("Comment::create_activity: id error");
        act.object_props
            .set_to_link_vec(
                note.object_props
                    .to_link_vec::<Id>()
                    .expect("Comment::create_activity: id error"),
            )
            .expect("Comment::create_activity: to error");
        act.object_props
            .set_cc_link_vec::<Id>(vec![])
            .expect("Comment::create_activity: cc error");
        act
    }
}

impl FromActivity<Note, Connection> for Comment {
    fn from_activity(conn: &Connection, note: Note, actor: Id) -> Comment {
        let comm = {
            let previous_url = note
                .object_props
                .in_reply_to
                .as_ref()
                .expect("Comment::from_activity: not an answer error")
                .as_str()
                .expect("Comment::from_activity: in_reply_to parsing error");
            let previous_comment = Comment::find_by_ap_url(conn, previous_url);

            let is_public = |v: &Option<serde_json::Value>| match v.as_ref().unwrap_or(&serde_json::Value::Null) {
                serde_json::Value::Array(v) => v.iter().filter_map(serde_json::Value::as_str).any(|s| s==PUBLIC_VISIBILTY),
                serde_json::Value::String(s) => s == PUBLIC_VISIBILTY,
                _ => false,
            };

            let public_visibility = is_public(&note.object_props.to) ||
                is_public(&note.object_props.bto) ||
                is_public(&note.object_props.cc) ||
                is_public(&note.object_props.bcc);

            let comm = Comment::insert(
                conn,
                NewComment {
                    content: SafeString::new(
                        &note
                            .object_props
                            .content_string()
                            .expect("Comment::from_activity: content deserialization error"),
                    ),
                    spoiler_text: note
                        .object_props
                        .summary_string()
                        .unwrap_or_default(),
                    ap_url: note.object_props.id_string().ok(),
                    in_response_to_id: previous_comment.clone().map(|c| c.id),
                    post_id: previous_comment.map(|c| c.post_id).unwrap_or_else(|| {
                        Post::find_by_ap_url(conn, previous_url)
                            .expect("Comment::from_activity: post error")
                            .id
                    }),
                    author_id: User::from_url(conn, actor.as_ref())
                        .expect("Comment::from_activity: author error")
                        .id,
                    sensitive: false, // "sensitive" is not a standard property, we need to think about how to support it with the activitypub crate
                    public_visibility
                },
            );

            // save mentions
            if let Some(serde_json::Value::Array(tags)) = note.object_props.tag.clone() {
                for tag in tags {
                    serde_json::from_value::<link::Mention>(tag)
                        .map(|m| {
                            let author = &Post::get(conn, comm.post_id)
                                .expect("Comment::from_activity: error")
                                .get_authors(conn)[0];
                            let not_author = m
                                .link_props
                                .href_string()
                                .expect("Comment::from_activity: no href error")
                                != author.ap_url.clone();
                            Mention::from_activity(conn, &m, comm.id, false, not_author)
                        })
                        .ok();
                }
            }
            comm
        };


        if !comm.public_visibility {
            let receivers_ap_url = |v: Option<serde_json::Value>| {
                let filter = |e: serde_json::Value| if let serde_json::Value::String(s) = e { Some(s) } else { None };
                match v.unwrap_or(serde_json::Value::Null) {
                    serde_json::Value::Array(v) => v,
                    v => vec![v],
                }.into_iter().filter_map(filter)
            };

            let mut note = note;

            let to = receivers_ap_url(note.object_props.to.take());
            let cc = receivers_ap_url(note.object_props.cc.take());
            let bto = receivers_ap_url(note.object_props.bto.take());
            let bcc = receivers_ap_url(note.object_props.bcc.take());

            let receivers_ap_url = to.chain(cc).chain(bto).chain(bcc)
                .collect::<HashSet<_>>()//remove duplicates (don't do a query more than once)
                .into_iter()
                .map(|v| if let Some(user) = User::from_url(conn,&v) {
                    vec![user]
                } else {
                    vec![]// TODO try to fetch collection
                })
                .flatten()
                .filter(|u| u.get_instance(conn).local)
                .collect::<HashSet<User>>();//remove duplicates (prevent db error)

            for user in &receivers_ap_url {
                CommentSeers::insert(
                    conn,
                    NewCommentSeers {
                        comment_id: comm.id,
                        user_id: user.id
                    }
                );
            }
        }

        comm.notify(conn);
        comm
    }
}

impl Notify<Connection> for Comment {
    fn notify(&self, conn: &Connection) {
        for author in self.get_post(conn).get_authors(conn) {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::COMMENT.to_string(),
                    object_id: self.id,
                    user_id: author.id,
                },
            );
        }
    }
}
