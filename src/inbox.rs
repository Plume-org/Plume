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
use serde_json;

use plume_common::activity_pub::{
    inbox::{Deletable, FromActivity, InboxError},
    Id,
};
use plume_models::{
    comments::Comment, follows::Follow, instance::Instance, likes, posts::Post, reshares::Reshare,
    users::User, Connection,
};

pub trait Inbox {
    fn received(&self, conn: &Connection, act: serde_json::Value) -> Result<(), Error> {
        let actor_id = Id::new(act["actor"].as_str().unwrap_or_else(|| {
            act["actor"]["id"]
                .as_str()
                .expect("Inbox::received: actor_id missing error")
        }));
        match act["type"].as_str() {
            Some(t) => match t {
                "Announce" => {
                    Reshare::from_activity(conn, serde_json::from_value(act.clone())?, actor_id);
                    Ok(())
                }
                "Create" => {
                    let act: Create = serde_json::from_value(act.clone())?;
                    if Post::try_from_activity(conn, act.clone())
                        || Comment::try_from_activity(conn, act)
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
                        conn,
                    );
                    Ok(())
                }
                "Follow" => {
                    Follow::from_activity(conn, serde_json::from_value(act.clone())?, actor_id);
                    Ok(())
                }
                "Like" => {
                    likes::Like::from_activity(
                        conn,
                        serde_json::from_value(act.clone())?,
                        actor_id,
                    );
                    Ok(())
                }
                "Undo" => {
                    let act: Undo = serde_json::from_value(act.clone())?;
                    match act.undo_props.object["type"]
                        .as_str()
                        .expect("Inbox::received: undo without original type error")
                    {
                        "Like" => {
                            likes::Like::delete_id(
                                &act.undo_props
                                    .object_object::<Like>()?
                                    .object_props
                                    .id_string()?,
                                actor_id.as_ref(),
                                conn,
                            );
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
                            );
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
                            );
                            Ok(())
                        }
                        _ => Err(InboxError::CantUndo)?,
                    }
                }
                "Update" => {
                    let act: Update = serde_json::from_value(act.clone())?;
                    Post::handle_update(conn, &act.update_props.object_object()?);
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
