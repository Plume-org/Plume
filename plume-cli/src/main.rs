extern crate clap;
extern crate diesel;
extern crate dotenv;
extern crate plume_models;
extern crate rpassword;

use clap::App;
use diesel::Connection;
use std::io::{self, prelude::*};
use plume_models::{DB_URL, Connection as Conn};

mod instance;
mod users;

fn main() {
    let mut app = App::new("Plume CLI")
        .bin_name("plm")
        .version("0.2.0")
        .about("Collection of tools to manage your Plume instance.")
        .subcommand(instance::command())
        .subcommand(users::command());
    let matches = app.clone().get_matches();

    dotenv::dotenv().ok();
    let conn = Conn::establish(DB_URL.as_str());

    match matches.subcommand() {
        ("instance", Some(args)) => instance::run(args, &conn.expect("Couldn't connect to the database.")),
        ("users", Some(args)) => users::run(args, &conn.expect("Couldn't connect to the database.")),
        _ => app.print_help().unwrap()
    };
}

pub fn ask_for(something: &str) -> String {
    print!("{}: ", something);
    io::stdout().flush().expect("Couldn't flush STDOUT");
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Unable to read line");
    input.retain(|c| c != '\n');
    input
}
