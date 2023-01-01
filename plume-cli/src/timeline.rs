use clap::{App, Arg, ArgMatches, SubCommand};

use plume_models::{instance::Instance, posts::Post, timeline::*, users::*, Connection};

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("timeline")
        .about("Manage public timeline")
        .subcommand(
            SubCommand::with_name("new")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of this timeline"),
                )
                .arg(
                    Arg::with_name("query")
                        .short("q")
                        .long("query")
                        .takes_value(true)
                        .help("The query posts in this timelines have to match"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help(
                            "Username of whom this timeline is for. Empty for an instance timeline",
                        ),
                )
                .arg(
                    Arg::with_name("preload-count")
                        .short("p")
                        .long("preload-count")
                        .takes_value(true)
                        .help("Number of posts to try to preload in this timeline at its creation"),
                )
                .about("Create a new timeline"),
        )
        .subcommand(
            SubCommand::with_name("delete")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of the timeline to delete"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help(
                            "Username of whom this timeline was for. Empty for instance timeline",
                        ),
                )
                .arg(
                    Arg::with_name("yes")
                        .short("y")
                        .long("yes")
                        .help("Confirm the deletion"),
                )
                .about("Delete a timeline"),
        )
        .subcommand(
            SubCommand::with_name("edit")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of the timeline to edit"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help("Username of whom this timeline is for. Empty for instance timeline"),
                )
                .arg(
                    Arg::with_name("query")
                        .short("q")
                        .long("query")
                        .takes_value(true)
                        .help("The query posts in this timelines have to match"),
                )
                .about("Edit the query of a timeline"),
        )
        .subcommand(
            SubCommand::with_name("repopulate")
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .help("The name of the timeline to repopulate"),
                )
                .arg(
                    Arg::with_name("user")
                        .short("u")
                        .long("user")
                        .takes_value(true)
                        .help(
                            "Username of whom this timeline was for. Empty for instance timeline",
                        ),
                )
                .arg(
                    Arg::with_name("preload-count")
                        .short("p")
                        .long("preload-count")
                        .takes_value(true)
                        .help("Number of posts to try to preload in this timeline at its creation"),
                )
                .about("Repopulate a timeline. Run this after modifying a list the timeline depends on."),
        )
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("new", Some(x)) => new(x, conn),
        ("edit", Some(x)) => edit(x, conn),
        ("delete", Some(x)) => delete(x, conn),
        ("repopulate", Some(x)) => repopulate(x, conn),
        ("", None) => command().print_help().unwrap(),
        _ => println!("Unknown subcommand"),
    }
}

fn get_timeline_identifier(args: &ArgMatches<'_>) -> (String, Option<String>) {
    let name = args
        .value_of("name")
        .map(String::from)
        .expect("No name provided for the timeline");
    let user = args.value_of("user").map(String::from);
    (name, user)
}

fn get_query(args: &ArgMatches<'_>) -> String {
    let query = args
        .value_of("query")
        .map(String::from)
        .expect("No query provided");

    match TimelineQuery::parse(&query) {
        Ok(_) => (),
        Err(QueryError::SyntaxError(start, end, message)) => panic!(
            "Query parsing error between {} and {}: {}",
            start, end, message
        ),
        Err(QueryError::UnexpectedEndOfQuery) => {
            panic!("Query parsing error: unexpected end of query")
        }
        Err(QueryError::RuntimeError(message)) => panic!("Query parsing error: {}", message),
    }

    query
}

fn get_preload_count(args: &ArgMatches<'_>) -> usize {
    args.value_of("preload-count")
        .map(|arg| arg.parse().expect("invalid preload-count"))
        .unwrap_or(plume_models::ITEMS_PER_PAGE as usize)
}

fn resolve_user(username: &str, conn: &Connection) -> User {
    let instance = Instance::get_local_uncached(conn).expect("Failed to load local instance");

    User::find_by_name(conn, username, instance.id).expect("User not found")
}

fn preload(timeline: Timeline, count: usize, conn: &Connection) {
    timeline.remove_all_posts(conn).unwrap();

    if count == 0 {
        return;
    }

    let mut posts = Vec::with_capacity(count as usize);
    for post in Post::list_filtered(conn, None, None, None)
        .unwrap()
        .into_iter()
        .rev()
    {
        if timeline.matches(conn, &post, Kind::Original).unwrap() {
            posts.push(post);
            if posts.len() >= count {
                break;
            }
        }
    }

    for post in posts.iter().rev() {
        timeline.add_post(conn, post).unwrap();
    }
}

fn new(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_timeline_identifier(args);
    let query = get_query(args);
    let preload_count = get_preload_count(args);

    let user = user.map(|user| resolve_user(&user, conn));

    let timeline = if let Some(user) = user {
        Timeline::new_for_user(conn, user.id, name, query)
    } else {
        Timeline::new_for_instance(conn, name, query)
    }
    .expect("Failed to create new timeline");

    preload(timeline, preload_count, conn);
}

fn edit(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_timeline_identifier(args);
    let query = get_query(args);

    let user = user.map(|user| resolve_user(&user, conn));

    let mut timeline = Timeline::find_for_user_by_name(conn, user.map(|u| u.id), &name)
        .expect("timeline not found");

    timeline.query = query;

    timeline.update(conn).expect("Failed to update timeline");
}

fn delete(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_timeline_identifier(args);

    if !args.is_present("yes") {
        panic!("Warning, this operation is destructive. Add --yes to confirm you want to do it.")
    }

    let user = user.map(|user| resolve_user(&user, conn));

    let timeline = Timeline::find_for_user_by_name(conn, user.map(|u| u.id), &name)
        .expect("timeline not found");

    timeline.delete(conn).expect("Failed to update timeline");
}

fn repopulate(args: &ArgMatches<'_>, conn: &Connection) {
    let (name, user) = get_timeline_identifier(args);
    let preload_count = get_preload_count(args);

    let user = user.map(|user| resolve_user(&user, conn));

    let timeline = Timeline::find_for_user_by_name(conn, user.map(|u| u.id), &name)
        .expect("timeline not found");
    preload(timeline, preload_count, conn);
}
