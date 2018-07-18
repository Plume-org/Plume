use activitypub::activity::{Announce, Create, Like, Undo};
use diesel::PgConnection;
use failure::Error;
use serde_json;

use plume_common::activity_pub::{Id, inbox::{Deletable, FromActivity, InboxError}};
use plume_models::{
    comments::Comment,
    follows::Follow,
    instance::Instance,
    likes,
    reshares::Reshare,
    posts::Post,
    users::User
};

pub trait Inbox {
    fn received(&self, conn: &PgConnection, act: serde_json::Value) -> Result<(), Error> {
        let actor_id = Id::new(act["actor"].as_str().unwrap_or_else(|| act["actor"]["id"].as_str().expect("No actor ID for incoming activity")));
        match act["type"].as_str() {
            Some(t) => {
                match t {
                    "Announce" => {
                        Reshare::from_activity(conn, serde_json::from_value(act.clone())?, actor_id);
                        Ok(())
                    },
                    "Create" => {
                        let act: Create = serde_json::from_value(act.clone())?;
                        if Post::try_from_activity(conn, act.clone()) || Comment::try_from_activity(conn, act) {
                            Ok(())
                        } else {
                            Err(InboxError::InvalidType)?
                        }
                    },
                    "Follow" => {
                        Follow::from_activity(conn, serde_json::from_value(act.clone())?, actor_id);
                        Ok(())
                    },
                    "Like" => {
                        likes::Like::from_activity(conn, serde_json::from_value(act.clone())?, actor_id);
                        Ok(())
                    },
                    "Undo" => {
                        let act: Undo = serde_json::from_value(act.clone())?;
                        match act.undo_props.object["type"].as_str().unwrap() {
                            "Like" => {
                                likes::Like::delete_activity(conn, Id::new(act.undo_props.object_object::<Like>()?.object_props.id_string()?));
                                Ok(())
                            },
                            "Announce" => {
                                Reshare::delete_activity(conn, Id::new(act.undo_props.object_object::<Announce>()?.object_props.id_string()?));
                                Ok(())
                            }
                            _ => Err(InboxError::CantUndo)?
                        }
                    }
                    _ => Err(InboxError::InvalidType)?
                }
            },
            None => Err(InboxError::NoType)?
        }
    }
}

impl Inbox for Instance {}
impl Inbox for User {}
