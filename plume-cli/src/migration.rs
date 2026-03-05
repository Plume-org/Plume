use clap::{ArgMatches, Command};

use plume_models::Connection;

pub fn command() -> Command {
    Command::new("migration")
        .about("Manage migrations")
        .subcommand(
            Command::new("run").about("Run migrations"),
        )
}

pub fn run(mut args: ArgMatches, conn: &mut Connection) {
    args.remove_subcommand().map(|(c, _)| {
        match c.as_str() {
            "run" => run_(conn),
            _ => command().print_help().unwrap(),
        }
    }).unwrap_or_else(|| println!("Unknown subcommand") )
}

fn run_(conn: &mut Connection) {
    plume_models::migrations::run_pending_migrations(conn)
        .expect("Failed to run migrations");
}
