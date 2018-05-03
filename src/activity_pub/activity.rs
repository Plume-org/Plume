use chrono;
use diesel::PgConnection;
use serde_json;
use std::str::FromStr;

use activity_pub::actor::Actor;
use activity_pub::object::Object;

pub trait Activity {
    fn get_id(&self) -> String;

    fn get_type(&self) -> String;    

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
            id: format!("{}/accept/{}/{}", who.compute_id(conn), what.get_type().to_lowercase(), what.get_id()),
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

    fn get_type(&self) -> String {
        "Accept".to_string()
    }

    fn serialize(&self) -> serde_json::Value {
        json!({
            "type": "Accept",
            "id": self.id,            
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
            id: format!("{}/activity", obj.compute_id(conn)),
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

    fn get_type(&self) -> String {
        "Create".to_string()
    }

    fn serialize(&self) -> serde_json::Value {
        json!({
            "type": "Create",
            "id": self.id,
            "actor": self.actor,
            "object": self.object,
            "published": self.date.to_rfc3339(),
            "to": self.object["to"],
            "cc": self.object["cc"]
        })
    }
}

#[derive(Clone)]
pub struct Follow {
    id: String,
    actor: serde_json::Value,
    object: serde_json::Value
}

impl Follow {
    pub fn new<A: Actor, B: Actor>(follower: &A, following: &B, conn: &PgConnection) -> Follow {
        Follow {
            id: format!("{}/follow/{}", follower.compute_id(conn), following.compute_id(conn)),
            actor: serde_json::Value::String(follower.compute_id(conn)),
            object: serde_json::Value::String(following.compute_id(conn))
        }
    }

    pub fn deserialize(json: serde_json::Value) -> Follow {
        Follow {
            id: json["id"].as_str().unwrap().to_string(),
            actor: json["actor"].clone(),
            object: json["object"].clone()
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

    fn get_type(&self) -> String {
        "Follow".to_string()
    }

    fn serialize(&self) -> serde_json::Value {
        json!({
            "type": "Follow",
            "id": self.id,            
            "actor": self.actor,
            "object": self.object
        })
    }
}
