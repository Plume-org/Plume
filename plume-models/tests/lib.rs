extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

extern crate plume_models;

use diesel::Connection;
use plume_models::{Connection as Conn, CONFIG};

#[cfg(feature = "sqlite")]
embed_migrations!("../migrations/sqlite");

#[cfg(feature = "postgres")]
embed_migrations!("../migrations/postgres");

fn db() -> Conn {
    let conn =
        Conn::establish(CONFIG.database_url.as_str()).expect("Couldn't connect to the database");
    embedded_migrations::run(&conn).expect("Couldn't run migrations");
    conn
}

#[test]
fn empty_test() {
    let conn = &db();
    conn.test_transaction::<_, (), _>(|| Ok(()));
}
