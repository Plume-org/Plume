/// Represents an ActivityPub inbox.
///
/// It routes an incoming Activity through the registered handlers.
///
/// # Example
///
/// ```rust
/// # extern crate activitypub;
/// # use activitypub::{actor::Person, activity::{Announce, Create}, object::Note};
/// # use plume_common::activity_pub::inbox::*;
/// # struct User;
/// # impl AsActor<&()> for User {
/// #    type Error = ();
/// #    fn get_or_fetch<S>(_: &(), _id: S) -> Result<Self, Self::Error> where S: AsRef<str> {
/// #        Ok(User)
/// #    }
/// #    fn get_inbox_url(&self) -> String {
/// #        String::new()
/// #    }
/// #    fn is_local(&self) -> bool { false }
/// # }
/// # struct Message;
/// # impl AsObject<User, Create, Note, &()> for Message {
/// #     type Error = ();
/// #     type Output = ();
/// #
/// #     fn activity(_: &(), _actor: User, _obj: Note, _id: &str) -> Result<(), ()> {
/// #         Ok(())
/// #     }
/// # }
/// # impl AsObject<User, Announce, Note, &()> for Message {
/// #     type Error = ();
/// #     type Output = ();
/// #
/// #     fn activity(_: &(), _actor: User, _obj: Note, _id: &str) -> Result<(), ()> {
/// #         Ok(())
/// #     }
/// # }
/// #
/// # let mut act = Create::default();
/// # act.object_props.set_id_string(String::from("https://test.ap/activity")).unwrap();
/// # let mut person = Person::default();
/// # person.object_props.set_id_string(String::from("https://test.ap/actor")).unwrap();
/// # act.create_props.set_actor_object(person).unwrap();
/// # act.create_props.set_object_object(Note::default()).unwrap();
/// # let activity_json = serde_json::to_value(act).unwrap();
/// #
/// # let conn = ();
/// #
/// let result: Result<(), ()> = Inbox::handle(&conn, activity_json)
///    .with::<User, Announce, Message, _>()
///    .with::<User, Create,   Message, _>()
///    .done();
/// ```
pub enum Inbox<'a, C, E, R> where E: From<InboxError> {
    /// The activity has not been handled yet
    ///
    /// # Structure
    ///
    /// - the context to be passed to each handler.
    /// - the activity
    /// - the reason it has not been handled yet
    NotHandled(&'a C, serde_json::Value, InboxError),

    /// A matching handler have been found but failed
    ///
    /// The wrapped value is the error returned by the handler
    Failed(E),

    /// The activity was successfully handled
    ///
    /// The wrapped value is the value returned by the handler
    Handled(R),
}

/// Possible reasons of inbox failure
#[derive(Debug)]
pub enum InboxError {
    /// None of the registered handlers matched
    NoMatch,

    /// The activity type matched for at least one handler, but then the actor was
    /// not of the expected type
    InvalidActor,

    /// Activity and Actor types matched, but not the Object
    InvalidObject,

    /// Error while dereferencing the object
    DerefError,
}

impl From<InboxError> for () {
    fn from(_: InboxError) {
        ()
    }
}

/*
 Type arguments:
 - C: Context
 - E: Error
 - R: Result
*/
impl<'a, C, E, R> Inbox<'a, C, E, R> where E: From<InboxError> {

