#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate bcrypt;
extern crate heck;
#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate rocket;
extern crate rocket_contrib;
#[feature(custom_attribute)]
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

use db_conn::DbConn;
use models::instance::*;

type PgPool = Pool<ConnectionManager<PgConnection>>;

/// Initializes a database pool.
fn init_pool() -> PgPool {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::new(manager).expect("db pool")
}

#[get("/")]
fn index(conn: DbConn) -> String {
    match Instance::get_local(&*conn) {
        Some(inst) => {
            format!("Welcome on {}", inst.name)
        }
        None => {
            String::from("Not initialized")
        }
    }
}

fn main() {
    rocket::ignite()
        .mount("/", routes![
            routes::well_known::host_meta,
            routes::well_known::webfinger,

            routes::instance::configure,
            routes::instance::post_config,

            routes::user::me,
            routes::user::details,
            routes::user::activity_details,
            routes::user::new,
            routes::user::create,

            routes::session::new,
            routes::session::create,
            routes::session::delete,

            routes::blogs::details,
            routes::blogs::activity_details,            
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
