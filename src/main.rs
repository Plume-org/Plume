#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate rocket;
extern crate rocket_contrib;
extern crate bcrypt;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenv::dotenv;
use std::env;
use rocket_contrib::Template;

mod db_conn;
mod models;
mod schema;
mod routes;

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
            routes::instance::configure,
            routes::instance::post_config,

            routes::user::me,
            routes::user::details,
            routes::user::new,
            routes::user::create,

            routes::session::new,
            routes::session::create
        ])
        .manage(init_pool())
        .attach(Template::fairing())
        .launch();
}