    /// Creates a new `Inbox` to handle an incoming activity.
    ///
    /// # Parameters
    ///
    /// - `ctx`: the context to pass to each handler
    /// - `json`: the JSON representation of the incoming activity
    pub fn handle(ctx: &'a C, json: serde_json::Value) -> Inbox<'a, C, E, R> {
        Inbox::NotHandled(ctx, json, InboxError::NoMatch)
    }

    /// Registers an handler on this Inbox.
    pub fn with<A, V, M, O>(self) -> Inbox<'a, C, E, R> where
        A: AsActor<&'a C, Error=E>,
        V: activitypub::Activity,
        M: AsObject<A, V, O, &'a C, Error=E>,
        M::Output: Into<R>,
        O: activitypub::Object,
    {
        match self {
            Inbox::NotHandled(ctx, act, e) => {
                if serde_json::from_value::<V>(act.clone()).is_ok() {
                    let actor_id = match get_id(act["actor"].clone()) {
                        Some(x) => x,
                        None => return Inbox::NotHandled(ctx, act, InboxError::InvalidActor),
                    };
                    let actor = match A::get_or_fetch(ctx, actor_id) {
                        Ok(a) => a,
                        // If the actor was not found, go to the next handler
                        Err(_) => return Inbox::NotHandled(ctx, act, InboxError::InvalidActor),
                    };

                    let act_id = match get_id(act["object"].clone()) {
                        Some(x) => x,
                        None => return Inbox::NotHandled(ctx, act, InboxError::InvalidObject),
                    };
                    let obj: O = match serde_json::from_value(act["object"].clone()) {
                        Ok(o) => o,
                        // If the object was not of the expected type, try to dereference it
                        // and if it is still not valid, go to the next handler
                        Err(_) => match reqwest::get(&act_id)
                            .map_err(|_| InboxError::DerefError)
                            .and_then(|mut r| r.json().map_err(|_| InboxError::InvalidObject))
                        {
                            Ok(o) => o,
                            Err(err) => return Inbox::NotHandled(ctx, act, err),
                        }
                    };

                    match M::activity(ctx, actor, obj, &act_id) {
                        Ok(res) => Inbox::Handled(res.into()),
                        Err(e) => Inbox::Failed(e)
                    }
                } else {
                    // If the Activity type is not matching the expected one for
                    // this handler, try with the next one.
                    Inbox::NotHandled(ctx, act, e)
                }
            },
            other => other
        }
    }

    /// Transforms the inbox in a `Result`
    pub fn done(self) -> Result<R, E> {
        match self {
            Inbox::Handled(res) => Ok(res),
            Inbox::NotHandled(_, _, err) => Err(E::from(err)),
            Inbox::Failed(err) => Err(err),
        }
    }
}

/// Get the ActivityPub ID of a JSON value.
///
/// If the value is a string, its value is returned.
/// Otherwise we return its `id` field, if it is a string
///
/// # Panics
///
/// This function panics if the value is neither a string nor an object with an
/// `id` field that is a string.
fn get_id<'a>(json: serde_json::Value) -> Option<String> {
    match json {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Object(map) => map.get("id")?.as_str().map(ToString::to_string),
        _ => None,
    }
}

/// Should be implemented by anything representing an ActivityPub actor.
///
/// # Type arguments
///
/// - `C`: the context to be passed to this activity handler from the `Inbox` (usually a database connection)
pub trait AsActor<C>: Sized {

    /// What kind of error is returned when something fails
    type Error;

    /// Should return the actor with the given ID.
    ///
    /// This actor should be fetched if not present in DB.
    fn get_or_fetch<S>(conn: C, id: S) -> Result<Self, Self::Error> where S: AsRef<str>;

    /// Return the URL of this actor's inbox
    fn get_inbox_url(&self) -> String;

    /// If this actor has shared inbox, its URL should be returned by this function
    fn get_shared_inbox_url(&self) -> Option<String> {
        None
    }

    /// `true` if this actor comes from the running ActivityPub server/instance
    fn is_local(&self) -> bool;
}

/// Should be implemented by anything representing an ActivityPub object.
///
/// # Type parameters
///
/// - `A`: the actor type
/// - `V`: the ActivityPub verb/activity
/// - `O`: the ActivityPub type of the Object for this activity (usually the type corresponding to `Self`)
/// - `C`: the context needed to handle the activity (usually a database connection)
///
/// # Example
///
/// An implementation of AsObject that handles Note creation by an Account model,
/// representing the Note by a Message type, without any specific context.
///
/// ```rust
/// # extern crate activitypub;
/// # use activitypub::{activity::Create, object::Note};
/// # use plume_common::activity_pub::inbox::{AsActor, AsObject};
/// # struct Account;
/// # impl AsActor<()> for Account {
/// #    type Error = ();
/// #    fn get_or_fetch<S>(_: (), _id: S) -> Result<Self, Self::Error> where S: AsRef<str> {
/// #        Ok(Account)
/// #    }
/// #    fn get_inbox_url(&self) -> String {
/// #        String::new()
/// #    }
/// #    fn is_local(&self) -> bool { false }
/// # }
/// #[derive(Debug)]
/// struct Message {
///     text: String,
/// }
///
/// impl AsObject<Account, Create, Note, ()> for Message {
///     type Error = ();
///     type Output = ();
///
///     fn activity(_: (), _actor: Account, obj: Note, _id: &str) -> Result<(), ()> {
///         let msg = Message {
///             text: obj.object_props.content_string().map_err(|_| ())?,
///         };
///         println!("New Note: {:?}", msg);
///         Ok(())
///     }
/// }
/// ```
pub trait AsObject<A, V, O, C>: Sized where
    V: activitypub::Activity,
    O: activitypub::Object,
{

    /// What kind of error is returned when something fails
    type Error;

    /// What is returned by `AsObject::activity`, if anything is returned
    type Output = ();

    /// Handle a specific type of activity dealing with this type of objects.
    ///
    /// The implementations should check that the actor is actually authorized
    /// to perform this action.
    ///
    /// # Parameters
    ///
    /// - `ctx`: the context passed to `Inbox::handle`
    /// - `actor`: the actor who did this activity
    /// - `obj`: the object of this activity
    /// - `id`: the ID of this activity
    fn activity(ctx: C, actor: A, obj: O, id: &str) -> Result<Self::Output, Self::Error>;
}

