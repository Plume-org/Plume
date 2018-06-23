use activitypub::{Object, activity::Create};

use activity_pub::Id;

#[derive(Fail, Debug)]
pub enum InboxError {
    #[fail(display = "The `type` property is required, but was not present")]
    NoType,
    #[fail(display = "Invalid activity type")]
    InvalidType,
    #[fail(display = "Couldn't undo activity")]
    CantUndo
}

pub trait FromActivity<T: Object, C>: Sized {
    fn from_activity(conn: &C, obj: T, actor: Id) -> Self;

    fn try_from_activity(conn: &C, act: Create) -> bool {
        if let Ok(obj) = act.create_props.object_object() {
            Self::from_activity(conn, obj, act.create_props.actor_link::<Id>().unwrap());
            true
        } else {
            false
        }
    }
}

pub trait Notify<C> {
    fn notify(&self, conn: &C);
}

pub trait Deletable<C> {
    /// true if success
    fn delete_activity(conn: &C, id: Id) -> bool;
}

pub trait WithInbox {
    fn get_inbox_url(&self) -> String;

    fn get_shared_inbox_url(&self) -> Option<String>;
}
