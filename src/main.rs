#![allow(clippy::too_many_arguments)]

#[macro_use]
extern crate gettext_macros;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_json;

use clap::Command;
use diesel::r2d2::ConnectionManager;
use plume_models::{
    db_conn::{DbPool, PragmaForeignKey},
    instance::Instance,
    remote_fetch_actor::RemoteFetchActor,
    search::{actor::SearchActor, Searcher as UnmanagedSearcher},
    Connection, CONFIG,
};
use rocket_csrf::Fairing;
use scheduled_thread_pool::ScheduledThreadPool;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::warn;

init_i18n!(
    "plume", af, ar, bg, ca, cs, cy, da, de, el, en, eo, es, eu, fa, fi, fr, gl, he, hi, hr, hu,
    it, ja, ko, nb, nl, no, pl, pt, ro, ru, sat, si, sk, sl, sr, sv, tr, uk, vi, zh
);

mod api;
mod inbox;
mod mail;
mod utils;
#[macro_use]
mod template_utils;
mod routes;
#[macro_use]
extern crate shrinkwraprs;
#[cfg(feature = "test")]
mod test_routes;

include!(concat!(env!("OUT_DIR"), "/templates.rs"));

compile_i18n!();

/// Initializes a database pool.
fn init_pool() -> Option<DbPool> {
    let manager = ConnectionManager::<Connection>::new(CONFIG.database_url.as_str());
    let mut builder = DbPool::builder()
        .connection_customizer(Box::new(PragmaForeignKey))
        .min_idle(CONFIG.db_min_idle);
    if let Some(max_size) = CONFIG.db_max_size {
        builder = builder.max_size(max_size);
    };
    let pool = builder.build(manager).ok()?;
    Some(pool)
}

pub(crate) fn init_rocket() -> rocket::Rocket<rocket::Build> {
    match dotenv::dotenv() {
        Ok(path) => eprintln!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => eprintln!("no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
    tracing_subscriber::fmt::init();

    Command::new("Plume")
        .bin_name("plume")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Plume backend server")
        .after_help(
            r#"
The plume command should be run inside the directory
containing the `.env` configuration file and `static` directory.
See https://docs.joinplu.me/installation/config
and https://docs.joinplu.me/installation/init for more info.
        "#,
        )
        .get_matches();
    let dbpool = init_pool().expect("main: database pool initialization error");

    let mut conn = dbpool.get().expect("Failed to retrieve connection");
    plume_models::migrations::run_pending_migrations(&mut conn).expect("Migrations error");

    Instance::cache_local(&mut conn);
    let _ = Instance::create_local_instance_user(&mut conn);
    Instance::cache_local_instance_user(&mut conn);

    let workpool = ScheduledThreadPool::builder().num_threads(num_cpus::get()).thread_name_pattern("worker {}").build();
    // we want a fast exit here, so
    let searcher = Arc::new(UnmanagedSearcher::open_or_recreate(
        &CONFIG.search_index,
        &CONFIG.search_tokenizers,
    ));
    RemoteFetchActor::init(dbpool.clone());
    SearchActor::init(searcher.clone(), dbpool.clone());
    let commiter = searcher.clone();
    workpool.execute_with_fixed_delay(
        Duration::from_secs(5),
        Duration::from_secs(60 * 30),
        move || commiter.commit(),
    );

    let search_unlocker = searcher.clone();
    ctrlc::set_handler(move || {
        search_unlocker.commit();
        search_unlocker.drop_writer();
        exit(0);
    })
    .expect("Error setting Ctrl-c handler");

    let mail = mail::init();
    if mail.is_none() {
        warn!("Warning: the email server is not configured (or not completely).");
        warn!("Please refer to the documentation to see how to configure it.");
    }

    rocket::custom(CONFIG.rocket.clone().unwrap())
        .mount(
            "/",
            routes![
                routes::blogs::details,
                routes::blogs::activity_details,
                routes::blogs::outbox,
                routes::blogs::outbox_page,
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
                routes::email_signups::create,
                routes::email_signups::created,
                routes::email_signups::show,
                routes::email_signups::signup,
                routes::instance::index,
                routes::instance::admin,
                routes::instance::admin_mod,
                routes::instance::admin_instances,
                routes::instance::admin_users,
                routes::instance::admin_search_users,
                routes::instance::admin_email_blocklist,
                routes::instance::add_email_blocklist,
                routes::instance::delete_email_blocklist,
                routes::instance::edit_users,
                routes::instance::toggle_block,
                routes::instance::update_settings,
                routes::instance::shared_inbox,
                routes::instance::interact,
                routes::instance::nodeinfo,
                routes::instance::about,
                routes::instance::privacy,
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
                routes::posts::remote_interact,
                routes::posts::remote_interact_post,
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
                routes::theme_files,
                routes::plume_static_files,
                routes::static_files,
                routes::plume_media_files,
                routes::tags::tag,
                routes::timelines::details,
                routes::timelines::new,
                routes::timelines::create,
                routes::timelines::edit,
                routes::timelines::update,
                routes::timelines::delete,
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
                routes::user::follow_not_connected,
                routes::user::follow_auth,
                routes::user::activity_details,
                routes::user::outbox,
                routes::user::outbox_page,
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
                api::posts::delete,
            ],
        )
        .register("/", catchers![
            routes::errors::not_found,
            routes::errors::unprocessable_entity,
            routes::errors::server_error
        ])
        .manage(Arc::new(Mutex::new(mail)))
        .manage::<Arc<Mutex<Vec<routes::session::ResetRequest>>>>(Arc::new(Mutex::new(vec![])))
        .manage(dbpool)
        .manage(Arc::new(workpool))
        .manage(searcher)
        .manage(include_i18n!())
        .attach(Fairing::default())
/*
        .attach(
            CsrfFairingBuilder::new()
                .set_default_target(
                    "/csrf-violation?target=<uri>".to_owned(),
                    rocket::http::Method::Post,
                )
                .add_exceptions(vec![
                    ("/inbox".to_owned(), "/inbox".to_owned(), None),
                    (
                        "/@/<name>/inbox".to_owned(),
                        "/@/<name>/inbox".to_owned(),
                        None,
                    ),
                    ("/api/<path..>".to_owned(), "/api/<path..>".to_owned(), None),
                ])
                .finalize()
                .expect("main: csrf fairing creation error"),
        )
*/
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let rocket = init_rocket();

    #[cfg(feature = "test")]
    let rocket = rocket.mount("/test", routes![test_routes::health,]);

    rocket.launch().await.map(|_|())
}
