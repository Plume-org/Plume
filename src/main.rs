#![feature(plugin, custom_derive, iterator_find_map)]
#![plugin(rocket_codegen)]

extern crate base64;
extern crate bcrypt;
extern crate chrono;
extern crate heck;
extern crate hex;
#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate openssl;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenv::dotenv;
use rocket_contrib::Template;
use std::env;

mod activity_pub;
mod db_conn;
mod models;
mod schema;
mod routes;
mod utils;

type PgPool = Pool<ConnectionManager<PgConnection>>;

/// Initializes a database pool.
fn init_pool() -> PgPool {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::new(manager).expect("db pool")
}

fn main() {
    rocket::ignite()
        .mount("/", routes![
            routes::well_known::host_meta,
            routes::well_known::webfinger,

            routes::instance::index,
            routes::instance::configure,
            routes::instance::post_config,

            routes::user::me,
            routes::user::details,
            routes::user::activity_details,
            routes::user::outbox,
            routes::user::inbox,
            routes::user::new,
            routes::user::create,

            routes::session::new,
            routes::session::create,
            routes::session::delete,

            routes::blogs::details,
            routes::blogs::activity_details,            
            routes::blogs::outbox,            
            routes::blogs::new,
            routes::blogs::create,

            routes::posts::details,
            routes::posts::new,
            routes::posts::new_auth,
            routes::posts::create
        ])
        .manage(init_pool())
        .attach(Template::fairing())
        .launch();
}
