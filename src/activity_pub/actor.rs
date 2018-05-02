use diesel::PgConnection;
use reqwest::Client;

use BASE_URL;
use activity_pub::{activity_pub, ActivityPub, context, ap_url};
use activity_pub::activity::Activity;
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

    fn get_instance(&self, conn: &PgConnection) -> Instance;

    fn get_actor_type() -> ActorType;

    fn as_activity_pub (&self, conn: &PgConnection) -> ActivityPub {
        activity_pub(json!({
            "@context": context(),
            "id": self.compute_id(conn),
            "type": Self::get_actor_type().to_string(),
            "inbox": self.compute_inbox(conn),
            "outbox": self.compute_outbox(conn),
            "preferredUsername": self.get_actor_id(),
            "name": "",
            "summary": "",
            "url": self.compute_id(conn),
            "endpoints": {
                "sharedInbox": ap_url(format!("{}/inbox", BASE_URL.as_str()))
            }
        }))
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

    fn send_to_inbox<A: Activity>(&self, conn: &PgConnection, act: A) {
        let res = Client::new()
            .post(&self.compute_inbox(conn)[..])
            .body(act.serialize().to_string())
            .send();
        match res {
            Ok(_) => println!("Successfully sent activity to inbox"),
            Err(_) => println!("Error while sending to inbox")
        }
    }

    fn from_url(conn: &PgConnection, url: String) -> Option<Self>;
}
