use diesel::PgConnection;
use serde_json;

use activity_pub::ap_url;
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

    fn get_inbox_url(&self) -> String;

    fn get_shared_inbox_url(&self) -> Option<String>;

    fn custom_props(&self, _conn: &PgConnection) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
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

    fn from_url(conn: &PgConnection, url: String) -> Option<Self>;
}
