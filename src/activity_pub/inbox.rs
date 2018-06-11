use activitypub::{
    Actor,
    activity::{Accept, Announce, Create, Follow, Like, Undo},
    object::Note
};
use activitystreams_types::object::Article;
use diesel::PgConnection;
use failure::Error;
use serde_json;

use activity_pub::{
    broadcast, Id, IntoId,
    actor::Actor as APActor,
    sign::*
};
use models::{
    blogs::Blog,
    comments::*,
    follows,
    likes,
    posts::*,
    reshares::*,
    users::User
};
use safe_string::SafeString;

#[derive(Fail, Debug)]
enum InboxError {
    #[fail(display = "The `type` property is required, but was not present")]
    NoType,
    #[fail(display = "Invalid activity type")]
    InvalidType,
    #[fail(display = "Couldn't undo activity")]
    CantUndo
}

pub trait Inbox {
    fn received(&self, conn: &PgConnection, act: serde_json::Value);

    fn new_article(&self, conn: &PgConnection, article: Article) -> Result<(), Error> {
        Post::insert(conn, NewPost {
            blog_id: 0, // TODO
            slug: String::from(""), // TODO
            title: article.object_props.name_string().unwrap(),
            content: SafeString::new(&article.object_props.content_string().unwrap()),
            published: true,
            license: String::from("CC-0"),
            ap_url: article.object_props.url_string()?
        });
        Ok(())
    }

    fn new_comment(&self, conn: &PgConnection, note: Note, actor_id: String) -> Result<(), Error> {
        let previous_url = note.object_props.in_reply_to.clone().unwrap().as_str().unwrap().to_string();
        let previous_comment = Comment::find_by_ap_url(conn, previous_url.clone());
        Comment::insert(conn, NewComment {
            content: SafeString::new(&note.object_props.content_string().unwrap()),
            spoiler_text: note.object_props.summary_string().unwrap_or(String::from("")),
            ap_url: note.object_props.id_string().ok(),
            in_response_to_id: previous_comment.clone().map(|c| c.id),
            post_id: previous_comment
                .map(|c| c.post_id)
                .unwrap_or_else(|| Post::find_by_ap_url(conn, previous_url).unwrap().id),
            author_id: User::from_url(conn, actor_id).unwrap().id,
            sensitive: false // "sensitive" is not a standard property, we need to think about how to support it with the activitystreams crate
        });
        Ok(())
    }

    fn follow(&self, conn: &PgConnection, follow: Follow) -> Result<(), Error> {
        let from = User::from_url(conn, follow.follow_props.actor.as_str().unwrap().to_string()).unwrap();
        match User::from_url(conn, follow.follow_props.object.as_str().unwrap().to_string()) {
            Some(u) => self.accept_follow(conn, &from, &u, follow, from.id, u.id),
            None => {
                let blog = Blog::from_url(conn, follow.follow_props.object.as_str().unwrap().to_string()).unwrap();
                self.accept_follow(conn, &from, &blog, follow, from.id, blog.id)
            }
        };
        Ok(())
    }

    fn like(&self, conn: &PgConnection, like: Like) -> Result<(), Error> {
        let liker = User::from_url(conn, like.like_props.actor.as_str().unwrap().to_string());
        let post = Post::find_by_ap_url(conn, like.like_props.object.as_str().unwrap().to_string());
        likes::Like::insert(conn, likes::NewLike {
            post_id: post.unwrap().id,
            user_id: liker.unwrap().id,
            ap_url: like.object_props.id_string()?
        });
        Ok(())
    }

    fn unlike(&self, conn: &PgConnection, undo: Undo) -> Result<(), Error> {
        let like = likes::Like::find_by_ap_url(conn, undo.undo_props.object_object::<Like>()?.object_props.id_string()?).unwrap();
        like.delete(conn);
        Ok(())
    }

    fn announce(&self, conn: &PgConnection, announce: Announce) -> Result<(), Error> {
        let user = User::from_url(conn, announce.announce_props.actor.as_str().unwrap().to_string());
        let post = Post::find_by_ap_url(conn, announce.announce_props.object.as_str().unwrap().to_string());
        Reshare::insert(conn, NewReshare {
            post_id: post.unwrap().id,
            user_id: user.unwrap().id,
            ap_url: announce.object_props.id_string()?
        });
        Ok(())
    }

    fn save(&self, conn: &PgConnection, act: serde_json::Value) -> Result<(), Error> {
        match act["type"].as_str() {
            Some(t) => {
                match t {
                    "Announce" => self.announce(conn, serde_json::from_value(act.clone())?),
                    "Create" => {
                        let act: Create = serde_json::from_value(act.clone())?;
                        match act.create_props.object["type"].as_str().unwrap() {
                            "Article" => self.new_article(conn, act.create_props.object_object()?),
                            "Note" => self.new_comment(conn, act.create_props.object_object()?, act.create_props.actor_link::<Id>()?.0),
                            _ => Err(InboxError::InvalidType)?
                        }
                    },
                    "Follow" => self.follow(conn, serde_json::from_value(act.clone())?),
                    "Like" => self.like(conn, serde_json::from_value(act.clone())?),
                    "Undo" => {
                        let act: Undo = serde_json::from_value(act.clone())?;
                        match act.undo_props.object["type"].as_str().unwrap() {
                            "Like" => self.unlike(conn, act),
                            _ => Err(InboxError::CantUndo)?
                        }
                    }
                    _ => Err(InboxError::InvalidType)?
                }
            },
            None => Err(InboxError::NoType)?
        }
    }

    fn accept_follow<A: Signer + IntoId + Clone, B: Clone + WithInbox + Actor>(
        &self,
        conn: &PgConnection,
        from: &A,
        target: &B,
        follow: Follow,
        from_id: i32,
        target_id: i32
    ) {
        follows::Follow::insert(conn, follows::NewFollow {
            follower_id: from_id,
            following_id: target_id
        });

        let mut accept = Accept::default();
        accept.accept_props.set_actor_link::<Id>(from.clone().into_id()).unwrap();
        accept.accept_props.set_object_object(follow).unwrap();
        broadcast(conn, &*from, accept, vec![target.clone()]);
    }
}

pub trait WithInbox {
    fn get_inbox_url(&self) -> String;

    fn get_shared_inbox_url(&self) -> Option<String>;
}
