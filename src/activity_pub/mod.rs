use models::instance::Instance;
use diesel::PgConnection;
use serde_json::Value;

pub mod webfinger;

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

pub trait Actor {
    fn get_box_prefix() -> &'static str;

    fn get_actor_id(&self) -> String;

    fn get_instance(&self, conn: &PgConnection) -> Instance;

    fn get_actor_type() -> ActorType;

    fn as_activity_pub (&self, conn: &PgConnection) -> Value {
        json!({
            "@context": [
                "https://www.w3.org/ns/activitystreams",
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
            ],
            "id": self.compute_id(conn),
            "type": Self::get_actor_type().to_string(),
            "inbox": self.compute_inbox(conn),
            "outbox": self.compute_outbox(conn),
            "preferredUsername": self.get_actor_id(),
            "name": "",
            "summary": "",
            "url": self.compute_id(conn),
            "endpoints": {
                "sharedInbox": "https://plu.me/inbox"
            }
        })
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
        format!(
            "https://{instance}/{prefix}/{user}",
            instance = self.get_instance(conn).public_domain,
            prefix = Self::get_box_prefix(),
            user = self.get_actor_id()
        )
    }
}
