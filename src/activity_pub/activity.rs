use chrono;
use diesel::PgConnection;
use serde_json;

use activity_pub::actor::Actor;
use activity_pub::object::Object;

#[derive(Clone)]
pub enum Activity {
    Create(Payload),
    Accept(Payload)
}
impl Activity {
    pub fn serialize(&self) -> serde_json::Value {
        json!({
            "type": self.get_type(),
            "actor": self.payload().by,
            "object": self.payload().object,
            "published": self.payload().date.to_rfc3339()
        })
    }

    pub fn get_type(&self) -> String {
        match self {
            Activity::Accept(_) => String::from("Accept"),
            Activity::Create(_) => String::from("Create")
        }
    }

    pub fn payload(&self) -> Payload {
        match self {
            Activity::Accept(p) => p.clone(),
            Activity::Create(p) => p.clone()
        }
    }

    pub fn create<T: Object, U: Actor>(by: &U, obj: T, conn: &PgConnection) -> Activity {
        Activity::Create(Payload::new(serde_json::Value::String(by.compute_id(conn)), obj.serialize(conn)))
    }

    pub fn accept<A: Actor>(by: &A, what: String, conn: &PgConnection) -> Activity {
        Activity::Accept(Payload::new(serde_json::Value::String(by.compute_id(conn)), serde_json::Value::String(what)))
    }
}

#[derive(Clone)]
pub struct Payload {
    by: serde_json::Value,
    object: serde_json::Value,
    date: chrono::DateTime<chrono::Utc>
}

impl Payload {
    pub fn new(by: serde_json::Value, obj: serde_json::Value) -> Payload {
        Payload {
            by: by,
            object: obj,
            date: chrono::Utc::now()
        }
    }
}

