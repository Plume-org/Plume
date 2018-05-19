use diesel::PgConnection;
use serde_json;

pub trait Object {
    fn serialize(&self, conn: &PgConnection) -> serde_json::Value;

    fn compute_id(&self, conn: &PgConnection) -> String;
}
