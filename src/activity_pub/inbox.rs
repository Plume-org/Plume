use activitypub::{
    Object,
    activity::{Announce, Create, Like, Undo}
};
use diesel::PgConnection;
use failure::Error;
use serde_json;

use activity_pub::{
    Id
};
use models::{
    comments::*,
    follows::Follow,
    likes,
    posts::*,
    reshares::*
};

#[derive(Fail, Debug)]
enum InboxError {
    #[fail(display = "The `type` property is required, but was not present")]
    NoType,
    #[fail(display = "Invalid activity type")]
    InvalidType,
    #[fail(display = "Couldn't undo activity")]
    CantUndo
}

pub trait FromActivity<T: Object>: Sized {
    fn from_activity(conn: &PgConnection, obj: T, actor: Id) -> Self;

    fn try_from_activity(conn: &PgConnection, act: Create) -> bool {
        if let Ok(obj) = act.create_props.object_object() {
            Self::from_activity(conn, obj, act.create_props.actor_link::<Id>().unwrap());
            true
        } else {
            false
        }
    }
}

pub trait Notify {
    fn notify(&self, conn: &PgConnection);
}

pub trait Deletable {
    /// true if success
    fn delete_activity(conn: &PgConnection, id: Id) -> bool;
}

pub trait Inbox {
    fn received(&self, conn: &PgConnection, act: serde_json::Value);

    fn unlike(&self, conn: &PgConnection, undo: Undo) -> Result<(), Error> {
        let like = likes::Like::find_by_ap_url(conn, undo.undo_props.object_object::<Like>()?.object_props.id_string()?).unwrap();
        like.delete(conn);
        Ok(())
    }

    fn save(&self, conn: &PgConnection, act: serde_json::Value) -> Result<(), Error> {
        let actor_id = Id::new(act["actor"].as_str().unwrap());
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

pub trait WithInbox {
    fn get_inbox_url(&self) -> String;

    fn get_shared_inbox_url(&self) -> Option<String>;
}
