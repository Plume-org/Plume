#![feature(custom_derive, plugin, decl_macro, proc_macro_hygiene)]

extern crate activitypub;
extern crate askama_escape;
extern crate atom_syndication;
extern crate canapi;
extern crate chrono;
extern crate colored;
extern crate diesel;
extern crate dotenv;
extern crate failure;
extern crate guid_create;
extern crate heck;
extern crate multipart;
extern crate plume_api;
extern crate plume_common;
extern crate plume_models;
#[macro_use]
extern crate rocket;
extern crate rocket_contrib;
extern crate rocket_csrf;
#[macro_use]
extern crate rocket_i18n;
extern crate rpassword;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate serde_qs;
extern crate validator;
#[macro_use]
extern crate validator_derive;
extern crate webfinger;
extern crate workerpool;

use diesel::r2d2::ConnectionManager;
use rocket::State;
use rocket_csrf::CsrfFairingBuilder;
use plume_models::{DATABASE_URL, Connection, db_conn::DbPool};
use workerpool::{Pool, thunk::ThunkWorker};

mod api;
mod inbox;
#[macro_use]
mod template_utils;
mod routes;

type Worker<'a> = State<'a, Pool<ThunkWorker<()>>>;

/// Initializes a database pool.
fn init_pool() -> Option<DbPool> {
    dotenv::dotenv().ok();

    let manager = ConnectionManager::<Connection>::new(DATABASE_URL.as_str());
    DbPool::new(manager).ok()
}

fn main() {
    let pool = init_pool().expect("main: database pool initialization error");
    rocket::ignite()
        .mount("/", routes![
            routes::blogs::paginated_details,
            routes::blogs::details,
            routes::blogs::activity_details,
            routes::blogs::outbox,
            routes::blogs::new,
            routes::blogs::new_auth,
            routes::blogs::create,
            routes::blogs::delete,
            routes::blogs::atom_feed,

            routes::comments::create,
            routes::comments::activity_pub,

            routes::instance::index,
            routes::instance::paginated_local,
            routes::instance::local,
            routes::instance::paginated_feed,
            routes::instance::feed,
            routes::instance::paginated_federated,
            routes::instance::federated,
            routes::instance::admin,
            routes::instance::admin_instances,
            routes::instance::admin_instances_paginated,
            routes::instance::admin_users,
            routes::instance::admin_users_paginated,
            routes::instance::ban,
            routes::instance::toggle_block,
            routes::instance::update_settings,
            routes::instance::shared_inbox,
            routes::instance::nodeinfo,
            routes::instance::about,
            routes::instance::web_manifest,

            routes::likes::create,
            routes::likes::create_auth,

            routes::medias::list,
            routes::medias::new,
            routes::medias::upload,
            routes::medias::details,
            routes::medias::delete,
            routes::medias::set_avatar,

            routes::notifications::paginated_notifications,
            routes::notifications::notifications,
            routes::notifications::notifications_auth,

            routes::posts::details,
            routes::posts::details_response,
            routes::posts::activity_details,
            routes::posts::edit,
            routes::posts::update,
            routes::posts::new,
            routes::posts::new_auth,
            routes::posts::create,
            routes::posts::delete,

            routes::reshares::create,
            routes::reshares::create_auth,

            routes::session::new,
            routes::session::new_message,
            routes::session::create,
            routes::session::delete,

            routes::static_files,

            routes::tags::tag,
            routes::tags::paginated_tag,

            routes::user::me,
            routes::user::details,
            routes::user::dashboard,
            routes::user::dashboard_auth,
            routes::user::followers_paginated,
            routes::user::followers,
            routes::user::edit,
            routes::user::edit_auth,
            routes::user::update,
            routes::user::delete,
            routes::user::follow,
            routes::user::follow_auth,
            routes::user::activity_details,
            routes::user::outbox,
            routes::user::inbox,
            routes::user::ap_followers,
            routes::user::new,
            routes::user::create,
            routes::user::atom_feed,

            routes::well_known::host_meta,
            routes::well_known::nodeinfo,
            routes::well_known::webfinger,

            routes::errors::csrf_violation
        ])
        .mount("/api/v1", routes![
            api::oauth,

            api::apps::create,

            api::posts::get,
            api::posts::list,
        ])
        .register(catchers![
            routes::errors::not_found,
            routes::errors::server_error
        ])
        .manage(pool)
        .manage(Pool::<ThunkWorker<()>>::new(4))
        .manage(include_i18n!("plume", [ "de", "en", "fr", "gl", "it", "nb", "pl", "ru" ]))
        .attach(CsrfFairingBuilder::new()
                .set_default_target("/csrf-violation?target=<uri>".to_owned(), rocket::http::Method::Post)
                .add_exceptions(vec![
                    ("/inbox".to_owned(), "/inbox".to_owned(), rocket::http::Method::Post),
                    ("/@/<name>/inbox".to_owned(), "/@/<name>/inbox".to_owned(), rocket::http::Method::Post),
                    ("/login".to_owned(), "/login".to_owned(), rocket::http::Method::Post),
                    ("/users/new".to_owned(), "/users/new".to_owned(), rocket::http::Method::Post),
                    ("/api/<path..>".to_owned(), "/api/<path..>".to_owned(), rocket::http::Method::Post)
                ])
                .finalize().expect("main: csrf fairing creation error"))
        .launch();
}

include!(concat!(env!("OUT_DIR"), "/templates.rs"));
