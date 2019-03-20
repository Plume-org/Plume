use activitypub::{
    activity::{Create, Delete},
    link,
    object::{Note, Tombstone},
};
use chrono::{self, NaiveDateTime};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};
use serde_json;

use std::collections::HashSet;

use comment_seers::{CommentSeers, NewCommentSeers};
use instance::Instance;
use mentions::Mention;
use notifications::*;
use plume_common::activity_pub::{
    inbox::{Deletable, FromActivity, Notify},
    Id, IntoId, PUBLIC_VISIBILTY,
};
use plume_common::utils;
use posts::Post;
use safe_string::SafeString;
use schema::comments;
use users::User;
use {Connection, Error, Result};

#[derive(Queryable, Identifiable, Clone, AsChangeset)]
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
    insert!(comments, NewComment, |inserted, conn| {
        if inserted.ap_url.is_none() {
            inserted.ap_url = Some(format!(
                "{}comment/{}",
                inserted.get_post(conn)?.ap_url,
                inserted.id
            ));
            let _: Comment = inserted.save_changes(conn)?;
        }
        Ok(inserted)
    });
    get!(comments);
    list_by!(comments, list_by_post, post_id as i32);
    find_by!(comments, find_by_ap_url, ap_url as &str);

    pub fn get_author(&self, conn: &Connection) -> Result<User> {
        User::get(conn, self.author_id)
    }

    pub fn get_post(&self, conn: &Connection) -> Result<Post> {
        Post::get(conn, self.post_id)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        use schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::get_local(conn)?.id))
            .select(users::id);
        comments::table
            .filter(comments::author_id.eq_any(local_authors))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_responses(&self, conn: &Connection) -> Result<Vec<Comment>> {
        comments::table
            .filter(comments::in_response_to_id.eq(self.id))
            .load::<Comment>(conn)
            .map_err(Error::from)
    }

    pub fn can_see(&self, conn: &Connection, user: Option<&User>) -> bool {
        self.public_visibility
            || user
                .as_ref()
                .map(|u| CommentSeers::can_see(conn, self, u).unwrap_or(false))
                .unwrap_or(false)
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<Note> {
        let (html, mentions, _hashtags) = utils::md_to_html(
            self.content.get().as_ref(),
            &Instance::get_local(conn)?.public_domain,
        );

        let author = User::get(conn, self.author_id)?;
        let mut note = Note::default();
        let to = vec![Id::new(PUBLIC_VISIBILTY.to_string())];

        note.object_props
            .set_id_string(self.ap_url.clone().unwrap_or_default())?;
        note.object_props
            .set_summary_string(self.spoiler_text.clone())?;
        note.object_props.set_content_string(html)?;
        note.object_props
            .set_in_reply_to_link(Id::new(self.in_response_to_id.map_or_else(
                || Ok(Post::get(conn, self.post_id)?.ap_url),
                |id| Ok(Comment::get(conn, id)?.ap_url.unwrap_or_default()) as Result<String>,
            )?))?;
        note.object_props
            .set_published_string(chrono::Utc::now().to_rfc3339())?;
        note.object_props
            .set_attributed_to_link(author.clone().into_id())?;
        note.object_props.set_to_link_vec(to.clone())?;
        note.object_props.set_tag_link_vec(
            mentions
                .into_iter()
                .filter_map(|m| Mention::build_activity(conn, &m).ok())
                .collect::<Vec<link::Mention>>(),
        )?;
        Ok(note)
    }

    pub fn create_activity(&self, conn: &Connection) -> Result<Create> {
        let author = User::get(conn, self.author_id)?;

        let note = self.to_activity(conn)?;
        let mut act = Create::default();
        act.create_props.set_actor_link(author.into_id())?;
        act.create_props.set_object_object(note.clone())?;
        act.object_props
            .set_id_string(format!("{}/activity", self.ap_url.clone()?,))?;
        act.object_props
            .set_to_link_vec(note.object_props.to_link_vec::<Id>()?)?;
        act.object_props.set_cc_link_vec::<Id>(vec![])?;
        Ok(act)
    }
}

impl FromActivity<Note, Connection> for Comment {
    type Error = Error;

