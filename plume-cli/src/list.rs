use clap::{App, Arg, ArgMatches, SubCommand};

use plume_models::{blogs::Blog, instance::Instance, lists::*, users::User, Connection};

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("lists")
        .about("Manage lists")
        .subcommand(
            SubCommand::with_name("new")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of this list"),
                )
                .arg(
                    Arg::with_name("type")
                        .short("t")
                        .long("type")
                        .takes_value(true)
                        .help(
                            r#"The type of this list (one of "user", "blog", "word" or "prefix")"#,
                        ),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help("Username of whom this list is for. Empty for an instance list"),
                )
                .about("Create a new list"),
        )
        .subcommand(
            SubCommand::with_name("delete")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of the list to delete"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help("Username of whom this list was for. Empty for instance list"),
                )
                .arg(
                    Arg::with_name("yes")
                        .short("y")
                        .long("yes")
                        .help("Confirm the deletion"),
                )
                .about("Delete a list"),
        )
        .subcommand(
            SubCommand::with_name("add")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of the list to add an element to"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help("Username of whom this list is for. Empty for instance list"),
                )
                .arg(
                    Arg::with_name("value")
                        .short("v")
                        .long("value")
                        .takes_value(true)
                        .help("The value to add"),
                )
                .about("Add element to a list"),
        )
        .subcommand(
            SubCommand::with_name("rm")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of the list to remove an element from"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help("Username of whom this list is for. Empty for instance list"),
                )
                .arg(
                    Arg::with_name("value")
                        .short("v")
                        .long("value")
                        .takes_value(true)
                        .help("The value to remove"),
                )
                .about("Remove element from list"),
        )
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("new", Some(x)) => new(x, conn),
        ("delete", Some(x)) => delete(x, conn),
        ("add", Some(x)) => add(x, conn),
        ("rm", Some(x)) => rm(x, conn),
        ("", None) => command().print_help().unwrap(),
        _ => println!("Unknown subcommand"),
    }
}

fn get_list_identifier(args: &ArgMatches<'_>) -> (String, Option<String>) {
    let name = args
        .value_of("name")
        .map(String::from)
        .expect("No name provided for the list");
    let user = args.value_of("user").map(String::from);
    (name, user)
}

fn get_list_type(args: &ArgMatches<'_>) -> ListType {
    let typ = args
        .value_of("type")
        .map(String::from)
        .expect("No name type for the list");
    match typ.as_str() {
        "user" => ListType::User,
        "blog" => ListType::Blog,
        "word" => ListType::Word,
        "prefix" => ListType::Prefix,
        _ => panic!("Invalid list type: {}", typ),
    }
}

fn get_value(args: &ArgMatches<'_>) -> String {
    args.value_of("value")
        .map(String::from)
        .expect("No query provided")
}

fn resolve_user(username: &str, conn: &Connection) -> User {
    let instance = Instance::get_local_uncached(conn).expect("Failed to load local instance");

    User::find_by_name(conn, username, instance.id).expect("User not found")
}

fn new(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_list_identifier(args);
    let typ = get_list_type(args);

    let user = user.map(|user| resolve_user(&user, conn));

    List::new(conn, &name, user.as_ref(), typ).expect("failed to create list");
}

fn delete(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_list_identifier(args);

    if !args.is_present("yes") {
        panic!("Warning, this operation is destructive. Add --yes to confirm you want to do it.")
    }

    let user = user.map(|user| resolve_user(&user, conn));

    let list =
        List::find_for_user_by_name(conn, user.map(|u| u.id), &name).expect("list not found");

    list.delete(conn).expect("Failed to update list");
}

fn add(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_list_identifier(args);
    let value = get_value(args);

    let user = user.map(|user| resolve_user(&user, conn));

    let list =
        List::find_for_user_by_name(conn, user.map(|u| u.id), &name).expect("list not found");

    match list.kind() {
        ListType::Blog => {
            let blog_id = Blog::find_by_fqn(conn, &value).expect("unknown blog").id;
            if !list.contains_blog(conn, blog_id).unwrap() {
                list.add_blogs(conn, &[blog_id]).unwrap();
            }
        }
        ListType::User => {
            let user_id = User::find_by_fqn(conn, &value).expect("unknown user").id;
            if !list.contains_user(conn, user_id).unwrap() {
                list.add_users(conn, &[user_id]).unwrap();
            }
        }
        ListType::Word => {
            if !list.contains_word(conn, &value).unwrap() {
                list.add_words(conn, &[&value]).unwrap();
            }
        }
        ListType::Prefix => {
            if !list.contains_prefix(conn, &value).unwrap() {
                list.add_prefixes(conn, &[&value]).unwrap();
            }
        }
    }
}

fn rm(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_list_identifier(args);
    let value = get_value(args);

    let user = user.map(|user| resolve_user(&user, conn));

    let list =
        List::find_for_user_by_name(conn, user.map(|u| u.id), &name).expect("list not found");

    match list.kind() {
        ListType::Blog => {
            let blog_id = Blog::find_by_fqn(conn, &value).expect("unknown blog").id;
            let mut blogs = list.list_blogs(conn).unwrap();
            if let Some(index) = blogs.iter().position(|b| b.id == blog_id) {
                blogs.swap_remove(index);
                let blogs = blogs.iter().map(|b| b.id).collect::<Vec<_>>();
                list.set_blogs(conn, &blogs).unwrap();
            }
        }
        ListType::User => {
            let user_id = User::find_by_fqn(conn, &value).expect("unknown user").id;
            let mut users = list.list_users(conn).unwrap();
            if let Some(index) = users.iter().position(|u| u.id == user_id) {
                users.swap_remove(index);
                let users = users.iter().map(|u| u.id).collect::<Vec<_>>();
                list.set_users(conn, &users).unwrap();
            }
        }
        ListType::Word => {
            let mut words = list.list_words(conn).unwrap();
            if let Some(index) = words.iter().position(|w| *w == value) {
                words.swap_remove(index);
                let words = words.iter().map(String::as_str).collect::<Vec<_>>();
                list.set_words(conn, &words).unwrap();
            }
        }
        ListType::Prefix => {
            let mut prefixes = list.list_prefixes(conn).unwrap();
            if let Some(index) = prefixes.iter().position(|p| *p == value) {
                prefixes.swap_remove(index);
                let prefixes = prefixes.iter().map(String::as_str).collect::<Vec<_>>();
                list.set_prefixes(conn, &prefixes).unwrap();
            }
        }
    }
}
