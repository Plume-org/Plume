use reqwest;
use std::fmt::Debug;

use super::{request, sign::Signer};

/// Represents an ActivityPub inbox.
///
/// It routes an incoming Activity through the registered handlers.
///
/// # Example
///
/// ```rust
/// # use activitystreams::{prelude::*, base::Base, actor::Person, activity::{Announce, Create}, object::Note, iri_string::types::IriString};
/// # use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa};
/// # use once_cell::sync::Lazy;
/// # use plume_common::activity_pub::inbox::*;
/// # use plume_common::activity_pub::sign::{gen_keypair, Error as SignError, Result as SignResult, Signer};
/// #
/// # static MY_SIGNER: Lazy<MySigner> = Lazy::new(|| MySigner::new());
/// #
/// # struct MySigner {
/// #     public_key: String,
/// #     private_key: String,
/// # }
/// #
/// # impl MySigner {
/// #     fn new() -> Self {
/// #         let (pub_key, priv_key) = gen_keypair();
/// #         Self {
/// #             public_key: String::from_utf8(pub_key).unwrap(),
/// #             private_key: String::from_utf8(priv_key).unwrap(),
/// #         }
/// #     }
/// # }
/// #
/// # impl Signer for MySigner {
/// #     fn get_key_id(&self) -> String {
/// #         "mysigner".into()
/// #     }
/// #
/// #     fn sign(&self, to_sign: &str) -> SignResult<Vec<u8>> {
/// #         let key = PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.as_ref()).unwrap())
/// #             .unwrap();
/// #         let mut signer = openssl::sign::Signer::new(MessageDigest::sha256(), &key).unwrap();
/// #         signer.update(to_sign.as_bytes()).unwrap();
/// #         signer.sign_to_vec().map_err(|_| SignError())
/// #     }
/// #
/// #     fn verify(&self, data: &str, signature: &[u8]) -> SignResult<bool> {
/// #         let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).unwrap())
/// #             .unwrap();
/// #         let mut verifier = openssl::sign::Verifier::new(MessageDigest::sha256(), &key).unwrap();
/// #         verifier.update(data.as_bytes()).unwrap();
/// #         verifier.verify(&signature).map_err(|_| SignError())
/// #     }
/// # }
/// #
/// # struct User;
/// # impl FromId<()> for User {
/// #     type Error = ();
/// #     type Object = Person;
/// #
/// #     fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
/// #         Ok(User)
/// #     }
/// #
/// #     fn from_activity(_: &(), obj: Person) -> Result<Self, Self::Error> {
/// #         Ok(User)
/// #     }
/// #
/// #     fn get_sender() -> &'static dyn Signer {
/// #         &*MY_SIGNER
/// #     }
/// # }
/// # impl AsActor<&()> for User {
/// #    fn get_inbox_url(&self) -> String {
/// #        String::new()
/// #    }
/// #    fn is_local(&self) -> bool { false }
/// # }
/// # struct Message;
/// # impl FromId<()> for Message {
/// #     type Error = ();
/// #     type Object = Note;
/// #
/// #     fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
/// #         Ok(Message)
/// #     }
/// #
/// #     fn from_activity(_: &(), obj: Note) -> Result<Self, Self::Error> {
/// #         Ok(Message)
/// #     }
/// #
/// #     fn get_sender() -> &'static dyn Signer {
/// #         &*MY_SIGNER
/// #     }
/// # }
/// # impl AsObject<User, Create, &()> for Message {
/// #     type Error = ();
/// #     type Output = ();
/// #
/// #     fn activity(self, _: &(), _actor: User, _id: &str) -> Result<(), ()> {
/// #         Ok(())
/// #     }
/// # }
/// # impl AsObject<User, Announce, &()> for Message {
/// #     type Error = ();
/// #     type Output = ();
/// #
/// #     fn activity(self, _: &(), _actor: User, _id: &str) -> Result<(), ()> {
/// #         Ok(())
/// #     }
/// # }
/// #
/// # let mut person = Person::new();
/// # person.set_id("https://test.ap/actor".parse::<IriString>().unwrap());
/// # let mut act = Create::new(
/// #     Base::retract(person).unwrap().into_generic().unwrap(),
/// #     Base::retract(Note::new()).unwrap().into_generic().unwrap()
/// # );
/// # act.set_id("https://test.ap/activity".parse::<IriString>().unwrap());
/// # let activity_json = serde_json::to_value(act).unwrap();
/// #
/// # let conn = ();
/// #
/// let result: Result<(), ()> = Inbox::handle(&conn, activity_json)
///    .with::<User, Announce, Message>(None)
///    .with::<User, Create,   Message>(None)
///    .done();
/// ```
pub enum Inbox<'a, C, E, R>
where
    E: From<InboxError<E>> + Debug,
{
    /// The activity has not been handled yet
    ///
    /// # Structure
    ///
    /// - the context to be passed to each handler.
    /// - the activity
    /// - the reason it has not been handled yet
    NotHandled(&'a mut C, serde_json::Value, InboxError<E>),

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
pub enum InboxError<E: Debug> {
    /// None of the registered handlers matched
    NoMatch,

    /// No ID was provided for the incoming activity, or it was not a string
    InvalidID,

    /// The activity type matched for at least one handler, but then the actor was
    /// not of the expected type
    InvalidActor(Option<E>),

    /// Activity and Actor types matched, but not the Object
    InvalidObject(Option<E>),

    /// Error while dereferencing the object
    DerefError,
}

impl<T: Debug> From<InboxError<T>> for () {
    fn from(_: InboxError<T>) {}
}

/*
 Type arguments:
 - C: Context
 - E: Error
 - R: Result
*/
impl<'a, C, E, R> Inbox<'a, C, E, R>
where
    E: From<InboxError<E>> + Debug,
{
    /// Creates a new `Inbox` to handle an incoming activity.
    ///
    /// # Parameters
    ///
    /// - `ctx`: the context to pass to each handler
    /// - `json`: the JSON representation of the incoming activity
    pub fn handle(ctx: &'a mut C, json: serde_json::Value) -> Inbox<'a, C, E, R> {
        Inbox::NotHandled(ctx, json, InboxError::NoMatch)
    }

    /// Registers an handler on this Inbox.
    pub async fn with<A, V, M>(self, proxy: Option<&reqwest::Proxy>) -> Self
    where
        A: AsActor<&'a mut C> + FromId<C, Error = E>,
        V: activitystreams::markers::Activity + serde::de::DeserializeOwned,
        M: AsObject<A, V, &'a mut C, Error = E> + FromId<C, Error = E>,
        M::Output: Into<R>,
    {
        if let Self::NotHandled(ctx, mut act, e) = self {
            if serde_json::from_value::<V>(act.clone()).is_ok() {
                let act_clone = act.clone();
                let act_id = match act_clone["id"].as_str() {
                    Some(x) => x,
                    None => return Self::NotHandled(ctx, act, InboxError::InvalidID),
                };

                // Get the actor ID
                let actor_id = match get_id(act["actor"].clone()) {
                    Some(x) => x,
                    None => return Self::NotHandled(ctx, act, InboxError::InvalidActor(None)),
                };

                if Self::is_spoofed_activity(&actor_id, &act) {
                    return Self::NotHandled(ctx, act, InboxError::InvalidObject(None));
                }

                // Transform this actor to a model (see FromId for details about the from_id function)
                let actor = match A::from_id(
                    ctx,
                    &actor_id,
                    serde_json::from_value(act["actor"].clone()).ok(),
                    proxy,
                ).await {
                    Ok(a) => a,
                    // If the actor was not found, go to the next handler
                    Err((json, e)) => {
                        if let Some(json) = json {
                            act["actor"] = json;
                        }
                        return Self::NotHandled(ctx, act, InboxError::InvalidActor(Some(e)));
                    }
                };

                // Same logic for "object"
                let obj_id = match get_id(act["object"].clone()) {
                    Some(x) => x,
                    None => return Self::NotHandled(ctx, act, InboxError::InvalidObject(None)),
                };
                let obj = match M::from_id(
                    ctx,
                    &obj_id,
                    serde_json::from_value(act["object"].clone()).ok(),
                    proxy,
                ).await {
                    Ok(o) => o,
                    Err((json, e)) => {
                        if let Some(json) = json {
                            act["object"] = json;
                        }
                        return Self::NotHandled(ctx, act, InboxError::InvalidObject(Some(e)));
                    }
                };

                // Handle the activity
                match obj.activity(ctx, actor, act_id).await {
                    Ok(res) => Self::Handled(res.into()),
                    Err(e) => Self::Failed(e),
                }
            } else {
                // If the Activity type is not matching the expected one for
                // this handler, try with the next one.
                Self::NotHandled(ctx, act, e)
            }
        } else {
            self
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

    fn is_spoofed_activity(actor_id: &str, act: &serde_json::Value) -> bool {
        use serde_json::Value::{Array, Object, String};

        let attributed_to = act["object"].get("attributedTo");
        if attributed_to.is_none() {
            return false;
        }
        let attributed_to = attributed_to.unwrap();
        match attributed_to {
            Array(v) => v.iter().all(|i| match i {
                String(s) => s != actor_id,
                Object(obj) => obj.get("id").map_or(true, |s| s != actor_id),
                _ => false,
            }),
            String(s) => s != actor_id,
            Object(obj) => obj.get("id").map_or(true, |s| s != actor_id),
            _ => false,
        }
    }
}

/// Get the ActivityPub ID of a JSON value.
///
/// If the value is a string, its value is returned.
/// If it is an object, and that its `id` field is a string, we return it.
///
/// Otherwise, `None` is returned.
fn get_id(json: serde_json::Value) -> Option<String> {
    match json {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Object(map) => map.get("id")?.as_str().map(ToString::to_string),
        _ => None,
    }
}

/// A trait for ActivityPub objects that can be retrieved or constructed from ID.
///
/// The two functions to implement are `from_activity` to create (and save) a new object
/// of this type from its AP representation, and `from_db` to try to find it in the database
/// using its ID.
///
/// When dealing with the "object" field of incoming activities, `Inbox` will try to see if it is
/// a full object, and if so, save it with `from_activity`. If it is only an ID, it will try to find
/// it in the database with `from_db`, and otherwise dereference (fetch) the full object and parse it
/// with `from_activity`.
#[allow(async_fn_in_trait)]
pub trait FromId<C>: Sized {
    /// The type representing a failure
    type Error: From<InboxError<Self::Error>> + Debug;

    /// The ActivityPub object type representing Self
    type Object: activitystreams::markers::Object + serde::de::DeserializeOwned;

    /// Tries to get an instance of `Self` from an ActivityPub ID.
    ///
    /// # Parameters
    ///
    /// - `ctx`: a context to get this instance (= a database in which to search)
    /// - `id`: the ActivityPub ID of the object to find
    /// - `object`: optional object that will be used if the object was not found in the database
    ///   If absent, the ID will be dereferenced.
    async fn from_id(
        ctx: &mut C,
        id: &str,
        object: Option<Self::Object>,
        proxy: Option<&reqwest::Proxy>,
    ) -> Result<Self, (Option<serde_json::Value>, Self::Error)> {
        match Self::from_db(ctx, id) {
            Ok(x) => Ok(x),
            _ => match object {
                Some(o) => Self::from_activity(ctx, o).await.map_err(|e| (None, e)),
                None => Self::from_activity(ctx, Self::deref(id, proxy.cloned())?).await
                    .map_err(|e| (None, e)),
            },
        }
    }

    /// Dereferences an ID
    fn deref(
        id: &str,
        proxy: Option<reqwest::Proxy>,
    ) -> Result<Self::Object, (Option<serde_json::Value>, Self::Error)> {
        request::get(id, Self::get_sender(), proxy)
            .map_err(|_| (None, InboxError::DerefError))
            .and_then(|r| {
                let json: serde_json::Value = r
                    .json()
                    .map_err(|_| (None, InboxError::InvalidObject(None)))?;
                serde_json::from_value(json.clone())
                    .map_err(|_| (Some(json), InboxError::InvalidObject(None)))
            })
            .map_err(|(json, e)| (json, e.into()))
    }

    /// Builds a `Self` from its ActivityPub representation
    async fn from_activity(ctx: &mut C, activity: Self::Object) -> Result<Self, Self::Error>;

    /// Tries to find a `Self` with a given ID (`id`), using `ctx` (a database)
    fn from_db(ctx: &mut C, id: &str) -> Result<Self, Self::Error>;

    fn get_sender() -> &'static dyn Signer;
}

/// Should be implemented by anything representing an ActivityPub actor.
///
/// # Type arguments
///
/// - `C`: the context to be passed to this activity handler from the `Inbox` (usually a database connection)
pub trait AsActor<C> {
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
/// # use activitystreams::{prelude::*, activity::Create, actor::Person, object::Note};
/// # use plume_common::activity_pub::inbox::{AsActor, AsObject, FromId};
/// # use plume_common::activity_pub::sign::{gen_keypair, Error as SignError, Result as SignResult, Signer};
/// # use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa};
/// # use once_cell::sync::Lazy;
/// #
/// # static MY_SIGNER: Lazy<MySigner> = Lazy::new(|| MySigner::new());
/// #
/// # struct MySigner {
/// #     public_key: String,
/// #     private_key: String,
/// # }
/// #
/// # impl MySigner {
/// #     fn new() -> Self {
/// #         let (pub_key, priv_key) = gen_keypair();
/// #         Self {
/// #             public_key: String::from_utf8(pub_key).unwrap(),
/// #             private_key: String::from_utf8(priv_key).unwrap(),
/// #         }
/// #     }
/// # }
/// #
/// # impl Signer for MySigner {
/// #     fn get_key_id(&self) -> String {
/// #         "mysigner".into()
/// #     }
/// #
/// #     fn sign(&self, to_sign: &str) -> SignResult<Vec<u8>> {
/// #         let key = PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.as_ref()).unwrap())
/// #             .unwrap();
/// #         let mut signer = openssl::sign::Signer::new(MessageDigest::sha256(), &key).unwrap();
/// #         signer.update(to_sign.as_bytes()).unwrap();
/// #         signer.sign_to_vec().map_err(|_| SignError())
/// #     }
/// #
/// #     fn verify(&self, data: &str, signature: &[u8]) -> SignResult<bool> {
/// #         let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).unwrap())
/// #             .unwrap();
/// #         let mut verifier = openssl::sign::Verifier::new(MessageDigest::sha256(), &key).unwrap();
/// #         verifier.update(data.as_bytes()).unwrap();
/// #         verifier.verify(&signature).map_err(|_| SignError())
/// #     }
/// # }
/// #
/// # struct Account;
/// # impl FromId<()> for Account {
/// #     type Error = ();
/// #     type Object = Person;
/// #
/// #     fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
/// #         Ok(Account)
/// #     }
/// #
/// #     fn from_activity(_: &(), obj: Person) -> Result<Self, Self::Error> {
/// #         Ok(Account)
/// #     }
/// #
/// #     fn get_sender() -> &'static dyn Signer {
/// #         &*MY_SIGNER
/// #     }
/// # }
/// # impl AsActor<()> for Account {
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
/// impl FromId<()> for Message {
///     type Error = ();
///     type Object = Note;
///
///     fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
///         Ok(Message { text: "From DB".into() })
///     }
///
///     fn from_activity(_: &(), obj: Note) -> Result<Self, Self::Error> {
///         Ok(Message {
///             text: obj.content()
///                 .and_then(|content| content.to_owned().single_xsd_string()).ok_or(())?
///         })
///     }
///
///     fn get_sender() -> &'static dyn Signer {
///         &*MY_SIGNER
///     }
/// }
///
/// impl AsObject<Account, Create, ()> for Message {
///     type Error = ();
///     type Output = ();
///
///     fn activity(self, _: (), _actor: Account, _id: &str) -> Result<(), ()> {
///         println!("New Note: {:?}", self);
///         Ok(())
///     }
/// }
/// ```
#[allow(async_fn_in_trait)]
pub trait AsObject<A, V, C>
where
    V: activitystreams::markers::Activity,
{
    /// What kind of error is returned when something fails
    type Error;

    /// What is returned by `AsObject::activity`, if anything is returned
    type Output;

    /// Handle a specific type of activity dealing with this type of objects.
    ///
    /// The implementations should check that the actor is actually authorized
    /// to perform this action.
    ///
    /// # Parameters
    ///
    /// - `self`: the object on which the activity acts
    /// - `ctx`: the context passed to `Inbox::handle`
    /// - `actor`: the actor who did this activity
    /// - `id`: the ID of this activity
    async fn activity(self, ctx: C, actor: A, id: &str) -> Result<Self::Output, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_pub::sign::{
        gen_keypair, Error as SignError, Result as SignResult, Signer,
    };
    use activitystreams::{
        activity::{Announce, Create, Delete, Like},
        actor::Person,
        base::Base,
        object::Note,
        prelude::*,
    };
    use once_cell::sync::Lazy;
    use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa};

    static MY_SIGNER: Lazy<MySigner> = Lazy::new(MySigner::new);

    struct MySigner {
        public_key: String,
        private_key: String,
    }

    impl MySigner {
        fn new() -> Self {
            let (pub_key, priv_key) = gen_keypair();
            Self {
                public_key: String::from_utf8(pub_key).unwrap(),
                private_key: String::from_utf8(priv_key).unwrap(),
            }
        }
    }

    impl Signer for MySigner {
        fn get_key_id(&self) -> String {
            "mysigner".into()
        }

        fn sign(&self, to_sign: &str) -> SignResult<Vec<u8>> {
            let key = PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.as_ref()).unwrap())
                .unwrap();
            let mut signer = openssl::sign::Signer::new(MessageDigest::sha256(), &key).unwrap();
            signer.update(to_sign.as_bytes()).unwrap();
            signer.sign_to_vec().map_err(|_| SignError())
        }

        fn verify(&self, data: &str, signature: &[u8]) -> SignResult<bool> {
            let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).unwrap())
                .unwrap();
            let mut verifier = openssl::sign::Verifier::new(MessageDigest::sha256(), &key).unwrap();
            verifier.update(data.as_bytes()).unwrap();
            verifier.verify(signature).map_err(|_| SignError())
        }
    }

    struct MyActor;
    impl FromId<()> for MyActor {
        type Error = ();
        type Object = Person;

        fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
            Ok(Self)
        }

        fn from_activity(_: &(), _obj: Person) -> Result<Self, Self::Error> {
            Ok(Self)
        }

        fn get_sender() -> &'static dyn Signer {
            &*MY_SIGNER
        }
    }

    impl AsActor<&()> for MyActor {
        fn get_inbox_url(&self) -> String {
            String::from("https://test.ap/my-actor/inbox")
        }

        fn is_local(&self) -> bool {
            false
        }
    }

    struct MyObject;
    impl FromId<()> for MyObject {
        type Error = ();
        type Object = Note;

        fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
            Ok(Self)
        }

        fn from_activity(_: &(), _obj: Note) -> Result<Self, Self::Error> {
            Ok(Self)
        }

        fn get_sender() -> &'static dyn Signer {
            &*MY_SIGNER
        }
    }
    impl AsObject<MyActor, Create, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(self, _: &(), _actor: MyActor, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is creating a Note");
            Ok(())
        }
    }

    impl AsObject<MyActor, Like, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(self, _: &(), _actor: MyActor, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is liking a Note");
            Ok(())
        }
    }

    impl AsObject<MyActor, Delete, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(self, _: &(), _actor: MyActor, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is deleting a Note");
            Ok(())
        }
    }

    impl AsObject<MyActor, Announce, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(self, _: &(), _actor: MyActor, _id: &str) -> Result<Self::Output, Self::Error> {
            println!("MyActor is announcing a Note");
            Ok(())
        }
    }

    fn build_create() -> Create {
        let mut person = Person::new();
        person.set_id("https://test.ap/actor".parse().unwrap());
        let mut note = Note::new();
        note.set_id("https://test.ap/note".parse().unwrap());
        let mut act = Create::new(
            Base::retract(person).unwrap().into_generic().unwrap(),
            Base::retract(note).unwrap().into_generic().unwrap(),
        );
        act.set_id("https://test.ap/activity".parse().unwrap());
        act
    }

    #[test]
    fn test_inbox_basic() {
        let act = serde_json::to_value(build_create()).unwrap();
        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<MyActor, Create, MyObject>(None)
            .done();
        assert!(res.is_ok());
    }

    #[test]
    fn test_inbox_multi_handlers() {
        let act = serde_json::to_value(build_create()).unwrap();
        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<MyActor, Announce, MyObject>(None)
            .with::<MyActor, Delete, MyObject>(None)
            .with::<MyActor, Create, MyObject>(None)
            .with::<MyActor, Like, MyObject>(None)
            .done();
        assert!(res.is_ok());
    }

    #[test]
    fn test_inbox_failure() {
        let act = serde_json::to_value(build_create()).unwrap();
        // Create is not handled by this inbox
        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<MyActor, Announce, MyObject>(None)
            .with::<MyActor, Like, MyObject>(None)
            .done();
        assert!(res.is_err());
    }

    struct FailingActor;
    impl AsActor<&()> for FailingActor {
        fn get_inbox_url(&self) -> String {
            String::from("https://test.ap/failing-actor/inbox")
        }

        fn is_local(&self) -> bool {
            false
        }
    }

    impl FromId<()> for FailingActor {
        type Error = ();
        type Object = Person;

        fn from_db(_: &(), _id: &str) -> Result<Self, Self::Error> {
            Err(())
        }

        fn from_activity(_: &(), _obj: Self::Object) -> Result<Self, Self::Error> {
            Err(())
        }

        fn get_sender() -> &'static dyn Signer {
            &*MY_SIGNER
        }
    }

    impl AsObject<FailingActor, Create, &()> for MyObject {
        type Error = ();
        type Output = ();

        fn activity(
            self,
            _: &(),
            _actor: FailingActor,
            _id: &str,
        ) -> Result<Self::Output, Self::Error> {
            println!("FailingActor is creating a Note");
            Ok(())
        }
    }

    #[test]
    fn test_inbox_actor_failure() {
        let act = serde_json::to_value(build_create()).unwrap();

        let res: Result<(), ()> = Inbox::handle(&(), act.clone())
            .with::<FailingActor, Create, MyObject>(None)
            .done();
        assert!(res.is_err());

        let res: Result<(), ()> = Inbox::handle(&(), act)
            .with::<FailingActor, Create, MyObject>(None)
            .with::<MyActor, Create, MyObject>(None)
            .done();
        assert!(res.is_ok());
    }
}
