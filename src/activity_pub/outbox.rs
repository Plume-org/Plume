use diesel::PgConnection;
use rocket::http::Status;
use rocket::response::{Response, Responder};
use rocket::request::Request;
use serde_json;

use activity_pub::{activity_pub, ActivityPub, context};
use activity_pub::activity::Activity;
use activity_pub::actor::Actor;
use models::users::User;

pub struct Outbox<A> where A: Activity + Clone {
    id: String,
    items: Vec<Box<A>>
}

impl<A: Activity + Clone + 'static> Outbox<A> {
    pub fn new(id: String, items: Vec<Box<A>>) -> Outbox<A> {
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

impl<'r, A: Activity + Clone + 'static> Responder<'r> for Outbox<A> {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        self.serialize().respond_to(request)
    }
}

pub fn broadcast<A: Activity + Clone>(conn: &PgConnection, act: A, to: Vec<User>) {
    for user in to {
        user.send_to_inbox(conn, act.clone()); // TODO: run it in Sidekiq or something like that
    }
}
