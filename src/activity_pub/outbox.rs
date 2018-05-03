use diesel::PgConnection;
use rocket::http::Status;
use rocket::response::{Response, Responder};
use rocket::request::Request;
use serde_json;
use std::sync::Arc;

use activity_pub::{activity_pub, ActivityPub, context};
use activity_pub::activity::Activity;
use activity_pub::actor::Actor;
use activity_pub::sign::Signer;
use models::users::User;

pub struct Outbox {
    id: String,
    items: Vec<Arc<Activity>>
}

impl Outbox {
    pub fn new(id: String, items: Vec<Arc<Activity>>) -> Outbox {
        Outbox {
            id: id,
            items: items
        }
    }

    fn serialize(&self) -> ActivityPub {
        let items = self.items.clone().into_iter().map(|i| i.serialize()).collect::<Vec<serde_json::Value>>();
        activity_pub(json!({
            "@context": context(),
            "type": "OrderedCollection",
            "id": self.id,
            "totalItems": items.len(),
            "orderedItems": items
        }))
    }
}

impl<'r> Responder<'r> for Outbox {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        self.serialize().respond_to(request)
    }
}

pub fn broadcast<A: Activity + Clone, S: Actor + Signer>(conn: &PgConnection, sender: &S, act: A, to: Vec<User>) {
    for user in to {
        user.send_to_inbox(conn, sender, act.clone()); // TODO: run it in Sidekiq or something like that
    }
}
