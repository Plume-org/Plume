use clap::{Arg, ArgMatches, Command};

use plume_models::{instance::Instance, posts::Post, timeline::*, users::*, Connection};

pub fn command() -> Command {
    Command::new("timeline")
        .about("Manage public timeline")
        .subcommand(
            Command::new("new")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of this timeline"),
                )
                .arg(
                    Arg::new("query")
                        .short('q')
                        .long("query")
                        .action(clap::ArgAction::Set)
                        .help("The query posts in this timelines have to match"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help(
                            "Username of whom this timeline is for. Empty for an instance timeline",
                        ),
                )
                .arg(
                    Arg::new("preload-count")
                        .short('p')
                        .long("preload-count")
                        .action(clap::ArgAction::Set)
                        .help("Number of posts to try to preload in this timeline at its creation"),
                )
                .about("Create a new timeline"),
        )
        .subcommand(
            Command::new("delete")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of the timeline to delete"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help(
                            "Username of whom this timeline was for. Empty for instance timeline",
                        ),
                )
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .help("Confirm the deletion"),
                )
                .about("Delete a timeline"),
        )
        .subcommand(
            Command::new("edit")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of the timeline to edit"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help("Username of whom this timeline is for. Empty for instance timeline"),
                )
                .arg(
                    Arg::new("query")
                        .short('q')
                        .long("query")
                        .action(clap::ArgAction::Set)
                        .help("The query posts in this timelines have to match"),
                )
                .about("Edit the query of a timeline"),
        )
        .subcommand(
            Command::new("repopulate")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of the timeline to repopulate"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help(
                            "Username of whom this timeline was for. Empty for instance timeline",
                        ),
                )
                .arg(
                    Arg::new("preload-count")
                        .short('p')
                        .long("preload-count")
                        .action(clap::ArgAction::Set)
                        .help("Number of posts to try to preload in this timeline at its creation"),
                )
                .about("Repopulate a timeline. Run this after modifying a list the timeline depends on."),
        )
}

pub async fn run<'a>(mut args: ArgMatches, conn: &mut Connection) {
    if let Some((c, a)) = args.remove_subcommand() {
        match c.as_str() {
            "new" => new(a, conn).await,
            "edit" => edit(a, conn),
            "delete" => delete(a, conn),
            "repopulate" => repopulate(a, conn).await,
            _ => command().print_help().unwrap(),
        }
    } else {
        println!("Unknown subcommand");
    }
}

fn get_timeline_identifier(args: &mut ArgMatches) -> (String, Option<String>) {
    let name = args
        .remove_one::<String>("name")
        .expect("No name provided for the timeline");
    let user = args.remove_one::<String>("user");
    (name, user)
}

fn get_query(args: &mut ArgMatches) -> String {
    let query = args
        .remove_one::<String>("query")
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

fn get_preload_count(args: &mut ArgMatches) -> usize {
    args.remove_one::<usize>("preload-count")
        .unwrap_or(plume_models::ITEMS_PER_PAGE as usize)
}

fn resolve_user(username: &str, conn: &mut Connection) -> User {
    let instance = Instance::get_local_uncached(conn).expect("Failed to load local instance");

    User::find_by_name(conn, username, instance.id).expect("User not found")
}

async fn preload(timeline: Timeline, count: usize, conn: &mut Connection) {
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
        if timeline.matches(conn, &post, &Kind::Original).await.unwrap() {
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

async fn new(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_timeline_identifier(&mut args);
    let query = get_query(&mut args);
    let preload_count = get_preload_count(&mut args);

    let user = user.map(|user| resolve_user(&user, conn));

    let timeline = if let Some(user) = user {
        Timeline::new_for_user(conn, user.id, name, query)
    } else {
        Timeline::new_for_instance(conn, name, query)
    }
    .expect("Failed to create new timeline");

    preload(timeline, preload_count, conn).await;
}

fn edit(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_timeline_identifier(&mut args);
    let query = get_query(&mut args);

    let user = user.map(|user| resolve_user(&user, conn));

    let mut timeline = Timeline::find_for_user_by_name(conn, user.map(|u| u.id), &name)
        .expect("timeline not found");

    timeline.query = query;

    timeline.update(conn).expect("Failed to update timeline");
}

fn delete(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_timeline_identifier(&mut args);

    if !args.contains_id("yes") {
        panic!("Warning, this operation is destructive. Add --yes to confirm you want to do it.")
    }

    let user = user.map(|user| resolve_user(&user, conn));

    let timeline = Timeline::find_for_user_by_name(conn, user.map(|u| u.id), &name)
        .expect("timeline not found");

    timeline.delete(conn).expect("Failed to update timeline");
}

async fn repopulate(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_timeline_identifier(&mut args);
    let preload_count = get_preload_count(&mut args);

    let user = user.map(|user| resolve_user(&user, conn));

    let timeline = Timeline::find_for_user_by_name(conn, user.map(|u| u.id), &name)
        .expect("timeline not found");
    preload(timeline, preload_count, conn).await;
}
