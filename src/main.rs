#![feature(plugin, custom_derive, iterator_find_map)]
#![plugin(rocket_codegen)]

extern crate base64;
extern crate bcrypt;
extern crate chrono;
extern crate heck;
extern crate hex;
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate diesel;
extern crate dotenv;
#[macro_use]
extern crate lazy_static;
extern crate openssl;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate url;

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

lazy_static! {
    pub static ref BASE_URL: String = env::var("BASE_URL")
        .unwrap_or(format!("127.0.0.1:{}", env::var("ROCKET_PORT").unwrap_or(String::from("8000"))));
    
    pub static ref DB_URL: String = env::var("DB_URL")
        .unwrap_or(format!("postgres://plume:plume@localhost/{}", env::var("DB_NAME").unwrap_or(String::from("plume"))));
}

type PgPool = Pool<ConnectionManager<PgConnection>>;

/// Initializes a database pool.
fn init_pool() -> PgPool {
    dotenv().ok();

    let manager = ConnectionManager::<PgConnection>::new(DB_URL.as_str());
    Pool::new(manager).expect("DB pool error")
}

fn main() {
    rocket::ignite()
        .mount("/", routes![
            routes::static_files,

            routes::well_known::host_meta,
            routes::well_known::webfinger,

            routes::instance::index,
            routes::instance::configure,
            routes::instance::post_config,

            routes::user::me,
            routes::user::details,
            routes::user::followers,            
            routes::user::edit,
            routes::user::update,
            routes::user::follow,
            routes::user::activity_details,
            routes::user::outbox,
            routes::user::inbox,
            routes::user::ap_followers,
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
            routes::posts::activity_details,
            routes::posts::new,
            routes::posts::new_auth,
            routes::posts::create,
            
            routes::comments::new,
            routes::comments::create,

            routes::likes::create
        ])
        .manage(init_pool())
        .attach(Template::fairing())
        .launch();
}
