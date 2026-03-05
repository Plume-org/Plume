use clap::{Arg, ArgMatches, Command};

use plume_models::{instance::Instance, users::*, Connection};
use std::io::{self, Write};

pub fn command() -> Command {
    Command::new("users")
        .about("Manage users")
        .subcommand(
            Command::new("new")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .alias("username")
                        .action(clap::ArgAction::Set)
                        .help("The username of the new user"),
                )
                .arg(
                    Arg::new("display-name")
                        .short('N')
                        .long("display-name")
                        .action(clap::ArgAction::Set)
                        .help("The display name of the new user"),
                )
                .arg(
                    Arg::new("biography")
                        .short('b')
                        .long("bio")
                        .alias("biography")
                        .action(clap::ArgAction::Set)
                        .help("The biography of the new user"),
                )
                .arg(
                    Arg::new("email")
                        .short('e')
                        .long("email")
                        .action(clap::ArgAction::Set)
                        .help("Email address of the new user"),
                )
                .arg(
                    Arg::new("password")
                        .short('p')
                        .long("password")
                        .action(clap::ArgAction::Set)
                        .help("The password of the new user"),
                )
                .arg(
                    Arg::new("admin")
                        .short('a')
                        .long("admin")
                        .action(clap::ArgAction::SetTrue)
                        .help("Makes the user an administrator of the instance"),
                )
                .arg(
                    Arg::new("moderator")
                        .short('m')
                        .long("moderator")
                        .action(clap::ArgAction::SetTrue)
                        .help("Makes the user a moderator of the instance"),
                )
                .about("Create a new user on this instance"),
        )
        .subcommand(
            Command::new("reset-password")
                .arg(
                    Arg::new("name")
                        .short('u')
                        .long("user")
                        .alias("username")
                        .action(clap::ArgAction::Set)
                        .help("The username of the user to reset password to"),
                )
                .arg(
                    Arg::new("password")
                        .short('p')
                        .long("password")
                        .action(clap::ArgAction::Set)
                        .help("The password new for the user"),
                )
                .about("Reset user password"),
        )
}

pub fn run(mut args: ArgMatches, conn: &mut Connection) {
    args.remove_subcommand().map(|(c, a)| {
        match c.as_str() {
            "new" => new(a, conn),
            "reset-password" => reset_password(a, conn),
            _ => command().print_help().unwrap(),
        }
    }).unwrap_or_else(|| println!("Unknown subcommand") )
}

fn new(mut args: ArgMatches, conn: &mut Connection) {
    let username = args
        .remove_one::<String>("name")
        .unwrap_or_else(|| super::ask_for("Username"));
    let display_name = args
        .remove_one::<String>("display-name")
        .unwrap_or_else(|| super::ask_for("Display name"));

    let admin = args.contains_id("admin");
    let moderator = args.contains_id("moderator");
    let role = if admin {
        Role::Admin
    } else if moderator {
        Role::Moderator
    } else {
        Role::Normal
    };

    let bio = args.remove_one::<String>("biography").unwrap_or(String::new());
    let email = args
        .remove_one::<String>("email")
        .unwrap_or_else(|| super::ask_for("Email address"));
    let password = args
        .remove_one::<String>("password")
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

fn reset_password(mut args: ArgMatches, conn: &mut Connection) {
    let username = args
        .remove_one::<String>("name")
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
        .remove_one::<String>("password")
        .unwrap_or_else(|| {
            print!("Password: ");
            io::stdout().flush().expect("Couldn't flush STDOUT");
            rpassword::read_password().expect("Couldn't read your password.")
        });
    user.reset_password(conn, &password)
        .expect("Failed to reset password");
}
