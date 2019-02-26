use activitypub::{activity::Create, Error as ApError, Object};

use activity_pub::Id;

#[derive(Fail, Debug)]
pub enum InboxError {
    #[fail(display = "The `type` property is required, but was not present")]
    NoType,
    #[fail(display = "Invalid activity type")]
    InvalidType,
    #[fail(display = "Couldn't undo activity")]
    CantUndo,
}

pub trait FromActivity<T: Object, C>: Sized {
    type Error: From<ApError>;

    fn from_activity(conn: &C, obj: T, actor: Id) -> Result<Self, Self::Error>;

    fn try_from_activity(conn: &C, act: Create) -> Result<Self, Self::Error> {
        Self::from_activity(
            conn,
            act.create_props.object_object()?,
            act.create_props.actor_link::<Id>()?,
        )
    }
}

pub trait Notify<C> {
    type Error;

    fn notify(&self, conn: &C) -> Result<(), Self::Error>;
}

pub trait Deletable<C, A> {
    type Error;

    fn delete(&self, conn: &C) -> Result<A, Self::Error>;
    fn delete_id(id: &str, actor_id: &str, conn: &C) -> Result<A, Self::Error>;
}

pub trait WithInbox {
    fn get_inbox_url(&self) -> String;

    fn get_shared_inbox_url(&self) -> Option<String>;

    fn is_local(&self) -> bool;
}
