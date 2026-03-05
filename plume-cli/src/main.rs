use clap::Command;
use diesel::Connection;
use plume_models::{instance::Instance, Connection as Conn, CONFIG};
use std::io::{self, prelude::*};

mod instance;
mod list;
mod migration;
mod search;
mod timeline;
mod users;

#[tokio::main]
async fn main() {
    let mut app = Command::new("Plume CLI")
        .bin_name("plm")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Collection of tools to manage your Plume instance.")
        .subcommand(instance::command())
        .subcommand(migration::command())
        .subcommand(search::command())
        .subcommand(timeline::command())
        .subcommand(list::command())
        .subcommand(users::command());
    let mut matches = app.clone().get_matches();

    match dotenv::dotenv() {
        Ok(path) => println!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => eprintln!("no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
    let mut conn = Conn::establish(CONFIG.database_url.as_str()).expect("Couldn't connect to the database.");
    let _ = Instance::cache_local(&mut conn);


    if let Some((c, args)) = matches.remove_subcommand() {
        match c.as_str() {
            "instance" => instance::run(args, &mut conn),
            "migration" => migration::run(args, &mut conn),
            "search" => search::run(args, &mut conn),
            "timeline" => timeline::run(args, &mut conn).await,
            "lists" => list::run(args, &mut conn).await,
            "users" => users::run(args, &mut conn),
            _ => app.print_help().expect("Couldn't print help"),
        }
    } else {
        app.print_help().expect("Couldn't print help");
    }
}

pub fn ask_for(something: &str) -> String {
    print!("{}: ", something);
    io::stdout().flush().expect("Couldn't flush STDOUT");
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Unable to read line");
    input.retain(|c| c != '\n');
    input
}
