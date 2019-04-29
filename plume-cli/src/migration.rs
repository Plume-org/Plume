use clap::{App, Arg, ArgMatches, SubCommand};

use plume_models::{migrations::IMPORTED_MIGRATIONS, Connection};
use std::path::Path;

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("migration")
        .about("Manage migrations")
        .subcommand(
            SubCommand::with_name("run")
                .arg(
                    Arg::with_name("path")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .about("Run migrations"),
        )
        .subcommand(
            SubCommand::with_name("redo")
                .arg(
                    Arg::with_name("path")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .about("Rerun latest migration"),
        )
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("run", Some(x)) => run_(x, conn),
        ("redo", Some(x)) => redo(x, conn),
        ("", None) => command().print_help().unwrap(),
        _ => println!("Unknown subcommand"),
    }
}

fn run_<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let path = args.value_of("path").unwrap_or(".");

    IMPORTED_MIGRATIONS
        .run_pending_migrations(conn, Path::new(path))
        .expect("Failed to run migrations")
}

fn redo<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let path = args.value_of("path").unwrap_or(".");

    IMPORTED_MIGRATIONS
        .rerun_last_migration(conn, Path::new(path))
        .expect("Failed to rerun migrations")
}
