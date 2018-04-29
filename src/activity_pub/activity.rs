use diesel::PgConnection;
use serde_json;

use activity_pub::actor::Actor;
use activity_pub::object::Object;

#[derive(Clone)]
pub enum Activity {
    Create(CreatePayload)
}
impl Activity {
    pub fn serialize(&self) -> serde_json::Value {
        match self {
            Activity::Create(data) => json!({
                "type": "Create",
                "by": data.by,
                "object": data.object
            })
        }
    }

    pub fn create<T: Object, U: Actor>(by: &U, obj: T, conn: &PgConnection) -> Activity {
        Activity::Create(CreatePayload::new(serde_json::Value::String(by.compute_id(conn)), obj.serialize()))
    }
}

#[derive(Clone)]
pub struct CreatePayload {
    by: serde_json::Value,
    object: serde_json::Value
}

impl CreatePayload {
    pub fn new(by: serde_json::Value, obj: serde_json::Value) -> CreatePayload {
        CreatePayload {
            by: by,
            object: obj
        }
    }
}

