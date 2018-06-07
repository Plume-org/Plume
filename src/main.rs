#![feature(plugin, custom_derive, iterator_find_map)]
#![plugin(rocket_codegen)]

extern crate activitystreams;
#[macro_use]
extern crate activitystreams_derive;
extern crate activitystreams_traits;
extern crate activitystreams_types;
extern crate array_tool;
extern crate base64;
extern crate bcrypt;
extern crate chrono;
extern crate comrak;
extern crate failure;
#[macro_use]
extern crate failure_derive;
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

use diesel::{pg::PgConnection, r2d2::{ConnectionManager, Pool}};
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
            routes::blogs::details,
            routes::blogs::activity_details,            
            routes::blogs::outbox,            
            routes::blogs::new,
            routes::blogs::new_auth,
            routes::blogs::create,

            routes::comments::new,
            routes::comments::new_auth,
            routes::comments::create,

            routes::instance::index,
            routes::instance::configure,
            routes::instance::post_config,
            routes::instance::shared_inbox,

            routes::likes::create,
            routes::likes::create_auth,

            routes::notifications::notifications,
            routes::notifications::notifications_auth,

            routes::posts::details,
            routes::posts::activity_details,
            routes::posts::new,
            routes::posts::new_auth,
            routes::posts::create,

            routes::reshares::create,
            routes::reshares::create_auth,

            routes::session::new,
            routes::session::new_message,
            routes::session::create,
            routes::session::delete,

            routes::static_files,

            routes::user::me,
            routes::user::details,
            routes::user::followers,            
            routes::user::edit,
            routes::user::edit_auth,
            routes::user::update,
            routes::user::follow,
            routes::user::follow_auth,
            routes::user::activity_details,
            routes::user::outbox,
            routes::user::inbox,
            routes::user::ap_followers,
            routes::user::new,
            routes::user::create,

            routes::well_known::host_meta,
            routes::well_known::webfinger
        ])
        .manage(init_pool())
        .attach(Template::fairing())
        .launch();
}
