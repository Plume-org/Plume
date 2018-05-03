use diesel::PgConnection;
use serde_json;

use activity_pub::actor::Actor;

pub trait Object {
    fn serialize(&self, conn: &PgConnection) -> serde_json::Value;

    fn compute_id(&self, conn: &PgConnection) -> String;
}

pub trait Attribuable {
    fn set_attribution<T>(&self, by: &T) where T: Actor;
}
