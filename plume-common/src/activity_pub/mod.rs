use activitypub::{Activity, Actor, Object, Link};
use array_tool::vec::Uniq;
use reqwest::Client;
use rocket::{
    Outcome,
    http::Status,
    response::{Response, Responder},
    request::{FromRequest, Request}
};
use serde_json;

use self::sign::Signable;

pub mod inbox;
pub mod request;
pub mod sign;

pub const CONTEXT_URL: &'static str = "https://www.w3.org/ns/activitystreams";
pub const PUBLIC_VISIBILTY: &'static str = "https://www.w3.org/ns/activitystreams#Public";

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

pub struct ActivityStream<T> (T);

impl<T> ActivityStream<T> {
    pub fn new(t: T) -> ActivityStream<T> {
        ActivityStream(t)
    }
}

impl<'r, O: Object> Responder<'r> for ActivityStream<O> {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        let mut json = serde_json::to_value(&self.0).map_err(|_| Status::InternalServerError)?;
        json["@context"] = context();
        serde_json::to_string(&json).respond_to(request).map(|r| Response::build_from(r)
            .raw_header("Content-Type", "application/activity+json")
            .finalize())
    }
}

#[derive(Clone)]
pub struct ApRequest;
impl<'a, 'r> FromRequest<'a, 'r> for ApRequest {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, (Status, Self::Error), ()> {
        request.headers().get_one("Accept").map(|header| header.split(",").map(|ct| match ct.trim() {
            // bool for Forward: true if found a valid Content-Type for Plume first (HTML), false otherwise
            "application/ld+json; profile=\"https://w3.org/ns/activitystreams\"" |
            "application/ld+json;profile=\"https://w3.org/ns/activitystreams\"" |
            "application/activity+json" |
            "application/ld+json" => Outcome::Success(ApRequest),
            "text/html" => Outcome::Forward(true),
            _ => Outcome::Forward(false)
        }).fold(Outcome::Forward(false), |out, ct| if out.is_success() || (out.is_forward() && out.clone().forwarded().unwrap()) {
                out
            } else {
                ct
        }).map_forward(|_| ())).unwrap_or(Outcome::Forward(()))
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, Properties)]
#[serde(rename_all = "camelCase")]
pub struct ApSignature {
    #[activitystreams(concrete(PublicKey), functional)]
    pub public_key: Option<serde_json::Value>
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Properties)]
#[serde(rename_all = "camelCase")]
pub struct PublicKey {
    #[activitystreams(concrete(String), functional)]
    pub id: Option<serde_json::Value>,

    #[activitystreams(concrete(String), functional)]
    pub owner: Option<serde_json::Value>,

    #[activitystreams(concrete(String), functional)]
    pub public_key_pem: Option<serde_json::Value>
}
