use activitypub::{Activity, Actor, Object, Link};
use array_tool::vec::Uniq;
use reqwest::Client;
use rocket::{
    http::{ContentType, Status},
    response::{Response, Responder, Content},
    request::Request
};
use rocket_contrib::Json;
use serde_json;

use self::sign::Signable;

pub mod actor;
pub mod inbox;
pub mod request;
pub mod sign;

pub type ActivityPub = Content<Json<serde_json::Value>>;

pub const CONTEXT_URL: &'static str = "https://www.w3.org/ns/activitystreams";
pub const PUBLIC_VISIBILTY: &'static str = "https://www.w3.org/ns/activitystreams#Public";

#[cfg(debug_assertions)]
pub fn ap_url(url: String) -> String {
    format!("http://{}", url)
}

#[cfg(not(debug_assertions))]
pub fn ap_url(url: String) -> String {
    format!("https://{}", url)
}

pub fn context() -> serde_json::Value {
    json!([
        CONTEXT_URL,
        "https://w3id.org/security/v1",
        {
            "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
            "sensitive": "as:sensitive",
            "movedTo": "as:movedTo",
            "Hashtag": "as:Hashtag",
            "ostatus":"http://ostatus.org#",
            "atomUri":"ostatus:atomUri",
            "inReplyToAtomUri":"ostatus:inReplyToAtomUri",
            "conversation":"ostatus:conversation",
            "toot":"http://joinmastodon.org/ns#",
            "Emoji":"toot:Emoji",
            "focalPoint": {
                "@container":"@list",
                "@id":"toot:focalPoint"
            },
            "featured":"toot:featured"
        }
    ])
}

pub fn activity_pub(json: serde_json::Value) -> ActivityPub {
    Content(ContentType::new("application", "activity+json"), Json(json))
}

pub struct ActivityStream<T> (T);

impl<T> ActivityStream<T> {
    pub fn new(t: T) -> ActivityStream<T> {
        ActivityStream(t)
    }
}

impl<'r, O: Object> Responder<'r> for ActivityStream<O> {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        serde_json::to_string(&self.0).respond_to(request).map(|r| Response::build_from(r)
            .raw_header("Content-Type", "application/activity+json")
            .finalize())
    }
}

pub fn broadcast<A: Activity, S: sign::Signer, T: inbox::WithInbox + Actor>(sender: &S, act: A, to: Vec<T>) {
    let boxes = to.into_iter()
        .map(|u| u.get_shared_inbox_url().unwrap_or(u.get_inbox_url()))
        .collect::<Vec<String>>()
        .unique();

    let mut act = serde_json::to_value(act).unwrap();
    act["@context"] = context();
    let signed = act.sign(sender);

    for inbox in boxes {
        // TODO: run it in Sidekiq or something like that
        let res = Client::new()
            .post(&inbox[..])
            .headers(request::headers())
            .header(request::signature(sender, request::headers()))
            .header(request::digest(signed.to_string()))
            .body(signed.to_string())
            .send();
        match res {
            Ok(mut r) => println!("Successfully sent activity to inbox ({})\n\n{:?}", inbox, r.text().unwrap()),
            Err(e) => println!("Error while sending to inbox ({:?})", e)
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Id(String);

impl Id {
    pub fn new<T: Into<String>>(id: T) -> Id {
        Id(id.into())
    }
}

impl Into<String> for Id {
    fn into(self) -> String {
        self.0.clone()
    }
}

pub trait IntoId {
    fn into_id(self) -> Id;
}

impl Link for Id {}