#[cfg(test)]
mod tests {
    use activitypub::{
        activity::*,
        actor::Person,
        object::Note,
    };
    use super::*;

    struct MyActor;
    impl AsActor<&()> for MyActor {
        type Error = ();

        fn get_or_fetch<S>(_: &(), _id: S) -> Result<Self, Self::Error> where S: AsRef<str> {
            Ok(MyActor)
        }

        fn get_inbox_url(&self) -> String {
            String::from("https://test.ap/my-actor/inbox")
        }

        fn is_local(&self) -> bool {
            false
        }
    }

    struct MyObject;
    impl AsObject<MyActor, Create, Note, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(_: &(), _actor: MyActor, _obj: Note, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is creating a Note");
            Ok(())
        }
    }

    impl AsObject<MyActor, Like, Note, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(_: &(), _actor: MyActor, _obj: Note, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is liking a Note");
            Ok(())
        }
    }

    impl AsObject<MyActor, Delete, Note, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(_: &(), _actor: MyActor, _obj: Note, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is deleting a Note");
            Ok(())
        }
    }

    impl AsObject<MyActor, Announce, Note, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(_: &(), _actor: MyActor, _obj: Note, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is announcing a Note");
            Ok(())
        }
    }

    fn build_create() -> Create {
        let mut act = Create::default();
        act.object_props.set_id_string(String::from("https://test.ap/activity")).unwrap();
        let mut person = Person::default();
        person.object_props.set_id_string(String::from("https://test.ap/actor")).unwrap();
        act.create_props.set_actor_object(person).unwrap();
        let mut note = Note::default();
        note.object_props.set_id_string(String::from("https://test.ap/note")).unwrap();
        act.create_props.set_object_object(note).unwrap();
        act
    }

    #[test]
    fn test_inbox_basic() {
        let act = serde_json::to_value(build_create()).unwrap();
        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<MyActor, Create, MyObject, _>()
            .done();
        assert!(res.is_ok());
    }

    #[test]
    fn test_inbox_multi_handlers() {
        let act = serde_json::to_value(build_create()).unwrap();
        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<MyActor, Announce, MyObject, _>()
            .with::<MyActor, Delete,   MyObject, _>()
            .with::<MyActor, Create,   MyObject, _>()
            .with::<MyActor, Like,     MyObject, _>()
            .done();
        assert!(res.is_ok());
    }

    #[test]
    fn test_inbox_failure() {
        let act = serde_json::to_value(build_create()).unwrap();
        // Create is not handled by this inbox
        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<MyActor, Announce, MyObject, _>()
            .with::<MyActor, Like,     MyObject, _>()
            .done();
        assert!(res.is_err());
    }

    struct FailingActor;
    impl AsActor<&()> for FailingActor {
        type Error = ();

        fn get_or_fetch<S>(_: &(), _id: S) -> Result<Self, Self::Error> where S: AsRef<str> {
            Err(())
        }

        fn get_inbox_url(&self) -> String {
            String::from("https://test.ap/failing-actor/inbox")
        }

        fn is_local(&self) -> bool {
            false
        }
    }

    impl AsObject<FailingActor, Create, Note, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(_: &(), _actor: FailingActor, _obj: Note, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("FailingActor is creating a Note");
            Ok(())
        }
    }

    #[test]
    fn test_inbox_actor_failure() {
        let act = serde_json::to_value(build_create()).unwrap();

        let res: Result<(), ()> = Inbox::handle(&(), act.clone())
            .with::<FailingActor, Create, MyObject, _>()
            .done();
        assert!(res.is_err());

        let res: Result<(), ()> = Inbox::handle(&(), act.clone())
            .with::<FailingActor, Create, MyObject, _>()
            .with::<MyActor,      Create, MyObject, _>()
            .done();
        assert!(res.is_ok());
    }
}
