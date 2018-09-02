#![feature(custom_derive, decl_macro, plugin)]
#![plugin(rocket_codegen)]

extern crate activitypub;
extern crate atom_syndication;
extern crate colored;
extern crate diesel;
extern crate dotenv;
extern crate failure;
extern crate gettextrs;
extern crate guid_create;
extern crate heck;
extern crate multipart;
extern crate plume_common;
extern crate plume_models;
extern crate rocket;
extern crate rocket_contrib;
extern crate rocket_csrf;
extern crate rocket_i18n;
extern crate rpassword;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate validator;
#[macro_use]
extern crate validator_derive;
extern crate webfinger;
extern crate workerpool;

use rocket_contrib::Template;
use rocket_csrf::CsrfFairingBuilder;
use workerpool::{Pool, thunk::ThunkWorker};

mod inbox;
mod setup;
mod routes;

fn main() {
    let pool = setup::check();
    rocket::ignite()
        .mount("/", routes![
            routes::blogs::paginated_details,
            routes::blogs::details,
            routes::blogs::activity_details,
            routes::blogs::outbox,
            routes::blogs::new,
            routes::blogs::new_auth,
            routes::blogs::create,
            routes::blogs::atom_feed,

            routes::comments::create,

            routes::instance::paginated_index,
            routes::instance::index,
            routes::instance::admin,
            routes::instance::update_settings,
            routes::instance::shared_inbox,
            routes::instance::nodeinfo,
            routes::instance::about,

            routes::likes::create,
            routes::likes::create_auth,

            routes::medias::list,
            routes::medias::new,
            routes::medias::upload,
            routes::medias::details,
            routes::medias::static_files,

            routes::notifications::paginated_notifications,
            routes::notifications::notifications,
            routes::notifications::notifications_auth,

            routes::posts::details,
            routes::posts::details_response,
            routes::posts::activity_details,
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

            routes::user::me,
            routes::user::details,
            routes::user::dashboard,
            routes::user::dashboard_auth,
            routes::user::followers_paginated,
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
            routes::user::atom_feed,

            routes::well_known::host_meta,
            routes::well_known::nodeinfo,
            routes::well_known::webfinger,

            routes::errors::csrf_violation
        ])
        .catch(catchers![
            routes::errors::not_found,
            routes::errors::server_error
        ])
        .manage(pool)
        .manage(Pool::<ThunkWorker<()>>::new(4))
        .attach(Template::custom(|engines| {
            rocket_i18n::tera(&mut engines.tera);
        }))
        .attach(rocket_i18n::I18n::new("plume"))
        .attach(CsrfFairingBuilder::new()
                .set_default_target("/csrf-violation?target=<uri>".to_owned(), rocket::http::Method::Post)
                .add_exceptions(vec![
                    ("/inbox".to_owned(), "/inbox".to_owned(), rocket::http::Method::Post),
                    ("/@/<name>/inbox".to_owned(), "/@/<name>/inbox".to_owned(), rocket::http::Method::Post),
                    ("/medias/new".to_owned(), "/medias/new".to_owned(), rocket::http::Method::Post), // not compatible with multipart/form-data
                ])
                .finalize().unwrap())
        .launch();
}
