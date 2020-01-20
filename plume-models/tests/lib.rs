use diesel::Connection;
use plume_common::utils::random_hex;
use plume_models::migrations::IMPORTED_MIGRATIONS;
use plume_models::{Connection as Conn, CONFIG};

use std::env::temp_dir;

fn db() -> Conn {
    let conn =
        Conn::establish(CONFIG.database_url.as_str()).expect("Couldn't connect to the database");
    let dir = temp_dir().join(format!("plume-test-{}", random_hex()));
    IMPORTED_MIGRATIONS
        .run_pending_migrations(&conn, &dir)
        .expect("Couldn't run migrations");
    conn
}

#[test]
fn empty_test() {
    let conn = &db();
    conn.test_transaction::<_, (), _>(|| Ok(()));
}
