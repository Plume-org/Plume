use diesel::PgConnection;
use reqwest::Client;
use serde_json;

use BASE_URL;
use activity_pub::{activity_pub, ActivityPub, context, ap_url, CONTEXT};
use activity_pub::activity::Activity;
use activity_pub::sign::*;
use models::instance::Instance;

pub enum ActorType {
    Person,
    Blog
}

impl ToString for ActorType {
    fn to_string(&self) -> String {
        String::from(match self {
            ActorType::Person => "Person",
            ActorType::Blog => "Blog"
        })
    }
}

pub trait Actor: Sized {
    fn get_box_prefix() -> &'static str;

    fn get_actor_id(&self) -> String;

    fn get_display_name(&self) -> String;

    fn get_summary(&self) -> String;

    fn get_instance(&self, conn: &PgConnection) -> Instance;

    fn get_actor_type() -> ActorType;

    fn custom_props(&self, _conn: &PgConnection) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
    }

    fn as_activity_pub (&self, conn: &PgConnection) -> ActivityPub {
        let mut repr = json!({
            "@context": context(),
            "id": self.compute_id(conn),
            "type": Self::get_actor_type().to_string(),
            "inbox": self.compute_inbox(conn),
            "outbox": self.compute_outbox(conn),
            "preferredUsername": self.get_actor_id(),
            "name": self.get_display_name(),
            "summary": self.get_summary(),
            "url": self.compute_id(conn),
            "endpoints": {
                "sharedInbox": ap_url(format!("{}/inbox", BASE_URL.as_str()))
            }
        });

        self.custom_props(conn).iter().for_each(|p| repr[p.0] = p.1.clone());

        activity_pub(repr)
    }

    fn compute_outbox(&self, conn: &PgConnection) -> String {
        self.compute_box(conn, "outbox")
    }

    fn compute_inbox(&self, conn: &PgConnection) -> String {
        self.compute_box(conn, "inbox")
    }

    fn compute_box(&self, conn: &PgConnection, box_name: &str) -> String {
        format!("{id}/{name}", id = self.compute_id(conn), name = box_name)
    }

    fn compute_id(&self, conn: &PgConnection) -> String {
        ap_url(format!(
            "{instance}/{prefix}/{user}",
            instance = self.get_instance(conn).public_domain,
            prefix = Self::get_box_prefix(),
            user = self.get_actor_id()
        ))
    }

    fn send_to_inbox<A: Activity, S: Actor + Signer>(&self, conn: &PgConnection, sender: &S, act: A) {
        let mut act = act.serialize();
        act["@context"] = CONTEXT;
        let signed = act.sign(sender, conn);
        let res = Client::new()
            .post(&self.compute_inbox(conn)[..])
            .body(signed.to_string())
            .send();
        match res {
            Ok(_) => println!("Successfully sent activity to inbox"),
            Err(_) => println!("Error while sending to inbox")
        }
    }

    fn from_url(conn: &PgConnection, url: String) -> Option<Self>;
}
