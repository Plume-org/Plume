use rocket::http::ContentType;
use rocket::response::Content;
use rocket_contrib::Json;
use serde_json;

pub mod activity;
pub mod actor;
pub mod inbox;
pub mod object;
pub mod outbox;
pub mod request;
pub mod sign;
pub mod webfinger;

pub type ActivityPub = Content<Json>;

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
