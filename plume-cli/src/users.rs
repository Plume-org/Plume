use clap::{Arg, ArgMatches, App, SubCommand};
use diesel::PgConnection;

use rpassword;
use std::io::{self, Write};
use plume_models::{
    users::*,
};

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("users")
        .about("Manage users")
        .subcommand(SubCommand::with_name("new")
            .arg(Arg::with_name("name")
                .short("n")
                .long("name")
                .alias("username")
                .takes_value(true)
                .help("The username of the new user")
            ).arg(Arg::with_name("display-name")
                .short("N")
                .long("display-name")
                .takes_value(true)
                .help("The display name of the new user")
            ).arg(Arg::with_name("biography")
                .short("b")
                .long("bio")
                .alias("biography")
                .takes_value(true)
                .help("The biography of the new user")
            ).arg(Arg::with_name("email")
                .short("e")
                .long("email")
                .takes_value(true)
                .help("Email address of the new user")
            ).arg(Arg::with_name("password")
                .short("p")
                .long("password")
                .takes_value(true)
                .help("The password of the new user")
            ).arg(Arg::with_name("admin")
                .short("a")
                .long("admin")
                .help("Makes the user an administrator of the instance")
            ).about("Create a new user on this instance"))
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &PgConnection) {
    let conn = conn;
    match args.subcommand() {
        ("new", Some(x)) => new(x, conn),
        _ => println!("Unknwon subcommand"),
    }
}

fn new<'a>(args: &ArgMatches<'a>, conn: &PgConnection) {
    let username = args.value_of("name").map(String::from).unwrap_or_else(|| super::ask_for("Username"));
    let display_name = args.value_of("display-name").map(String::from).unwrap_or_else(|| super::ask_for("Display name"));
    let admin = args.is_present("admin");
    let bio = args.value_of("biography").unwrap_or("").to_string();
    let email = args.value_of("email").map(String::from).unwrap_or_else(|| super::ask_for("Email address"));
    let password = args.value_of("password").map(String::from).unwrap_or_else(|| {
        print!("Password: ");
        io::stdout().flush().expect("Couldn't flush STDOUT");
        rpassword::read_password().expect("Couldn't read your password.")
    });

    NewUser::new_local(
        conn,
        username,
        display_name,
        admin,
        bio,
        email,
        User::hash_pass(password),
    ).update_boxes(conn);
}
