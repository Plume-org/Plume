use clap::{Arg, ArgMatches, App, SubCommand};

use std::env;
use plume_models::{
    Connection,
    instance::*,
    safe_string::SafeString,
};

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("instance")
        .about("Manage instances")
        .subcommand(SubCommand::with_name("new")
            .arg(Arg::with_name("domain")
                .short("d")
                .long("domain")
                .takes_value(true)
                .help("The domain name of your instance")
            ).arg(Arg::with_name("name")
                .short("n")
                .long("name")
                .takes_value(true)
                .help("The name of your instance")
            ).arg(Arg::with_name("default-license")
                .short("l")
                .long("default-license")
                .takes_value(true)
                .help("The license that will be used by default for new articles on this instance")
            ).arg(Arg::with_name("private")
                .short("p")
                .long("private")
                .help("Closes the registrations on this instance")
            ).about("Create a new local instance"))
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("new", Some(x)) => new(x, conn),
        _ => println!("Unknwon subcommand"),
    }
}

fn new<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let domain = args.value_of("domain").map(String::from)
        .unwrap_or_else(|| env::var("BASE_URL")
            .unwrap_or_else(|_| super::ask_for("Domain name")));
    let name = args.value_of("name").map(String::from).unwrap_or_else(|| super::ask_for("Instance name"));
    let license = args.value_of("default-license").map(String::from).unwrap_or(String::from("CC-0"));
    let open_reg = !args.is_present("private");

    Instance::insert(conn, NewInstance {
        public_domain: domain,
        name: name,
        local: true,
        long_description: SafeString::new(""),
        short_description: SafeString::new(""),
        default_license: license,
        open_registrations: open_reg,
        short_description_html: String::new(),
        long_description_html: String::new()
    });
}
