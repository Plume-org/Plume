use activitypub::{
    activity::{
        Announce,
        Create,
        Delete,
        Follow as FollowAct,
        Like,
        Undo,
        Update
    },
    object::Tombstone
};
use failure::Error;
use rocket::{
    data::*,
    http::Status,
    Outcome::{self, *},
    Request,
};
use rocket_contrib::json::*;
use serde::Deserialize;
use serde_json;

use std::io::Read;

use plume_common::activity_pub::{
    inbox::{Deletable, FromActivity, InboxError, Notify},
    Id,request::Digest,
};
use plume_models::{
    comments::Comment, follows::Follow, instance::Instance, likes, posts::Post, reshares::Reshare,
    users::User, search::Searcher, Connection,
};

pub trait Inbox {
    fn received(&self, conn: &Connection, searcher: &Searcher, act: serde_json::Value) -> Result<(), Error> {
        let actor_id = Id::new(act["actor"].as_str().unwrap_or_else(|| {
            act["actor"]["id"]
                .as_str()
                .expect("Inbox::received: actor_id missing error")
        }));
        match act["type"].as_str() {
            Some(t) => match t {
                "Announce" => {
                    Reshare::from_activity(conn, serde_json::from_value(act.clone())?, actor_id)
                        .expect("Inbox::received: Announce error");;
                    Ok(())
                }
                "Create" => {
                    let act: Create = serde_json::from_value(act.clone())?;
                    if Post::try_from_activity(&(conn, searcher), act.clone()).is_ok()
                        || Comment::try_from_activity(conn, act).is_ok()
                    {
                        Ok(())
                    } else {
                        Err(InboxError::InvalidType)?
                    }
                }
                "Delete" => {
                    let act: Delete = serde_json::from_value(act.clone())?;
                    Post::delete_id(
                        &act.delete_props
                            .object_object::<Tombstone>()?
                            .object_props
                            .id_string()?,
                        actor_id.as_ref(),
                        &(conn, searcher),
                    ).ok();
                    Comment::delete_id(
                        &act.delete_props
                            .object_object::<Tombstone>()?
                            .object_props
                            .id_string()?,
                        actor_id.as_ref(),
                        conn,
                    ).ok();
                    Ok(())
                }
                "Follow" => {
                    Follow::from_activity(conn, serde_json::from_value(act.clone())?, actor_id)
                        .and_then(|f| f.notify(conn)).expect("Inbox::received: follow from activity error");;
                    Ok(())
                }
                "Like" => {
                    likes::Like::from_activity(
                        conn,
                        serde_json::from_value(act.clone())?,
                        actor_id,
                    ).expect("Inbox::received: like from activity error");;
                    Ok(())
                }
                "Undo" => {
                    let act: Undo = serde_json::from_value(act.clone())?;
                    if let Some(t) = act.undo_props.object["type"].as_str() {
                        match t {
                            "Like" => {
                                likes::Like::delete_id(
                                    &act.undo_props
                                        .object_object::<Like>()?
                                        .object_props
                                        .id_string()?,
                                    actor_id.as_ref(),
                                    conn,
                                ).expect("Inbox::received: undo like fail");;
                                Ok(())
                            }
                            "Announce" => {
                                Reshare::delete_id(
                                    &act.undo_props
                                        .object_object::<Announce>()?
                                        .object_props
                                        .id_string()?,
                                    actor_id.as_ref(),
                                    conn,
                                ).expect("Inbox::received: undo reshare fail");;
                                Ok(())
                            }
                            "Follow" => {
                                Follow::delete_id(
                                    &act.undo_props
                                        .object_object::<FollowAct>()?
                                        .object_props
                                        .id_string()?,
                                    actor_id.as_ref(),
                                    conn,
                                ).expect("Inbox::received: undo follow error");;
                                Ok(())
                            }
                            _ => Err(InboxError::CantUndo)?,
                        }
                    } else {
                        let link = act.undo_props.object.as_str().expect("Inbox::received: undo don't contain type and isn't Link");
                        if let Ok(like) = likes::Like::find_by_ap_url(conn, link) {
                            likes::Like::delete_id(&like.ap_url, actor_id.as_ref(), conn).expect("Inbox::received: delete Like error");
                            Ok(())
                        } else if let Ok(reshare) = Reshare::find_by_ap_url(conn, link) {
                            Reshare::delete_id(&reshare.ap_url, actor_id.as_ref(), conn).expect("Inbox::received: delete Announce error");
                            Ok(())
                        } else if let Ok(follow) = Follow::find_by_ap_url(conn, link) {
                            Follow::delete_id(&follow.ap_url, actor_id.as_ref(), conn).expect("Inbox::received: delete Follow error");
                            Ok(())
                        } else {
                            Err(InboxError::NoType)?
                        }
                    }
                }
                "Update" => {
                    let act: Update = serde_json::from_value(act.clone())?;
                    Post::handle_update(conn, &act.update_props.object_object()?, searcher).expect("Inbox::received: post update error");;
                    Ok(())
                }
                _ => Err(InboxError::InvalidType)?,
            },
            None => Err(InboxError::NoType)?,
        }
    }
}

impl Inbox for Instance {}
impl Inbox for User {}

const JSON_LIMIT: u64 = 1 << 20;

pub struct SignedJson<T>(pub Digest, pub Json<T>);

impl<'a, T: Deserialize<'a>> FromData<'a> for SignedJson<T> {
    type Error = JsonError<'a>;
    type Owned = String;
    type Borrowed = str;

    fn transform(r: &Request, d: Data) -> Transform<Outcome<Self::Owned, (Status, Self::Error), Data>> {
        let size_limit = r.limits().get("json").unwrap_or(JSON_LIMIT);
        let mut s = String::with_capacity(512);
        match d.open().take(size_limit).read_to_string(&mut s) {
            Ok(_) => Transform::Borrowed(Success(s)),
            Err(e) => Transform::Borrowed(Failure((Status::BadRequest, JsonError::Io(e))))
        }
    }

    fn from_data(_: &Request, o: Transformed<'a, Self>) -> Outcome<Self, (Status, Self::Error), Data> {
        let string = o.borrowed()?;
        match serde_json::from_str(&string) {
            Ok(v) => Success(SignedJson(Digest::from_body(&string),Json(v))),
            Err(e) => {
                if e.is_data() {
                    Failure((Status::UnprocessableEntity, JsonError::Parse(string, e)))
                } else {
                    Failure((Status::BadRequest, JsonError::Parse(string, e)))
                }
            }
        }
    }
}
