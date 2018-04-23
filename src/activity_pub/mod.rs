use models::instance::Instance;
use diesel::PgConnection;

trait Actor {
    fn get_box_prefix() -> String;

    fn get_actor_id(&self) -> String;

    fn get_instance(&self, conn: &PgConnection) -> Instance;

    fn compute_outbox(&self, conn: &PgConnection) -> String {
        self.compute_box(conn, "outbox")
    }

    fn compute_inbox(&self, conn: &PgConnection) -> String {
        self.compute_box(conn, "inbox")
    }

    fn compute_box(&self, conn: &PgConnection, box_name: &str) -> String {
        format!(
            "https://{instance}/{prefix}/{user}/{name}",
            instance = self.get_instance(conn).public_domain,
            prefix = Self::get_box_prefix(),
            user = self.get_actor_id(),
            name = box_name
        )
    }
}
