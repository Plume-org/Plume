use array_tool::vec::Uniq;
use diesel::PgConnection;
use reqwest::Client;
use rocket::http::Status;
use rocket::response::{Response, Responder};
use rocket::request::Request;
use serde_json;
use std::sync::Arc;

use activity_pub::{activity_pub, ActivityPub, context};
use activity_pub::activity::Activity;
use activity_pub::actor::Actor;
use activity_pub::request;
use activity_pub::sign::*;

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

pub fn broadcast<A: Activity + Clone, S: Actor + Signer, T: Actor>(conn: &PgConnection, sender: &S, act: A, to: Vec<T>) {
    let boxes = to.into_iter()
        .map(|u| u.get_shared_inbox_url().unwrap_or(u.get_inbox_url()))
        .collect::<Vec<String>>()
        .unique();
    for inbox in boxes {
        // TODO: run it in Sidekiq or something like that        
        
        let mut act = act.serialize();
        act["@context"] = context();
        let signed = act.sign(sender, conn);
        
        let res = Client::new()
            .post(&inbox[..])
            .headers(request::headers())
            .header(request::signature(sender, request::headers(), conn))
            .header(request::digest(signed.to_string()))
            .body(signed.to_string())
            .send();
        match res {
            Ok(mut r) => println!("Successfully sent activity to inbox ({})\n\n{:?}", inbox, r.text().unwrap()),
            Err(e) => println!("Error while sending to inbox ({:?})", e)
        }
    }
}
