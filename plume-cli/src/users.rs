use clap::{App, Arg, ArgMatches, SubCommand};

use plume_models::{instance::Instance, users::*, Connection};
use rpassword;
use std::io::{self, Write};

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("users")
        .about("Manage users")
        .subcommand(
            SubCommand::with_name("new")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .alias("username")
                        .takes_value(true)
                        .help("The username of the new user"),
                )
                .arg(
                    Arg::with_name("display-name")
                        .short("N")
                        .long("display-name")
                        .takes_value(true)
                        .help("The display name of the new user"),
                )
                .arg(
                    Arg::with_name("biography")
                        .short("b")
                        .long("bio")
                        .alias("biography")
                        .takes_value(true)
                        .help("The biography of the new user"),
                )
                .arg(
                    Arg::with_name("email")
                        .short("e")
                        .long("email")
                        .takes_value(true)
                        .help("Email address of the new user"),
                )
                .arg(
                    Arg::with_name("password")
                        .short("p")
                        .long("password")
                        .takes_value(true)
                        .help("The password of the new user"),
                )
                .arg(
                    Arg::with_name("admin")
                        .short("a")
                        .long("admin")
                        .help("Makes the user an administrator of the instance"),
                )
                .arg(
                    Arg::with_name("moderator")
                        .short("m")
                        .long("moderator")
                        .help("Makes the user a moderator of the instance"),
                )
                .about("Create a new user on this instance"),
        )
        .subcommand(
            SubCommand::with_name("reset-password")
                .arg(
                    Arg::with_name("name")
                        .short("u")
                        .long("user")
                        .alias("username")
                        .takes_value(true)
                        .help("The username of the user to reset password to"),
                )
                .arg(
                    Arg::with_name("password")
                        .short("p")
                        .long("password")
                        .takes_value(true)
                        .help("The password new for the user"),
                )
                .about("Reset user password"),
        )
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("new", Some(x)) => new(x, conn),
        ("reset-password", Some(x)) => reset_password(x, conn),
        ("", None) => command().print_help().unwrap(),
        _ => println!("Unknown subcommand"),
    }
}

fn new<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let username = args
        .value_of("name")
        .map(String::from)
        .unwrap_or_else(|| super::ask_for("Username"));
    let display_name = args
        .value_of("display-name")
        .map(String::from)
        .unwrap_or_else(|| super::ask_for("Display name"));

    let admin = args.is_present("admin");
    let moderator = args.is_present("moderator");
    let role = if admin {
        Role::Admin
    } else if moderator {
        Role::Moderator
    } else {
        Role::Normal
    };

    let bio = args.value_of("biography").unwrap_or("").to_string();
    let email = args
        .value_of("email")
        .map(String::from)
        .unwrap_or_else(|| super::ask_for("Email address"));
    let password = args
        .value_of("password")
        .map(String::from)
        .unwrap_or_else(|| {
            print!("Password: ");
            io::stdout().flush().expect("Couldn't flush STDOUT");
            rpassword::read_password().expect("Couldn't read your password.")
        });

    NewUser::new_local(
        conn,
        username,
        display_name,
        role,
        &bio,
        email,
        Some(User::hash_pass(&password).expect("Couldn't hash password")),
    )
    .expect("Couldn't save new user");
}

fn reset_password<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let username = args
        .value_of("name")
        .map(String::from)
        .unwrap_or_else(|| super::ask_for("Username"));
    let user = User::find_by_name(
        conn,
        &username,
        Instance::get_local()
            .expect("Failed to get local instance")
            .id,
    )
    .expect("Failed to get user");
    let password = args
        .value_of("password")
        .map(String::from)
        .unwrap_or_else(|| {
            print!("Password: ");
            io::stdout().flush().expect("Couldn't flush STDOUT");
            rpassword::read_password().expect("Couldn't read your password.")
        });
    user.reset_password(conn, &password)
        .expect("Failed to reset password");
}
