extern crate clap;
extern crate diesel;
extern crate dotenv;
extern crate plume_models;

use clap::App;
use diesel::{Connection, PgConnection};
use std::io::{self, Write};
use plume_models::DB_URL;

mod instance;

fn main() {
    let mut app = App::new("Plume CLI")
        .bin_name("plm")
        .version("0.2.0")
        .about("Collection of tools to manage your Plume instance.")
        .subcommand(instance::command());
    let matches = app.clone().get_matches();

    dotenv::dotenv().ok();
    let conn = PgConnection::establish(DB_URL.as_str());

    match matches.subcommand() {
        ("instance", Some(args)) => instance::run(args, &conn.expect("Couldn't connect to the database.")),
        _ => app.print_help().unwrap()
    };
}

pub fn ask_for(something: &str) -> String {
    write!(io::stdout(), "{}", something).ok();
    write!(io::stdout(), ": ").ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Unable to read line");
    input.retain(|c| c != '\n');
    input
}