    fn from_activity(conn: &Connection, note: Note, actor: Id) -> Result<Comment> {
        let comm = {
            let previous_url = note.object_props.in_reply_to.as_ref()?.as_str()?;
            let previous_comment = Comment::find_by_ap_url(conn, previous_url);

            let is_public = |v: &Option<serde_json::Value>| match v
                .as_ref()
                .unwrap_or(&serde_json::Value::Null)
            {
                serde_json::Value::Array(v) => v
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .any(|s| s == PUBLIC_VISIBILTY),
                serde_json::Value::String(s) => s == PUBLIC_VISIBILTY,
                _ => false,
            };

            let public_visibility = is_public(&note.object_props.to)
                || is_public(&note.object_props.bto)
                || is_public(&note.object_props.cc)
                || is_public(&note.object_props.bcc);

            let comm = Comment::insert(
                conn,
                NewComment {
                    content: SafeString::new(&note.object_props.content_string()?),
                    spoiler_text: note.object_props.summary_string().unwrap_or_default(),
                    ap_url: note.object_props.id_string().ok(),
                    in_response_to_id: previous_comment.iter().map(|c| c.id).next(),
                    post_id: previous_comment.map(|c| c.post_id).or_else(|_| {
                        Ok(Post::find_by_ap_url(conn, previous_url)?.id) as Result<i32>
                    })?,
                    author_id: User::from_url(conn, actor.as_ref())?.id,
                    sensitive: false, // "sensitive" is not a standard property, we need to think about how to support it with the activitypub crate
                    public_visibility,
                },
            )?;

            // save mentions
            if let Some(serde_json::Value::Array(tags)) = note.object_props.tag.clone() {
                for tag in tags {
                    serde_json::from_value::<link::Mention>(tag)
                        .map_err(Error::from)
                        .and_then(|m| {
                            let author = &Post::get(conn, comm.post_id)?.get_authors(conn)?[0];
                            let not_author = m.link_props.href_string()? != author.ap_url.clone();
                            Ok(Mention::from_activity(
                                conn, &m, comm.id, false, not_author,
                            )?)
                        })
                        .ok();
                }
            }
            comm
        };

        if !comm.public_visibility {
            let receivers_ap_url = |v: Option<serde_json::Value>| {
                let filter = |e: serde_json::Value| {
                    if let serde_json::Value::String(s) = e {
                        Some(s)
                    } else {
                        None
                    }
                };
                match v.unwrap_or(serde_json::Value::Null) {
                    serde_json::Value::Array(v) => v,
                    v => vec![v],
                }
                .into_iter()
                .filter_map(filter)
            };

            let mut note = note;

            let to = receivers_ap_url(note.object_props.to.take());
            let cc = receivers_ap_url(note.object_props.cc.take());
            let bto = receivers_ap_url(note.object_props.bto.take());
            let bcc = receivers_ap_url(note.object_props.bcc.take());

            let receivers_ap_url = to
                .chain(cc)
                .chain(bto)
                .chain(bcc)
                .collect::<HashSet<_>>() //remove duplicates (don't do a query more than once)
                .into_iter()
                .map(|v| {
                    if let Ok(user) = User::from_url(conn, &v) {
                        vec![user]
                    } else {
                        vec![] // TODO try to fetch collection
                    }
                })
                .flatten()
                .filter(|u| u.get_instance(conn).map(|i| i.local).unwrap_or(false))
                .collect::<HashSet<User>>(); //remove duplicates (prevent db error)

            for user in &receivers_ap_url {
                CommentSeers::insert(
                    conn,
                    NewCommentSeers {
                        comment_id: comm.id,
                        user_id: user.id,
                    },
                )?;
            }
        }

        comm.notify(conn)?;
        Ok(comm)
    }
}

impl Notify<Connection> for Comment {
    type Error = Error;

    fn notify(&self, conn: &Connection) -> Result<()> {
        for author in self.get_post(conn)?.get_authors(conn)? {
            Notification::insert(
                conn,
                NewNotification {
                    kind: notification_kind::COMMENT.to_string(),
                    object_id: self.id,
                    user_id: author.id,
                },
            )?;
        }
        Ok(())
    }
}

pub struct CommentTree {
    pub comment: Comment,
    pub responses: Vec<CommentTree>,
}

impl CommentTree {
    pub fn from_post(conn: &Connection, p: &Post, user: Option<&User>) -> Result<Vec<Self>> {
        Ok(Comment::list_by_post(conn, p.id)?
            .into_iter()
            .filter(|c| c.in_response_to_id.is_none())
            .filter(|c| c.can_see(conn, user))
            .filter_map(|c| Self::from_comment(conn, c, user).ok())
            .collect())
    }

    pub fn from_comment(conn: &Connection, comment: Comment, user: Option<&User>) -> Result<Self> {
        let responses = comment
            .get_responses(conn)?
            .into_iter()
            .filter(|c| c.can_see(conn, user))
            .filter_map(|c| Self::from_comment(conn, c, user).ok())
            .collect();
        Ok(CommentTree { comment, responses })
    }
}

impl<'a> Deletable<Connection, Delete> for Comment {
    type Error = Error;

    fn delete(&self, conn: &Connection) -> Result<Delete> {
        let mut act = Delete::default();
        act.delete_props
            .set_actor_link(self.get_author(conn)?.into_id())?;

        let mut tombstone = Tombstone::default();
        tombstone.object_props.set_id_string(self.ap_url.clone()?)?;
        act.delete_props.set_object_object(tombstone)?;

        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url.clone().unwrap()))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILTY)])?;

        for m in Mention::list_for_comment(&conn, self.id)? {
            m.delete(conn)?;
        }
        diesel::update(comments::table)
            .filter(comments::in_response_to_id.eq(self.id))
            .set(comments::in_response_to_id.eq(self.in_response_to_id))
            .execute(conn)?;
        diesel::delete(self).execute(conn)?;
        Ok(act)
    }

    fn delete_id(id: &str, actor_id: &str, conn: &Connection) -> Result<Delete> {
        let actor = User::find_by_ap_url(conn, actor_id)?;
        let comment = Comment::find_by_ap_url(conn, id)?;
        if comment.author_id == actor.id {
            comment.delete(conn)
        } else {
            Err(Error::Unauthorized)
        }
    }
}
