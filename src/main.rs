#![allow(clippy::too_many_arguments)]
#![feature(decl_macro, proc_macro_hygiene)]

extern crate activitypub;
extern crate askama_escape;
extern crate atom_syndication;
extern crate canapi;
extern crate chrono;
extern crate colored;
extern crate ctrlc;
extern crate diesel;
extern crate dotenv;
extern crate failure;
#[macro_use]
extern crate gettext_macros;
extern crate gettext_utils;
extern crate guid_create;
extern crate heck;
extern crate lettre;
extern crate lettre_email;
extern crate multipart;
extern crate num_cpus;
extern crate plume_api;
extern crate plume_common;
extern crate plume_models;
#[macro_use]
extern crate rocket;
extern crate rocket_contrib;
extern crate rocket_csrf;
extern crate rocket_i18n;
extern crate scheduled_thread_pool;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate serde_qs;
extern crate validator;
#[macro_use]
extern crate validator_derive;
extern crate webfinger;

use diesel::r2d2::ConnectionManager;
use plume_models::{
    db_conn::{DbPool, PragmaForeignKey},
    search::{Searcher as UnmanagedSearcher, SearcherError},
    Connection, Error, CONFIG,
};
use rocket::State;
use rocket_csrf::CsrfFairingBuilder;
use scheduled_thread_pool::ScheduledThreadPool;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Duration;

init_i18n!("plume", ar, de, en, es, fr, gl, it, ja, nb, pl, pt, ru);

mod api;
mod inbox;
mod mail;
#[macro_use]
mod template_utils;
mod routes;

include!(concat!(env!("OUT_DIR"), "/templates.rs"));

compile_i18n!();

type Worker<'a> = State<'a, ScheduledThreadPool>;
type Searcher<'a> = State<'a, Arc<UnmanagedSearcher>>;

/// Initializes a database pool.
fn init_pool() -> Option<DbPool> {
    dotenv::dotenv().ok();

    let manager = ConnectionManager::<Connection>::new(CONFIG.database_url.as_str());
    DbPool::builder()
        .connection_customizer(Box::new(PragmaForeignKey))
        .build(manager)
        .ok()
}

fn main() {
    let dbpool = init_pool().expect("main: database pool initialization error");
    let workpool = ScheduledThreadPool::with_name("worker {}", num_cpus::get());
    // we want a fast exit here, so
    #[allow(clippy::match_wild_err_arm)]
    let searcher = match UnmanagedSearcher::open(&CONFIG.search_index) {
        Err(Error::Search(e)) => match e {
            SearcherError::WriteLockAcquisitionError => panic!(
                r#"
Your search index is locked. Plume can't start. To fix this issue
make sure no other Plume instance is started, and run:

    plm search unlock

Then try to restart Plume.
"#
            ),
            SearcherError::IndexOpeningError => panic!(
                r#"
Plume was unable to open the search index. If you created the index
before, make sure to run Plume in the directory it was created, or 
to set SEARCH_INDEX accordingly. If you did not create the search 
index, run this command:

    plm search init

Then try to restart Plume
"#
            ),
            e => Err(e).unwrap(),
        },
        Err(_) => panic!("Unexpected error while opening search index"),
        Ok(s) => Arc::new(s),
    };

    let commiter = searcher.clone();
    workpool.execute_with_fixed_delay(
        Duration::from_secs(5),
        Duration::from_secs(60 * 30),
        move || commiter.commit(),
    );

    let search_unlocker = searcher.clone();
    ctrlc::set_handler(move || {
        search_unlocker.drop_writer();
        exit(0);
    })
    .expect("Error setting Ctrl-c handler");

    let mail = mail::init();
    if mail.is_none() && CONFIG.rocket.as_ref().unwrap().environment.is_prod() {
        println!("Warning: the email server is not configured (or not completely).");
        println!("Please refer to the documentation to see how to configure it.");
    }

    rocket::custom(CONFIG.rocket.clone().unwrap())
        .mount(
            "/",
            routes![
                routes::blogs::details,
                routes::blogs::activity_details,
                routes::blogs::outbox,
                routes::blogs::new,
                routes::blogs::new_auth,
                routes::blogs::create,
                routes::blogs::delete,
                routes::blogs::edit,
                routes::blogs::update,
                routes::blogs::atom_feed,
                routes::comments::create,
                routes::comments::delete,
                routes::comments::activity_pub,
                routes::instance::index,
                routes::instance::local,
                routes::instance::feed,
                routes::instance::federated,
                routes::instance::admin,
                routes::instance::admin_instances,
                routes::instance::admin_users,
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
                routes::notifications::notifications,
                routes::notifications::notifications_auth,
                routes::posts::details,
                routes::posts::activity_details,
                routes::posts::edit,
                routes::posts::update,
                routes::posts::new,
                routes::posts::new_auth,
                routes::posts::create,
                routes::posts::delete,
                routes::reshares::create,
                routes::reshares::create_auth,
                routes::search::search,
                routes::session::new,
                routes::session::create,
                routes::session::delete,
                routes::session::password_reset_request_form,
                routes::session::password_reset_request,
                routes::session::password_reset_form,
                routes::session::password_reset,
                routes::plume_static_files,
                routes::static_files,
                routes::tags::tag,
                routes::user::me,
                routes::user::details,
                routes::user::dashboard,
                routes::user::dashboard_auth,
                routes::user::followers,
                routes::user::followed,
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
            ],
        )
        .mount(
            "/api/v1",
            routes![
                api::oauth,
                api::apps::create,
                api::posts::get,
                api::posts::list,
                api::posts::create,
            ],
        )
        .register(catchers![
            routes::errors::not_found,
            routes::errors::unprocessable_entity,
            routes::errors::server_error
        ])
        .manage(Arc::new(Mutex::new(mail)))
        .manage::<Arc<Mutex<Vec<routes::session::ResetRequest>>>>(Arc::new(Mutex::new(vec![])))
        .manage(dbpool)
        .manage(workpool)
        .manage(searcher)
        .manage(include_i18n!())
        .attach(
            CsrfFairingBuilder::new()
                .set_default_target(
                    "/csrf-violation?target=<uri>".to_owned(),
                    rocket::http::Method::Post,
                )
                .add_exceptions(vec![
                    (
                        "/inbox".to_owned(),
                        "/inbox".to_owned(),
                        rocket::http::Method::Post,
                    ),
                    (
                        "/@/<name>/inbox".to_owned(),
                        "/@/<name>/inbox".to_owned(),
                        rocket::http::Method::Post,
                    ),
                    (
                        "/login".to_owned(),
                        "/login".to_owned(),
                        rocket::http::Method::Post,
                    ),
                    (
                        "/users/new".to_owned(),
                        "/users/new".to_owned(),
                        rocket::http::Method::Post,
                    ),
                    (
                        "/api/<path..>".to_owned(),
                        "/api/<path..>".to_owned(),
                        rocket::http::Method::Post,
                    ),
                ])
                .finalize()
                .expect("main: csrf fairing creation error"),
        )
        .launch();
}
