use chrono;
use diesel::PgConnection;
use serde_json;
use std::str::FromStr;

use activity_pub::actor::Actor;
use activity_pub::object::Object;

pub trait Activity {
    fn get_id(&self) -> String;

    fn serialize(&self) -> serde_json::Value;

    // fn deserialize(serde_json::Value) -> Self;
}

#[derive(Clone)]
pub struct Accept {
    id: String,
    actor: serde_json::Value,
    object: serde_json::Value,
    date: chrono::DateTime<chrono::Utc>
}

impl Accept {
    pub fn new<A: Activity, B: Actor>(who: &B, what: &A, conn: &PgConnection) -> Accept {
        Accept {
            id: "TODO".to_string(),
            actor: serde_json::Value::String(who.compute_id(conn)),
            object: serde_json::Value::String(what.get_id()),
            date: chrono::Utc::now()
        }
    }
}

impl Activity for Accept {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn serialize(&self) -> serde_json::Value {
        json!({
            "type": "Accept",
            "actor": self.actor,
            "object": self.object,
            "published": self.date.to_rfc3339()
        })
    }
}

#[derive(Clone)]
pub struct Create {
    id: String,
    actor: serde_json::Value,
    object: serde_json::Value,
    date: chrono::DateTime<chrono::Utc>
}

impl Create {
    pub fn new<A: Actor, B: Object>(actor: &A, obj: &B, conn: &PgConnection) -> Create {
        Create {
            id: "TODO".to_string(),
            actor: serde_json::Value::String(actor.compute_id(conn)),
            object: obj.serialize(conn),
            date: chrono::Utc::now()
        }
    }
}

impl Activity for Create {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn serialize(&self) -> serde_json::Value {
        json!({
            "type": "Create",
            "actor": self.actor,
            "object": self.object,
            "published": self.date.to_rfc3339()
        })
    }
}

#[derive(Clone)]
pub struct Follow {
    id: String,
    actor: serde_json::Value,
    object: serde_json::Value,
    date: chrono::DateTime<chrono::Utc>
}

impl Follow {
    pub fn new<A: Actor, B: Actor>(follower: &A, following: &B, conn: &PgConnection) -> Follow {
        Follow {
            id: "TODO".to_string(),
            actor: serde_json::Value::String(follower.compute_id(conn)),
            object: serde_json::Value::String(following.compute_id(conn)),
            date: chrono::Utc::now()
        }
    }

    pub fn deserialize(json: serde_json::Value) -> Follow {
        Follow {
            id: json["id"].as_str().unwrap().to_string(),
            actor: json["actor"].clone(),
            object: json["object"].clone(),
            date: chrono::DateTime::from_str(json["published"].as_str().unwrap()).unwrap()
        }
    }

    pub fn get_target_id(&self) -> String {
        self.object.as_str().unwrap().to_string()
    }
}

impl Activity for Follow {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn serialize(&self) -> serde_json::Value {
        json!({
            "type": "Follow",
            "actor": self.actor,
            "object": self.object,
            "published": self.date.to_rfc3339()
        })
    }
}
