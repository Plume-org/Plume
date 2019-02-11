use activitypub::activity::*;
use rocket::{
    data::*,
    http::Status,
    Outcome::{self, *},
    Request,
    response::status,
};
use rocket_contrib::json::*;
use serde::Deserialize;
use serde_json;

use std::io::Read;

use plume_common::activity_pub::{
    inbox::{AsActor, Inbox},
    request::Digest,
    sign::{verify_http_headers, Signable},
};
use plume_models::{
    Context, comments::Comment, db_conn::DbConn, follows, headers::Headers,
    instance::Instance, likes, posts::Post, reshares::Reshare,
    users::User, search::Searcher, Connection, Error,
};

macro_rules! impl_into_inbox_result {
    ( $( $t:ty => $variant:ident ),+ ) => {
        $(
            impl From<$t> for InboxResult {
                fn from(x: $t) -> InboxResult {
                    InboxResult::$variant(x)
                }
            }
        )+
    }
}

pub enum InboxResult {
    Commented(Comment),
    Followed(follows::Follow),
    Liked(likes::Like),
    Other,
    Post(Post),
    Reshared(Reshare),
}

impl From<()> for InboxResult {
    fn from(_: ()) -> InboxResult {
        InboxResult::Other
    }
}

impl_into_inbox_result! {
    Comment => Commented,
    follows::Follow => Followed,
    likes::Like => Liked,
    Post => Post,
    Reshare => Reshared
}

fn inbox(conn: &Connection, search: &Searcher, act: serde_json::Value) -> Result<InboxResult, Error> {
    Inbox::handle(&Context::build(conn, search), act)
        .with::<User, Announce, Post,            _>()
        .with::<User, Create,   Comment,         _>()
        .with::<User, Create,   Post,            _>()
        .with::<User, Delete,   Comment,         _>()
        .with::<User, Delete,   Post,            _>()
        .with::<User, Follow,   User,            _>()
        .with::<User, Like,     Post,            _>()
        .with::<User, Undo,     Reshare,         _>()
        .with::<User, Undo,     follows::Follow, _>()
        .with::<User, Undo,     likes::Like,     _>()
        .with::<User, Update,   Post,            _>()
        .done()
}

pub fn handle_incoming(
    conn: DbConn,
    data: SignedJson<serde_json::Value>,
    headers: Headers,
    searcher: crate::Searcher
) -> Result<String, status::BadRequest<&'static str>> {
    let act = data.1.into_inner();
    let sig = data.0;

    let activity = act.clone();
    let actor_id = activity["actor"].as_str()
        .or_else(|| activity["actor"]["id"].as_str())
        .ok_or(status::BadRequest(Some("Missing actor id for activity")))?;

    let actor = User::get_or_fetch(&Context::build(&conn, &searcher), actor_id)
        .expect("instance::shared_inbox: user error");
    if !verify_http_headers(&actor, &headers.0, &sig).is_secure() &&
        !act.clone().verify(&actor) {
        // maybe we just know an old key?
        actor.refetch(&conn).and_then(|_| User::get(&conn, actor.id))
            .and_then(|u| if verify_http_headers(&u, &headers.0, &sig).is_secure() ||
                      act.clone().verify(&u) {
                          Ok(())
                      } else {
                          Err(Error::Signature)
                      })
            .map_err(|_| {
                println!("Rejected invalid activity supposedly from {}, with headers {:?}", actor.username, headers.0);
                status::BadRequest(Some("Invalid signature"))})?;
    }

    if Instance::is_blocked(&*conn, actor_id).map_err(|_| status::BadRequest(Some("Can't tell if instance is blocked")))? {
        return Ok(String::new());
    }

    Ok(match inbox(&conn, &searcher, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("Shared inbox error: {:?}", e);
            format!("Error: {:?}", e)
        }
    })
}

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
