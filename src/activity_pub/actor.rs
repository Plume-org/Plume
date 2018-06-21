use diesel::PgConnection;

use activity_pub::ap_url;
use models::instance::Instance;

pub trait Actor: Sized {
    fn get_box_prefix() -> &'static str;

    fn get_actor_id(&self) -> String;

    fn get_instance(&self, conn: &PgConnection) -> Instance;

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
}
