use clap::{Arg, ArgMatches, Command};

use plume_models::{instance::*, safe_string::SafeString, Connection};
use std::env;

pub fn command() -> Command {
    Command::new("instance")
        .about("Manage instances")
        .subcommand(Command::new("new")
            .arg(Arg::new("domain")
                .short('d')
                .long("domain")
                .action(clap::ArgAction::Set)
                .help("The domain name of your instance")
            ).arg(Arg::new("name")
                .short('n')
                .long("name")
                .action(clap::ArgAction::Set)
                .help("The name of your instance")
            ).arg(Arg::new("default-license")
                .short('l')
                .long("default-license")
                .action(clap::ArgAction::Set)
                .help("The license that will be used by default for new articles on this instance")
            ).arg(Arg::new("private")
                .short('p')
                .long("private")
                .action(clap::ArgAction::SetTrue)
                .help("Closes the registrations on this instance")
            ).about("Create a new local instance"))
}

pub fn run(mut args: ArgMatches, conn: &mut Connection) {
    args.remove_subcommand().map(|(c, a)| {
        match c.as_str() {
            "new" => new(a, conn),
            _ => command().print_help().unwrap(),
        }
    }).unwrap_or_else(|| println!("Unknown subcommand") )
}

fn new(mut args: ArgMatches, conn: &mut Connection) {
    let domain = args
        .remove_one::<String>("domain")
        .unwrap_or_else(|| env::var("BASE_URL").unwrap_or_else(|_| super::ask_for("Domain name")));
    let name = args
        .remove_one::<String>("name")
        .unwrap_or_else(|| super::ask_for("Instance name"));
    let license = args
        .remove_one::<String>("default-license")
        .unwrap_or_else(|| String::from("CC-BY-SA"));
    let open_reg = !args.contains_id("private");

    Instance::insert(
        conn,
        NewInstance {
            public_domain: domain,
            name,
            local: true,
            long_description: SafeString::new(""),
            short_description: SafeString::new(""),
            default_license: license,
            open_registrations: open_reg,
            short_description_html: String::new(),
            long_description_html: String::new(),
        },
    )
    .expect("Couldn't save instance");
    Instance::cache_local(conn);
    Instance::create_local_instance_user(conn).expect("Couldn't save local instance user");
}
