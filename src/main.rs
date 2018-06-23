#![feature(custom_derive, decl_macro, plugin)]
#![plugin(rocket_codegen)]
 
extern crate activitypub;
extern crate colored;
extern crate diesel;
extern crate dotenv;
extern crate failure;
extern crate gettextrs;
extern crate heck;
extern crate plume_common;
extern crate plume_models;
extern crate rocket;
extern crate rocket_contrib;
extern crate rocket_i18n;
extern crate rpassword;
#[macro_use]
extern crate serde_json;
extern crate webfinger;

use rocket_contrib::Template;

mod inbox;
mod setup;
mod routes;

fn main() {
    let pool = setup::check();
    rocket::ignite()
        .mount("/", routes![
            routes::blogs::details,
            routes::blogs::activity_details,
            routes::blogs::outbox,
            routes::blogs::new,
            routes::blogs::new_auth,
            routes::blogs::create,

            routes::comments::create,
            routes::comments::create_response,

            routes::instance::index,
            routes::instance::shared_inbox,
            routes::instance::nodeinfo,

            routes::likes::create,
            routes::likes::create_auth,

            routes::notifications::notifications,
            routes::notifications::notifications_auth,

            routes::posts::details,
            routes::posts::details_response,
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
            routes::user::dashboard,
            routes::user::dashboard_auth,
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
            routes::well_known::nodeinfo,
            routes::well_known::webfinger
        ])
        .catch(catchers![
            routes::errors::not_found,
            routes::errors::server_error
        ])
        .manage(pool)
        .attach(Template::custom(|engines| {
            rocket_i18n::tera(&mut engines.tera);
        }))
        .attach(rocket_i18n::I18n::new("plume"))
        .launch();
}
